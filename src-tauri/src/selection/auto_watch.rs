//! Selection UX watcher — Easydict-style:
//! WH_MOUSE_LL gestures → delay 150ms → get selection → pop button / translate
//! Hover dictionary (Alt+dwell) remains polled lightly.

use super::hover_pick::{
    format_dict_body, is_ui_chrome_word, pick_word_line_strip_ocr, pick_word_near_cursor_ocr,
    HoverDedupe,
};
use crate::config::{SelectionTriggerMode, SelectionUxConfig};
use crate::dictionary;
use crate::overlay;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

static WATCHER_RUNNING: AtomicBool = AtomicBool::new(false);

pub struct SelectionAutoWatch {
    config: Arc<Mutex<SelectionUxConfig>>,
    stop: Arc<AtomicBool>,
}

impl SelectionAutoWatch {
    pub fn new(config: SelectionUxConfig) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn update_config(&self, config: SelectionUxConfig) {
        #[cfg(windows)]
        super::mouse_hook::set_min_drag_px(config.min_drag_px);
        *self.config.lock().await = config;
    }

    pub fn start(self: &Arc<Self>, app: AppHandle) {
        if WATCHER_RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let stop = Arc::clone(&self.stop);
        stop.store(false, Ordering::SeqCst);
        let cfg = Arc::clone(&self.config);
        crate::overlay::window_manager::hide_overlay_window(&app);
        let _ = super::pop_button::dismiss(&app);
        tauri::async_runtime::spawn(async move {
            run_loop(app, cfg, stop).await;
            WATCHER_RUNNING.store(false, Ordering::SeqCst);
        });
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

async fn run_loop(app: AppHandle, config: Arc<Mutex<SelectionUxConfig>>, stop: Arc<AtomicBool>) {
    let job_gen = Arc::new(AtomicU64::new(0));

    // --- Easydict: real WH_MOUSE_LL (not GetAsyncKeyState polling) ---
    #[cfg(windows)]
    let mut async_rx = {
        {
            let px = config.lock().await.min_drag_px;
            super::mouse_hook::set_min_drag_px(px);
        }
        let hook_rx = super::mouse_hook::install();
        if hook_rx.is_some() {
            tracing::info!("[selection_ux] mouse hook active");
        } else {
            tracing::warn!("[selection_ux] mouse hook unavailable — gesture path degraded");
        }
        let (async_tx, async_rx) =
            tokio::sync::mpsc::unbounded_channel::<super::mouse_hook::MouseHookEvent>();
        if let Some(rx) = hook_rx {
            std::thread::Builder::new()
                .name("moon-hook-bridge".into())
                .spawn(move || {
                    while let Ok(ev) = rx.recv() {
                        if async_tx.send(ev).is_err() {
                            break;
                        }
                    }
                })
                .ok();
        }
        async_rx
    };
    #[cfg(not(windows))]
    let mut async_rx = {
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        rx
    };

    let mut hover_still_since: Option<Instant> = None;
    let mut hover_anchor = (i32::MIN, i32::MIN);
    let mut hover_dedupe = HoverDedupe::new();
    let mut last_hover_lookup = Instant::now() - Duration::from_secs(10);
    // QTranslate: mouse-leave debounce before hide (~120ms)
    let mut overlay_leave_since: Option<Instant> = None;

    tracing::info!("[selection_ux] watcher started (Easydict WH_MOUSE_LL)");

    while !stop.load(Ordering::SeqCst) {
        // Drain hook events with short timeout so hover still runs
        let timed = tokio::time::timeout(Duration::from_millis(40), async_rx.recv()).await;
        match timed {
            Ok(Some(ev)) => {
                #[cfg(windows)]
                handle_hook_event(&app, &config, &job_gen, ev).await;
                #[cfg(not(windows))]
                let _ = ev;
            },
            Ok(None) => {
                break;
            },
            Err(_) => {},
        }

        let ux = config.lock().await.clone();
        let (cx, cy) = cursor_pos();

        // Mouse-leave dismiss overlay (QTranslate 120ms debounce — no flicker)
        if !crate::overlay::window_manager::overlay_shown_within_ms(800) {
            if let Some((ox, oy, ow, oh)) =
                crate::overlay::window_manager::overlay_screen_bounds(&app)
            {
                let plausible = ow >= 40.0 && oh >= 20.0 && (ox > 2.0 || oy > 2.0 || ow > 100.0);
                let inside = cx >= ox - 12.0
                    && cx <= ox + ow + 12.0
                    && cy >= oy - 12.0
                    && cy <= oy + oh + 12.0;
                let on_pop = super::pop_button::hit_test(&app, cx, cy);
                if plausible && !inside && !on_pop {
                    if overlay_leave_since.is_none() {
                        overlay_leave_since = Some(Instant::now());
                    }
                    if overlay_leave_since
                        .map(|t| t.elapsed() >= Duration::from_millis(120))
                        .unwrap_or(false)
                    {
                        crate::overlay::window_manager::hide_overlay_window(&app);
                        overlay_leave_since = None;
                    }
                } else {
                    overlay_leave_since = None;
                }
            } else {
                overlay_leave_since = None;
            }
        }

        // Hover dictionary (MTT-inspired on desktop):
        // - dwell then pick; never while typing (key within 1.5s)
        // - editable focus: skip free-hover (don't block typing)
        // - terminals: OFF free-hover
        // - unit: word | sentence (Alt held forces sentence)
        // - typing/KeyDown dismisses stuck cards
        if !ux.hover_dictionary || crate::selection::mouse_hook::key_pressed_within_ms(1500) {
            hover_still_since = None;
        } else if !left_button_down()
            && !super::pop_button::has_pending()
            && !is_own_window_foreground(&app)
            && crate::overlay::window_manager::overlay_screen_bounds(&app).is_none()
        {
            let fg = super::process_class::foreground_process();
            let hover_skip = fg
                .as_ref()
                .map(|p| {
                    p.is_terminal
                        || p.is_self
                        || matches!(
                            p.strategy(&ux.exclude_processes),
                            super::process_class::SelectionStrategy::Skip
                        )
                })
                .unwrap_or(false);
            let cell = ((cx / 20.0) as i32, (cy / 20.0) as i32);
            if hover_skip {
                hover_still_since = None;
            } else if cell != hover_anchor {
                hover_anchor = cell;
                hover_still_since = Some(Instant::now());
            } else if let Some(since) = hover_still_since {
                let dwell = Duration::from_millis(ux.hover_dwell_ms.max(350) as u64);
                if since.elapsed() >= dwell
                    && last_hover_lookup.elapsed() >= Duration::from_millis(900)
                {
                    last_hover_lookup = Instant::now();
                    // MTT: hide free-hover while caret is in an edit field
                    if super::hover_pick::is_editable_control_focused() {
                        hover_still_since = None;
                        continue;
                    }
                    // Never free-OCR hover on terminals/browsers without TextPattern —
                    // OCR strip reads chrome ("PowerShell", "Google") more often than page words.
                    let ocr_fb = super::ocr_force_allowed(&ux)
                        && fg
                            .as_ref()
                            .map(|p| !p.is_terminal && !p.is_browser)
                            .unwrap_or(true);
                    let unit = ux.hover_unit.to_ascii_lowercase();
                    let alt_sentence = super::modifier_key_satisfied("alt");
                    let want_sentence = unit == "sentence" || unit == "sent" || alt_sentence;
                    let pick = tokio::task::spawn_blocking(move || {
                        super::hover_pick::pick_at_cursor_uia(want_sentence).or_else(|| {
                            if ocr_fb {
                                pick_word_line_strip_ocr()
                            } else {
                                None
                            }
                        })
                    })
                    .await
                    .ok()
                    .flatten();
                    match pick {
                        Some(pick) => {
                            let w = pick.word.trim().to_string();
                            let ok = if want_sentence {
                                w.chars().count() >= 2
                                    && w.chars().count() <= 120
                                    && !is_junk_hover_word(&w)
                                    && w.chars().any(|c| c.is_alphanumeric())
                            } else {
                                dictionary::is_single_word(&w)
                                    && w.chars().count() >= 2
                                    && w.chars().count() <= 28
                                    && !w.contains('\n')
                                    && w.chars().any(|c| c.is_alphanumeric())
                                    && !is_junk_hover_word(&w)
                            };
                            if ok
                                && !hover_dedupe.should_skip(
                                    &w,
                                    pick.x,
                                    pick.y,
                                    Duration::from_secs(3),
                                )
                            {
                                tracing::info!(
                                    "[selection_ux] hover hit: {:?} via {} (sentence={})",
                                    w,
                                    pick.source,
                                    want_sentence
                                );
                                hover_still_since = None;
                                if want_sentence && !dictionary::is_single_word(&w) {
                                    // Sentence → machine translate (not dict card)
                                    show_selection_translate_text(&app, &w).await;
                                } else {
                                    show_hover_dictionary(&app, &w, pick.x, pick.y).await;
                                }
                            }
                        },
                        None => {},
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    super::mouse_hook::uninstall();
    tracing::info!("[selection_ux] watcher stopped");
}

#[cfg(windows)]
async fn handle_hook_event(
    app: &AppHandle,
    config: &Arc<Mutex<SelectionUxConfig>>,
    job_gen: &Arc<AtomicU64>,
    ev: super::mouse_hook::MouseHookEvent,
) {
    use super::mouse_hook::MouseHookEvent;
    match ev {
        MouseHookEvent::MouseDownOnPop => {
            // Use text already captured at gesture time — do NOT re-read selection
            // (moving to click pop often clears terminal/browser selection).
            if let Some(text) = super::pop_button::take_pending() {
                let _ = super::pop_button::dismiss(app);
                job_gen.fetch_add(1, Ordering::SeqCst); // cancel in-flight fetch jobs
                tracing::info!(
                    "[selection_ux] pop confirm ({} chars) text={:?}",
                    text.chars().count(),
                    text.chars().take(40).collect::<String>()
                );
                let app_c = app.clone();
                tauri::async_runtime::spawn(async move {
                    show_selection_translate_text(&app_c, &text).await;
                });
            } else {
                tracing::warn!("[selection_ux] pop click but no pending text");
            }
        },
        MouseHookEvent::MouseDownOutsidePop => {
            job_gen.fetch_add(1, Ordering::SeqCst);
            let _ = super::pop_button::dismiss(app);
        },
        MouseHookEvent::MouseScroll | MouseHookEvent::RightMouseDown => {
            job_gen.fetch_add(1, Ordering::SeqCst);
            let _ = super::pop_button::dismiss(app);
        },
        MouseHookEvent::KeyDown => {
            // Typing: cancel jobs + hide stuck hover/selection cards (terminal spam fix)
            job_gen.fetch_add(1, Ordering::SeqCst);
            crate::overlay::window_manager::hide_overlay_window(app);
            let _ = super::pop_button::dismiss(app);
        },
        MouseHookEvent::SelectionGesture(pt) => {
            let ux = config.lock().await.clone();
            if !matches!(
                ux.trigger_mode,
                SelectionTriggerMode::AutoOnSelect | SelectionTriggerMode::PopButton
            ) {
                return;
            }
            if is_own_window_foreground(app) {
                return;
            }

            let gen = job_gen.fetch_add(1, Ordering::SeqCst) + 1;
            let job_gen_c = Arc::clone(job_gen);
            let app_c = app.clone();
            let mode = ux.trigger_mode.clone();
            let min_chars = ux.auto_min_chars.max(1) as usize;
            let exclude = ux.exclude_processes.clone();
            // Capture modifier at gesture time (before 150ms delay) so user can release after
            let ocr_force = super::ocr_force_allowed(&ux);
            // Same gate as free-hover: never OCR-force on terminal/browser chrome.
            let ocr_force_ok = ocr_force
                && super::process_class::foreground_process()
                    .map(|p| !p.is_terminal && !p.is_browser)
                    .unwrap_or(true);
            let release_x = pt.x as f64;
            let release_y = pt.y as f64;

            // Easydict SelectionDelayMs = 150
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_millis(150)).await;
                if job_gen_c.load(Ordering::SeqCst) != gen {
                    return;
                }
                if is_own_window_foreground(&app_c) {
                    return;
                }

                match try_get_selection_text(&app_c, &exclude).await {
                    Some(text) if text.chars().count() >= min_chars => {
                        if job_gen_c.load(Ordering::SeqCst) != gen {
                            return;
                        }
                        let trimmed = text.trim().to_string();
                        tracing::info!(
                            "[selection_ux] gesture ok mode={:?} chars={}",
                            mode,
                            trimmed.chars().count()
                        );
                        match mode {
                            SelectionTriggerMode::PopButton => {
                                if let Err(e) =
                                    super::pop_button::show(&app_c, trimmed, release_x, release_y)
                                {
                                    tracing::warn!("[selection_ux] pop show: {e}");
                                }
                            },
                            SelectionTriggerMode::AutoOnSelect => {
                                show_selection_translate_text(&app_c, &trimmed).await;
                            },
                            SelectionTriggerMode::HotkeyOnly => {},
                        }
                    },
                    _ if ocr_force_ok => {
                        if job_gen_c.load(Ordering::SeqCst) != gen {
                            return;
                        }
                        if let Some(pick) =
                            tokio::task::spawn_blocking(|| pick_word_near_cursor_ocr(90, 36))
                                .await
                                .ok()
                                .flatten()
                        {
                            match mode {
                                SelectionTriggerMode::PopButton => {
                                    let _ = super::pop_button::show(
                                        &app_c,
                                        pick.word.clone(),
                                        pick.x,
                                        pick.y,
                                    );
                                },
                                _ => {
                                    show_ocr_force_translate(&app_c, &pick.word, pick.x, pick.y)
                                        .await;
                                },
                            }
                        }
                    },
                    _ => {
                        tracing::debug!("[selection_ux] gesture: no selection text");
                    },
                }
            });
        },
    }
}

fn is_junk_hover_word(w: &str) -> bool {
    is_ui_chrome_word(w)
}

async fn show_hover_dictionary(app: &AppHandle, word: &str, x: f64, y: f64) {
    let word = word.trim();
    if word.is_empty() || !dictionary::is_single_word(word) || is_junk_hover_word(word) {
        return;
    }
    if crate::selection::mouse_hook::key_pressed_within_ms(400) {
        return;
    }

    let source = {
        app.try_state::<crate::AppState>()
            .map(|s| {
                s.system
                    .config
                    .blocking_lock()
                    .selection_ux
                    .hover_dict_source
                    .clone()
            })
            .unwrap_or_else(|| "auto".into())
    };
    let source = source.to_ascii_lowercase();
    let use_ecdict = matches!(source.as_str(), "auto" | "ecdict" | "local");
    let use_youdao = matches!(source.as_str(), "auto" | "youdao" | "online");

    let mut results = Vec::new();
    // Local ECDICT first for English when enabled
    if use_ecdict && !dictionary::is_cjk(word) {
        if let Some(state) = app.try_state::<crate::AppState>() {
            if let Some(pool) = state.ecdict_pool.as_ref() {
                if let Ok(body) = lookup_ecdict_overlay(word, pool).await {
                    if dictionary::has_real_meanings(&body) {
                        results = body;
                    }
                }
            }
        }
    }
    if results.is_empty() && use_youdao {
        let dict = dictionary::Dictionary::new();
        results = if dictionary::is_cjk(word) {
            let mut found = Vec::new();
            let chars: Vec<char> = word.chars().collect();
            if chars.len() > 1 {
                for len in (1..=chars.len().min(4)).rev() {
                    for start in 0..=(chars.len() - len) {
                        let sub: String = chars[start..start + len].iter().collect();
                        if let Ok(r) = dict.lookup_chinese(&sub).await {
                            if dictionary::has_real_meanings(&r) {
                                found = r;
                                break;
                            }
                        }
                    }
                    if !found.is_empty() {
                        break;
                    }
                }
            }
            if found.is_empty() {
                dict.lookup_chinese(word).await.unwrap_or_default()
            } else {
                found
            }
        } else {
            dict.lookup(word).await.unwrap_or_default()
        };
    }
    // ecdict-only miss: optional youdao already handled; pure miss → silent
    if results.is_empty() && use_ecdict && !use_youdao && dictionary::is_cjk(word) {
        // CJK has no ECDICT path — fall back youdao once if source was ecdict-only
    }

    // Hover = real dictionary hit only (never MT app titles / OCR junk).
    let Some(body) = format_dict_body(word, &results) else {
        return;
    };
    if crate::selection::mouse_hook::key_pressed_within_ms(400) {
        return;
    }
    let pos = overlay::OverlayPosition::at_cursor(x, y);
    let line_n = body.lines().count().max(1) as f64;
    let h = (64.0 + line_n * 22.0).clamp(80.0, 300.0);
    // CJK ~14–16 CSS px/glyph; ASCII ~8. Mixed text → weight CJK higher.
    let longest = body
        .lines()
        .map(|l| {
            l.chars()
                .map(|c| if c >= '\u{3000}' { 15.0_f64 } else { 8.0 })
                .sum::<f64>()
        })
        .fold(0.0_f64, f64::max)
        .max(96.0);
    let w = (longest + 56.0).clamp(220.0, 460.0);
    let content = overlay::OverlayContent {
        source: String::new(),
        translated: body,
        source_app: Some("hover-dict".into()),
        window_title: None,
    };
    let html = overlay::html_builder::build_html(&content, overlay::OverlayLevel::Minimal, 4500);
    let _ = overlay::window_manager::create_overlay_window_ex(
        app, &html, pos.x, pos.y, w, h, true, false,
    );
}

fn push_ecdict_def(
    meanings: &mut Vec<crate::models::dictionary::Meaning>,
    pos: &str,
    def_text: &str,
) {
    use crate::models::dictionary::{Definition, Meaning};
    let def_text = def_text.trim();
    if def_text.is_empty() {
        return;
    }
    let pos_key = if pos.is_empty() { "" } else { pos };
    if let Some(m) = meanings.iter_mut().find(|m| m.part_of_speech == pos_key) {
        if m.definitions.len() < 6 {
            m.definitions.push(Definition {
                definition: def_text.to_string(),
                example: None,
                synonyms: vec![],
                antonyms: vec![],
            });
        }
        return;
    }
    if meanings.len() >= 6 {
        return;
    }
    meanings.push(Meaning {
        part_of_speech: pos_key.to_string(),
        definitions: vec![Definition {
            definition: def_text.to_string(),
            example: None,
            synonyms: vec![],
            antonyms: vec![],
        }],
    });
}

/// ECDICT → DictionaryResult for hover overlay.
async fn lookup_ecdict_overlay(
    word: &str,
    pool: &sqlx::SqlitePool,
) -> Result<Vec<crate::models::dictionary::DictionaryResult>, String> {
    use crate::models::dictionary::{DictionaryResult, Meaning};
    use sqlx::Row;
    let key = word.trim().to_lowercase();
    // Prefer pos column when present (ECDICT schema); fall back without it.
    let row = match sqlx::query(
        "SELECT word, phonetic, definition, translation, pos FROM stardict WHERE word = ?1 COLLATE NOCASE LIMIT 1",
    )
    .bind(&key)
    .fetch_optional(pool)
    .await
    {
        Ok(r) => r,
        Err(_) => {
            sqlx::query(
                "SELECT word, phonetic, definition, translation FROM stardict WHERE word = ?1 COLLATE NOCASE LIMIT 1",
            )
            .bind(&key)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?
        },
    };
    let Some(row) = row else {
        return Ok(vec![]);
    };
    let head: String = row.try_get("word").unwrap_or_else(|_| word.to_string());
    let phonetic: Option<String> = row.try_get("phonetic").ok().flatten();
    let translation: Option<String> = row.try_get("translation").ok().flatten();
    let definition: Option<String> = row.try_get("definition").ok().flatten();
    let pos_raw: Option<String> = row.try_get("pos").ok().flatten();
    let default_pos = pos_raw
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();
    let mut meanings: Vec<Meaning> = Vec::new();
    // ECDICT translation lines often look like "n. 名词" / "v. 动词"
    if let Some(tr) = translation {
        for line in tr.split(['\n', '\\']) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let (pos, def) = if let Some((p, rest)) = line.split_once('.') {
                let p = p.trim();
                if p.len() <= 6 && p.chars().all(|c| c.is_ascii_alphabetic()) {
                    (p, rest.trim())
                } else {
                    (default_pos.as_str(), line)
                }
            } else {
                (default_pos.as_str(), line)
            };
            push_ecdict_def(&mut meanings, pos, def);
        }
    }
    if meanings.is_empty() {
        if let Some(def) = definition {
            for line in def.split('\n').take(4) {
                push_ecdict_def(&mut meanings, default_pos.as_str(), line);
            }
        }
    }
    if meanings.is_empty() {
        return Ok(vec![]);
    }
    Ok(vec![DictionaryResult {
        word: head,
        phonetic: phonetic.filter(|p| !p.is_empty()).map(|p| {
            if p.starts_with('/') || p.starts_with('[') {
                p
            } else {
                format!("/{}/", p)
            }
        }),
        meanings,
        source_urls: vec![],
    }])
}

/// Public entry for dictionary hotkey / external callers (dict-first for single words).
pub async fn show_selection_translate_text_public(app: &AppHandle, text: &str) {
    show_selection_translate_text(app, text).await;
}

async fn show_selection_translate_text(app: &AppHandle, text: &str) {
    let Some(state) = app.try_state::<crate::AppState>() else {
        return;
    };
    let (from, to, _level, dismiss) = {
        let c = state.system.config.lock().await;
        (
            c.default_from.clone(),
            c.default_to.clone(),
            c.overlay_level,
            c.overlay_auto_dismiss_ms,
        )
    };
    let trimmed = text.trim();
    // Single word: real dict hit only; miss / OCR garbage (e.g. "repare") → machine translate
    if dictionary::is_single_word(trimmed) && trimmed.chars().count() <= 32 {
        let dict = dictionary::Dictionary::new();
        let results = if dictionary::is_cjk(trimmed) {
            dict.lookup_chinese(trimmed).await.unwrap_or_default()
        } else {
            dict.lookup(trimmed).await.unwrap_or_default()
        };
        if let Some(body) = format_dict_body(trimmed, &results) {
            let (cx, cy) = cursor_pos();
            let pos = overlay::OverlayPosition::at_cursor(cx, cy);
            let line_n = body.lines().count().max(2) as f64;
            let h = (56.0 + line_n * 22.0).clamp(80.0, 200.0);
            let content = overlay::OverlayContent {
                source: trimmed.to_string(),
                translated: body,
                source_app: Some("dict".into()),
                window_title: None,
            };
            let html = overlay::html_builder::build_html(
                &content,
                overlay::OverlayLevel::Minimal,
                dismiss.max(3000),
            );
            let _ = overlay::window_manager::create_overlay_window_ex(
                app, &html, pos.x, pos.y, 300.0, h, true, false,
            );
            return;
        }
        // no real meanings → fall through to MT below
    }

    match state
        .translation
        .service
        .run_full(
            crate::models::translation::TranslateChannel::Selection,
            text,
            &from,
            &to,
        )
        .await
    {
        Ok(resp) => {
            let display = {
                let joined = resp.display_text();
                if joined.is_empty() {
                    format!("（无翻译结果）\n{}", text)
                } else {
                    joined
                }
            };
            let (cx, cy) = cursor_pos();
            let pos = overlay::OverlayPosition::at_cursor(cx, cy);
            let line_n = display.lines().count().max(1) as f64;
            let h = (56.0 + line_n * 22.0).clamp(72.0, 320.0);
            let w = (display
                .lines()
                .map(|l| l.chars().count())
                .max()
                .unwrap_or(16) as f64
                * 8.0
                + 48.0)
                .clamp(200.0, 460.0);
            let content = overlay::OverlayContent {
                source: text.to_string(),
                translated: display,
                source_app: Some("selection".into()),
                window_title: None,
            };
            let html = overlay::html_builder::build_html(
                &content,
                overlay::OverlayLevel::Standard,
                dismiss.max(5000),
            );
            let _ = overlay::window_manager::create_overlay_window_ex(
                app, &html, pos.x, pos.y, w, h, true, false,
            );
        },
        Err(e) => {
            tracing::warn!("[selection_ux] translate failed: {e}");
            let (cx, cy) = cursor_pos();
            let pos = overlay::OverlayPosition::at_cursor(cx, cy);
            let content = overlay::OverlayContent {
                source: text.to_string(),
                translated: format!("翻译失败：{e}"),
                source_app: Some("selection".into()),
                window_title: None,
            };
            let html =
                overlay::html_builder::build_html(&content, overlay::OverlayLevel::Minimal, 4000);
            let _ = overlay::window_manager::create_overlay_window_ex(
                app, &html, pos.x, pos.y, 320.0, 120.0, true, false,
            );
        },
    }
}

async fn show_ocr_force_translate(app: &AppHandle, text: &str, x: f64, y: f64) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    if dictionary::is_single_word(text) {
        show_hover_dictionary(app, text, x, y).await;
        return;
    }
    let Some(state) = app.try_state::<crate::AppState>() else {
        return;
    };
    let (from, to) = {
        let c = state.system.config.lock().await;
        (c.default_from.clone(), c.default_to.clone())
    };
    match state
        .translation
        .service
        .run_full(
            crate::models::translation::TranslateChannel::Selection,
            text,
            &from,
            &to,
        )
        .await
    {
        Ok(resp) => {
            let translated = resp.display_text();
            if translated.is_empty() {
                return;
            }
            let pos = overlay::OverlayPosition::at_cursor(x, y);
            let line_n = translated.lines().count().max(1) as f64;
            let h = (56.0 + line_n * 22.0).clamp(72.0, 320.0);
            let w = (translated
                .lines()
                .map(|l| l.chars().count())
                .max()
                .unwrap_or(16) as f64
                * 8.0
                + 48.0)
                .clamp(200.0, 460.0);
            let content = overlay::OverlayContent {
                source: text.to_string(),
                translated,
                source_app: Some("ocr-force".into()),
                window_title: None,
            };
            let html =
                overlay::html_builder::build_html(&content, overlay::OverlayLevel::Standard, 5000);
            let _ = overlay::window_manager::create_overlay_window_ex(
                app, &html, pos.x, pos.y, w, h, true, false,
            );
        },
        Err(e) => tracing::warn!("[selection_ux] OCR force translate failed: {}", e),
    }
}

fn left_button_down() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
        unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000 != 0 }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn cursor_pos() -> (f64, f64) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut pt = POINT::default();
        unsafe {
            if GetCursorPos(&mut pt).is_ok() {
                return (pt.x as f64, pt.y as f64);
            }
        }
        (0.0, 0.0)
    }
    #[cfg(not(windows))]
    {
        (0.0, 0.0)
    }
}

fn is_own_window_foreground(app: &AppHandle) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        let fg = unsafe { GetForegroundWindow() };
        let fg_raw = fg.0 as isize;
        if fg_raw == 0 {
            return false;
        }
        for label in [
            "main",
            "ocr-region-frame",
            "ocr-selector",
            "overlay",
            "selection-pop",
        ] {
            if let Some(w) = app.get_webview_window(label) {
                if let Ok(hwnd) = w.hwnd() {
                    if hwnd.0 as isize == fg_raw {
                        return true;
                    }
                }
            }
        }
        false
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        false
    }
}

async fn try_get_selection_text(app: &AppHandle, exclude: &[String]) -> Option<String> {
    let state = app.try_state::<crate::AppState>()?;
    let result = state
        .system
        .selection_manager
        .get_selection_routed(exclude)
        .await?;
    let t = result.text.trim().to_string();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}
