//! Selection UX watcher — Easydict-style:
//! WH_MOUSE_LL gestures → delay 150ms → get selection → pop button / translate
//! Hover dictionary (Alt+dwell) remains polled lightly.

use super::hover_pick::{
    is_ui_chrome_word, pick_word_line_strip_ocr, pick_word_near_cursor_ocr, HoverDedupe,
};
use super::present;
use crate::config::{SelectionTriggerMode, SelectionUxConfig};
use crate::dictionary;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

static WATCHER_RUNNING: AtomicBool = AtomicBool::new(false);

/// Monotonic ms timestamp of the last mouse-up selection gesture (drag /
/// double-click). Hover is suppressed for a short window after a selection so
/// the pop/translation card wins over a coincident hover on the same word.
static LAST_SELECTION_MS: AtomicU64 = AtomicU64::new(0);

pub fn note_selection_gesture() {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    LAST_SELECTION_MS.store(ms, Ordering::SeqCst);
}

/// True if a selection gesture (drag / double-click / pop click) happened
/// within the last `ms` milliseconds. Used to prioritize 划词 over hover.
pub fn selection_gesture_within_ms(ms: u64) -> bool {
    use std::time::{SystemTime, UNIX_EPOCH};
    let last = LAST_SELECTION_MS.load(Ordering::SeqCst);
    if last == 0 {
        return false;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    now.saturating_sub(last) < ms
}

pub struct SelectionAutoWatch {
    config: Arc<Mutex<SelectionUxConfig>>,
    stop: Arc<AtomicBool>,
    /// S2-2: stored so `stop_and_wait` can await the run_loop task instead of
    /// fire-and-forget. Previously the JoinHandle was discarded, making it
    /// impossible to know when the watcher actually stopped.
    ///
    /// Note: we use `tauri::async_runtime::JoinHandle` (not `tokio::task::JoinHandle`)
    /// because `start()` spawns via `tauri::async_runtime::spawn`, which returns
    /// the former. The two are distinct types — mixing them is a compile error.
    task_handle: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
}

impl SelectionAutoWatch {
    pub fn new(config: SelectionUxConfig) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            stop: Arc::new(AtomicBool::new(false)),
            task_handle: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn update_config(self: &Arc<Self>, config: SelectionUxConfig, app: &AppHandle) {
        // P0 fix (Fix 1): detect need_hook changes and restart the watcher.
        // Without this, switching from HotkeyOnly+hover-off to
        // PopButton/hover-on at runtime leaves the mouse hook in the wrong
        // state — drag/double-click/pop-click/KeyDown all silently fail until
        // app restart.
        let old_need = {
            let old = self.config.lock().await;
            old.hover_dictionary
                || matches!(
                    old.trigger_mode,
                    SelectionTriggerMode::AutoOnSelect | SelectionTriggerMode::PopButton
                )
                || old.ocr_force_pickup
        };
        let new_need = config.hover_dictionary
            || matches!(
                config.trigger_mode,
                SelectionTriggerMode::AutoOnSelect | SelectionTriggerMode::PopButton
            )
            || config.ocr_force_pickup;
        #[cfg(windows)]
        super::mouse_hook::set_min_drag_px(config.min_drag_px);
        *self.config.lock().await = config;
        if old_need != new_need {
            tracing::info!(
                "[selection_ux] need_hook changed {}→{}, restarting watcher",
                old_need,
                new_need
            );
            self.stop_and_wait().await;
            self.start(app.clone());
        }
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
        let handle = tauri::async_runtime::spawn(async move {
            run_loop(app, cfg, stop).await;
            WATCHER_RUNNING.store(false, Ordering::SeqCst);
        });
        // S2-2: store the JoinHandle so stop_and_wait can await it.
        if let Ok(mut slot) = self.task_handle.try_lock() {
            *slot = Some(handle);
        }
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// S2-2: request stop and await the watcher task with a 500ms timeout.
    /// If the task doesn't finish in time, abort it so the tokio runtime
    /// can reclaim the resource. The moon-hook-bridge std::thread exits on
    /// its own when the mouse hook channel closes (uninstall is called at
    /// the end of run_loop), so it does not need explicit joining.
    pub async fn stop_and_wait(&self) {
        self.stop.store(true, Ordering::SeqCst);
        let handle = { self.task_handle.lock().await.take() };
        if let Some(h) = handle {
            // Grace period: let run_loop observe `stop` and exit on its own.
            for _ in 0..50 {
                if h.inner().is_finished() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            if !h.inner().is_finished() {
                tracing::warn!("[selection_ux] run_loop did not exit within 500ms, aborting");
                // P0 fix: abort the stale task and force-reset WATCHER_RUNNING.
                // Without this, WATCHER_RUNNING stays true → next start() silently
                // fails → watcher permanently dead after a config change.
                h.abort();
                let _ = h.await;
            }
            WATCHER_RUNNING.store(false, Ordering::SeqCst);
        }
    }
}

async fn run_loop(app: AppHandle, config: Arc<Mutex<SelectionUxConfig>>, stop: Arc<AtomicBool>) {
    let job_gen = Arc::new(AtomicU64::new(0));

    // --- Easydict: WH_MOUSE_LL only when gestures/hover need it ---
    // HotkeyOnly + hover off → skip global LL hooks (less mouse latency).
    #[cfg(windows)]
    let mut async_rx = {
        let (need_hook, px) = {
            let ux = config.lock().await;
            let need = ux.hover_dictionary
                || matches!(
                    ux.trigger_mode,
                    SelectionTriggerMode::AutoOnSelect | SelectionTriggerMode::PopButton
                )
                || ux.ocr_force_pickup;
            (need, ux.min_drag_px)
        };
        super::mouse_hook::set_min_drag_px(px);
        let (async_tx, async_rx) =
            tokio::sync::mpsc::unbounded_channel::<super::mouse_hook::MouseHookEvent>();
        if need_hook {
            let hook_rx = super::mouse_hook::install();
            if let Some(rx) = hook_rx {
                tracing::info!("[selection_ux] mouse hook active");
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
            } else {
                tracing::warn!("[selection_ux] mouse hook unavailable — gesture path degraded");
            }
        } else {
            tracing::info!(
                "[selection_ux] skip WH_MOUSE_LL (hotkey-only / hover off / no ocr-force)"
            );
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
    // QTranslate: mouse-leave debounce before hide (~500ms)
    let mut overlay_leave_since: Option<Instant> = None;
    // Same debounce for the translate card (separate timer so the overlay block
    // below doesn't keep resetting it on its own `else` branch).
    let mut card_leave_since: Option<Instant> = None;

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

        // Mouse-leave dismiss overlay (QTranslate debounce — no flicker)
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
                        .map(|t| t.elapsed() >= Duration::from_millis(500))
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

        // Translate-card mouse-leave dismiss — hover/dict (no-focus) cards only.
        // User-initiated cards close on window blur instead (FE), so this block
        // is skipped when the last card was shown with focus.
        if !crate::overlay::translate_card::translate_card_no_focus_mode() {
            card_leave_since = None;
        } else if crate::overlay::translate_card::translate_card_shown_within_ms(800) {
            // No-flicker guard (QTranslate behavior, mirrors the legacy overlay):
            // never start a leave-timer within the first 800ms after a show.
            card_leave_since = None;
        } else if let Some((ox, oy, ow, oh)) =
            crate::overlay::translate_card::translate_card_screen_bounds(&app)
        {
            let plausible = ow >= 40.0 && oh >= 20.0 && (ox > 2.0 || oy > 2.0 || ow > 100.0);
            let inside = cx >= ox - 12.0
                && cx <= ox + ow + 12.0
                && cy >= oy - 12.0
                && cy <= oy + oh + 12.0;
            // The hovered word that spawned this card also keeps it alive:
            // parking the cursor on the word (the card sits just below it) must
            // not dismiss the card (flicker / frequent disappear-and-reappear).
            let inside = inside
                || crate::overlay::translate_card::translate_card_keep_alive_hit(cx, cy);
            let on_pop = super::pop_button::hit_test(&app, cx, cy);
            if plausible && !inside && !on_pop {
                if card_leave_since.is_none() {
                    card_leave_since = Some(Instant::now());
                }
                if card_leave_since
                    .map(|t| t.elapsed() >= Duration::from_millis(500))
                    .unwrap_or(false)
                {
                    crate::overlay::translate_card::hide_translate_card(&app);
                    card_leave_since = None;
                }
            } else {
                card_leave_since = None;
            }
        } else {
            card_leave_since = None;
        }

        // Hover dictionary (MTT-inspired on desktop):
        // - dwell then pick; never while typing (key within 500ms — 1.5s was
        //   too long, users hover another word right after typing one)
        // - editable focus: skip free-hover (don't block typing)
        // - terminals: OFF free-hover
        // - unit: word | sentence (Alt held forces sentence)
        // - typing/KeyDown dismisses stuck cards
        // 划词优先: while the left button is down (drag-select in progress) or
        // shortly after a selection gesture, hover must NOT fire — the dwell
        // timer is also reset so a stale dwell (armed before the drag) can't
        // trigger on release.
        if !ux.hover_dictionary
            || crate::selection::mouse_hook::key_pressed_within_ms(500)
            || left_button_down()
            || selection_gesture_within_ms(3000)
        {
            hover_still_since = None;
        } else if !super::pop_button::has_pending()
            && !is_own_window_foreground(&app)
            && crate::overlay::window_manager::overlay_screen_bounds(&app).is_none()
            // Hover must not fire over the translate card itself (its own DOM /
            // OCR would re-trigger a card on the card) — Bug: hover dict firing
            // on the selection-translate page. Also back off briefly after a
            // manual close so the card doesn't instantly reappear under the
            // still-hovering cursor.
            && !crate::overlay::translate_card::translate_card_is_visible(&app)
            && !crate::overlay::translate_card::translate_card_closed_within_ms(1200)
            // After a manual close, the card is latched shut around the cursor
            // position until the user moves away — otherwise clicking X on a
            // hover card just has the card pop back a moment later.
            && !crate::overlay::translate_card::translate_card_close_latch_hit(cx, cy)
            // 划词优先: after a selection gesture, the pop/translation owns the
            // interaction — don't also fire hover on the same word.
            && !selection_gesture_within_ms(3000)
        {
            let fg = super::process_class::foreground_process();
            let hover_skip = fg
                .as_ref()
                .map(|p| {
                    // Allow hover on terminals: WindowsTerminal exposes UIA
                    // TextPattern->RangeFromPoint, so a real word under the
                    // cursor can be read. Only self and excluded processes skip.
                    p.is_self
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
                // P0: removed global 900ms cooldown — HoverDedupe already prevents
                // same-word/cell repeat (3s). The global cooldown blocked legitimate
                // quick word-to-word hover transitions.
                if since.elapsed() >= dwell {
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
                            // CJK hover is opt-in (hover_cjk). Default off: hovering
                            // Chinese text misfires on chrome and triggers the slow
                            // LLM fallback, so skip unless the user enabled it.
                            let cjk_ok = ux.hover_cjk || !dictionary::is_cjk(&w);
                            let ok = if want_sentence {
                                w.chars().count() >= 2
                                    && w.chars().count() <= 120
                                    && !is_junk_hover_word(&w)
                                    && w.chars().any(|c| c.is_alphanumeric())
                                    && cjk_ok
                            } else {
                                dictionary::is_single_word(&w)
                                    && w.chars().count() >= 2
                                    && w.chars().count() <= 28
                                    && !w.contains('\n')
                                    && w.chars().any(|c| c.is_alphanumeric())
                                    && !is_junk_hover_word(&w)
                                    && cjk_ok
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
                                    present::present_selection(&app, &w, pick.bounds.as_ref(), false)
                                        .await;
                                } else {
                                    present::present_hover_dictionary(
                                        &app,
                                        &w,
                                        pick.x,
                                        pick.y,
                                        pick.bounds.as_ref(),
                                    )
                                    .await;
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
            note_selection_gesture();
            // Use text already captured at gesture time — do NOT re-read selection
            // (moving to click pop often clears terminal/browser selection).
            // R2: only trust pending captured at pop show — never re-GetSelection.
            if let Some(text) = super::pop_button::take_pending() {
                let _ = super::pop_button::dismiss(app);
                job_gen.fetch_add(1, Ordering::SeqCst); // cancel in-flight fetch jobs
                let app_c = app.clone();
                tauri::async_runtime::spawn(async move {
                    present::present_selection(&app_c, &text, None, true).await;
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
            crate::overlay::translate_card::hide_translate_card(app);
            let _ = super::pop_button::dismiss(app);
        },
        MouseHookEvent::SelectionGesture(pt) => {
            tracing::info!("[selection_ux] SelectionGesture at ({},{})", pt.x, pt.y);
            note_selection_gesture();
            let ux = config.lock().await.clone();
            if !matches!(
                ux.trigger_mode,
                SelectionTriggerMode::AutoOnSelect | SelectionTriggerMode::PopButton
            ) {
                tracing::debug!(
                    "[selection_ux] gesture dropped: trigger_mode={:?}",
                    ux.trigger_mode
                );
                return;
            }
            if is_own_window_foreground(app) {
                tracing::debug!("[selection_ux] gesture dropped: own window foreground");
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
                    tracing::debug!("[selection_ux] gesture job {} superseded", gen);
                    return;
                }
                if is_own_window_foreground(&app_c) {
                    tracing::debug!(
                        "[selection_ux] gesture dropped: own window foreground (at trigger)"
                    );
                    return;
                }

                match try_get_selection_text(&app_c, &exclude).await {
                    Some((text, bounds)) if text.chars().count() >= min_chars => {
                        if job_gen_c.load(Ordering::SeqCst) != gen {
                            return;
                        }
                        let trimmed = text.trim().to_string();
                        // R2: junk / chrome never show pop or MT.
                        if !present::accept_for_pop(&trimmed) {
                            tracing::info!(
                                "[pop] pending_len={} preview={:?} route=reject",
                                trimmed.chars().count(),
                                trimmed.chars().take(40).collect::<String>()
                            );
                            return;
                        }
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
                                // P1 fix (Fix 6): if Ctrl is held at trigger time,
                                // user likely wants to copy (Ctrl+C) — skip to
                                // avoid translating every Ctrl-select/copy.
                                if super::modifier_key_satisfied("ctrl") {
                                    tracing::debug!(
                                        "[selection_ux] auto-on-select skipped (Ctrl held — copy intent)"
                                    );
                                    return;
                                }
                                present::present_selection(&app_c, &trimmed, bounds.as_ref(), true)
                                    .await;
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
                            if !present::accept_for_pop(&pick.word) {
                                return;
                            }
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
                                    present::present_selection(
                                        &app_c,
                                        &pick.word,
                                        pick.bounds.as_ref(),
                                        true,
                                    )
                                    .await;
                                },
                            }
                        }
                    },
                    _ => {
                        tracing::info!("[selection_ux] gesture: no selection text");
                    },
                }
            });
        },
    }
}

fn is_junk_hover_word(w: &str) -> bool {
    is_ui_chrome_word(w)
        || super::hover_pick::looks_like_app_or_process_name(w)
        || looks_like_measurement(w)
}

/// "300ms", "1.5s", "1080p", "4k", "100%", "3.5gb" — numeric tokens with a short
/// unit suffix. Hovering these (latency readouts, resolutions, sizes) just
/// spawns junk translate cards; skip them.
fn looks_like_measurement(w: &str) -> bool {
    let t = w.trim();
    let bytes = t.as_bytes();
    let mut i = 0usize;
    let mut digit_count = 0usize;
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
        if bytes[i].is_ascii_digit() {
            digit_count += 1;
        }
        i += 1;
    }
    if digit_count == 0 || i == 0 || i == bytes.len() {
        return false;
    }
    let rest = &t[i..];
    rest.len() <= 4
        && rest
            .chars()
            .all(|c| c.is_ascii_alphabetic() || matches!(c, '%' | 'x' | 'X'))
}

fn left_button_down() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
        // SAFETY: GetAsyncKeyState is a pure Win32 query (i32 vk → i16) with
        // no preconditions; the high bit of the return indicates current state.
        unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000 != 0 }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn cursor_pos() -> (f64, f64) {
    // S1-6: delegate to the shared crate::win::cursor_pos() instead of a
    // local GetCursorPos wrapper.
    crate::win::cursor_pos()
}

fn is_own_window_foreground(app: &AppHandle) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        // SAFETY: GetForegroundWindow is a pure Win32 query (no args, returns
        // HWND). Returned handle is only compared against stored values.
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

async fn try_get_selection_text(
    app: &AppHandle,
    exclude: &[String],
) -> Option<(String, Option<crate::selection::SelectionBounds>)> {
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
        Some((t, result.bounds))
    }
}
