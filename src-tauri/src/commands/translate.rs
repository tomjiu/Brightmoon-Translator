use crate::capabilities::input_replacement::ReplacementResult;
use crate::dictionary::{self, DictionaryResult};
use crate::engine::TranslateResponse;
use crate::error::AppError;
use crate::lang_detect::{self, DetectionResult};
use crate::security;
use crate::AppState;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use tauri::{Emitter, State};

// Shared clipboard monitoring state (event-driven; not a poll stub)
static CLIPBOARD_MONITORING: AtomicBool = AtomicBool::new(false);
/// Message-loop thread id for clean WM_QUIT shutdown (0 = none)
static CLIPBOARD_LISTENER_TID: AtomicU32 = AtomicU32::new(0);
/// Last emitted clipboard text for dedupe across restarts of the listener
static LAST_CLIPBOARD_TEXT: Mutex<String> = Mutex::new(String::new());

#[derive(Deserialize)]
pub struct TranslateRequest {
    pub text: String,
    pub from: String,
    pub to: String,
    /// Optional product channel: "ui" | "ocr" | "selection" | … (default ui)
    #[serde(default)]
    pub channel: Option<String>,
}

fn parse_channel(raw: Option<&str>) -> crate::models::translation::TranslateChannel {
    use crate::models::translation::TranslateChannel;
    match raw.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("ocr") => TranslateChannel::Ocr,
        Some("selection") => TranslateChannel::Selection,
        Some("replace") => TranslateChannel::Replace,
        Some("hook") => TranslateChannel::Hook,
        Some("clipboard") => TranslateChannel::Clipboard,
        Some("document") => TranslateChannel::Document,
        Some("subtitle") => TranslateChannel::Subtitle,
        Some("image") => TranslateChannel::Image,
        Some("http") => TranslateChannel::Http,
        Some("browser") => TranslateChannel::Browser,
        Some("plugin") => TranslateChannel::Plugin,
        _ => TranslateChannel::Ui,
    }
}

#[tauri::command]
pub async fn translate(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: TranslateRequest,
) -> Result<TranslateResponse, AppError> {
    // Validate inputs
    security::validate_text_length(&request.text, security::MAX_TRANSLATION_TEXT_LENGTH)?;
    security::validate_language_code(&request.from)?;
    security::validate_language_code(&request.to)?;

    let channel = parse_channel(request.channel.as_deref());
    let response = state
        .translation
        .service
        .run_full(channel, &request.text, &request.from, &request.to)
        .await?;

    // Auto-copy result if enabled
    let config = state.system.config.lock().await;
    if config.auto_copy_result {
        if let Some(first) = response.results.first() {
            let copy_text = match config.auto_copy_mode.as_str() {
                "source" => request.text.clone(),
                "both" => format!("{}\n{}", request.text, first.text),
                _ => first.text.clone(), // "translated" or default
            };
            let _ = app.emit("auto-copy", &copy_text);
        }
    }

    Ok(response)
}

#[tauri::command]
pub async fn translate_stream(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: TranslateRequest,
) -> Result<String, AppError> {
    // Validate inputs
    security::validate_text_length(&request.text, security::MAX_TRANSLATION_TEXT_LENGTH)?;
    security::validate_language_code(&request.from)?;
    security::validate_language_code(&request.to)?;

    // Create channel for streaming tokens
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);

    // Spawn task to forward tokens to Tauri event
    let app_handle = app.clone();
    let forward_handle = tokio::spawn(async move {
        let mut full_text = String::new();
        while let Some(chunk) = rx.recv().await {
            full_text.push_str(&chunk);
            let _ = app_handle.emit(
                "stream-chunk",
                serde_json::json!({
                    "chunk": chunk,
                    "done": false,
                }),
            );
        }
        // Emit completion
        let _ = app_handle.emit(
            "stream-chunk",
            serde_json::json!({
                "chunk": "",
                "done": true,
            }),
        );
        full_text
    });

    // Stream translation using TranslationService
    let result = state
        .translation
        .service
        .translate_stream(&request.text, &request.from, &request.to, tx)
        .await;

    // Wait for forwarding to complete
    let _full_text = forward_handle.await?;

    result.map_err(AppError::from)
}

/// Start main-window clipboard monitor (event-driven on Windows).
/// Emits `clipboard-changed` with the new text; FE hydrates MainTranslator + translates.
#[tauri::command]
pub async fn start_clipboard_monitor(
    app: tauri::AppHandle,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    match CLIPBOARD_MONITORING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst) {
        Ok(false) => {},
        _ => return Ok(()),
    }

    let app_handle = app.clone();

    #[cfg(target_os = "windows")]
    {
        std::thread::spawn(move || {
            main_clipboard_listener_thread(app_handle);
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Non-Windows: no native format listener; keep flag true so stop is honest,
        // but do not claim support via a fake poll loop.
        CLIPBOARD_MONITORING.store(false, Ordering::Relaxed);
        return Err("Clipboard monitor is only supported on Windows".to_string());
    }

    Ok(())
}

#[tauri::command]
pub async fn stop_clipboard_monitor() -> Result<(), String> {
    CLIPBOARD_MONITORING.store(false, Ordering::Relaxed);

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};

        let tid = CLIPBOARD_LISTENER_TID.swap(0, Ordering::SeqCst);
        if tid != 0 {
            // SAFETY: tid was registered by the listener thread; WM_QUIT ends GetMessageW.
            unsafe {
                let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn main_clipboard_listener_thread(app: tauri::AppHandle) {
    use std::time::Duration;
    use windows::Win32::System::DataExchange::{
        AddClipboardFormatListener, RemoveClipboardFormatListener,
    };
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::*;

    // SAFETY: dedicated thread + message-only window for WM_CLIPBOARDUPDATE.
    unsafe {
        let tid = GetCurrentThreadId();
        CLIPBOARD_LISTENER_TID.store(tid, Ordering::SeqCst);

        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            windows::core::w!("STATIC"),
            windows::core::w!("MoonMainClipListener"),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            None,
            None,
            None,
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("[clipboard-monitor] CreateWindowExW failed: {}", e);
                CLIPBOARD_MONITORING.store(false, Ordering::Relaxed);
                CLIPBOARD_LISTENER_TID.store(0, Ordering::SeqCst);
                return;
            },
        };

        if AddClipboardFormatListener(hwnd).is_err() {
            tracing::error!("[clipboard-monitor] AddClipboardFormatListener failed");
            let _ = DestroyWindow(hwnd);
            CLIPBOARD_MONITORING.store(false, Ordering::Relaxed);
            CLIPBOARD_LISTENER_TID.store(0, Ordering::SeqCst);
            return;
        }

        tracing::info!("[clipboard-monitor] event-driven listener started");

        let mut msg = MSG::default();
        loop {
            if !CLIPBOARD_MONITORING.load(Ordering::Relaxed) {
                break;
            }

            let result = GetMessageW(&mut msg, None, 0, 0);
            if !result.as_bool() {
                break;
            }

            if msg.message == WM_CLIPBOARDUPDATE {
                // Short settle so writers finish OpenClipboard (STranslate-style)
                std::thread::sleep(Duration::from_millis(50));
                if let Some(text) = read_clipboard_unicode_text() {
                    let trimmed = text.trim().to_string();
                    if trimmed.len() >= 2 && crate::clipboard_dedupe::claim_clipboard_text(&trimmed)
                    {
                        let mut last = LAST_CLIPBOARD_TEXT
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        *last = trimmed.clone();
                        drop(last);
                        let _ = app.emit("clipboard-changed", &trimmed);
                    }
                }
            }

            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = RemoveClipboardFormatListener(hwnd);
        let _ = DestroyWindow(hwnd);
        CLIPBOARD_LISTENER_TID.store(0, Ordering::SeqCst);
        CLIPBOARD_MONITORING.store(false, Ordering::Relaxed);
        tracing::info!("[clipboard-monitor] listener stopped");
    }
}

/// Read CF_UNICODETEXT from the system clipboard.
#[cfg(target_os = "windows")]
fn read_clipboard_unicode_text() -> Option<String> {
    unsafe {
        use windows::Win32::Foundation::HGLOBAL;
        use windows::Win32::System::DataExchange::{
            CloseClipboard, GetClipboardData, OpenClipboard,
        };
        use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

        const CF_UNICODETEXT: u32 = 13;

        // Brief retry if another app still holds the clipboard
        for _ in 0..5 {
            if OpenClipboard(None).is_ok() {
                let result = (|| -> Option<String> {
                    let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
                    let h_global = HGLOBAL(handle.0);
                    let p_data = GlobalLock(h_global);
                    if p_data.is_null() {
                        return None;
                    }
                    let size = GlobalSize(h_global);
                    if size <= 2 {
                        let _ = GlobalUnlock(h_global);
                        return None;
                    }
                    let slice = std::slice::from_raw_parts(p_data as *const u16, size / 2);
                    let text = String::from_utf16_lossy(slice);
                    let text = text.trim_end_matches('\0').to_string();
                    let _ = GlobalUnlock(h_global);
                    Some(text)
                })();
                let _ = CloseClipboard();
                return result;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        None
    }
}

#[tauri::command]
pub async fn translate_selection_with_text(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> Result<(), AppError> {
    if text.trim().is_empty() {
        return Err(AppError::EmptyText);
    }

    // Get config
    let config = state.system.config.lock().await;
    let from = config.default_from.clone();
    let to = config.default_to.clone();
    drop(config);

    let response = state
        .translation
        .service
        .run_full(
            crate::models::translation::TranslateChannel::Selection,
            &text,
            &from,
            &to,
        )
        .await?;

    if let Some(first) = response.results.first() {
        // Emit result to frontend for overlay display
        let _ = app.emit(
            "selection-translated",
            serde_json::json!({
                "source": text,
                "translated": first.text,
                "engine": first.engine,
            }),
        );
    }

    Ok(())
}

/// Get selected text via SelectionProviderManager, translate, and replace in foreground app.
/// Uses the InputReplacement capability: selection → translate → clipboard paste or type.
/// No frontend clipboard read needed — the capability handles everything.
#[tauri::command]
pub async fn replace_translate(state: State<'_, AppState>) -> Result<ReplacementResult, AppError> {
    let config = state.system.config.lock().await;
    let from = config.default_from.clone();
    let to = config.default_to.clone();
    let use_clipboard_output = config.use_clipboard_output;
    drop(config);

    let cap = state.input_replacement.get().ok_or_else(|| {
        AppError::Internal("InputReplacement capability not initialized".to_string())
    })?;

    let result = cap
        .replace_translate(&from, &to, use_clipboard_output)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(result)
}

/// Replace text in the foreground application via the InputReplacement capability.
#[tauri::command]
pub async fn replace_text_in_app(state: State<'_, AppState>, text: String) -> Result<(), AppError> {
    let use_clipboard_output = {
        let config = state.system.config.lock().await;
        config.use_clipboard_output
    };

    let cap = state.input_replacement.get().ok_or_else(|| {
        AppError::Internal("InputReplacement capability not initialized".to_string())
    })?;

    cap.replace_text(&text, use_clipboard_output)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn back_translate(
    state: State<'_, AppState>,
    text: String,
    from: String,
    to: String,
) -> Result<String, AppError> {
    security::validate_text_length(&text, security::MAX_TRANSLATION_TEXT_LENGTH)?;
    security::validate_language_code(&from)?;
    security::validate_language_code(&to)?;

    if text.trim().is_empty() {
        return Err(AppError::EmptyText);
    }

    // Translate back: swap from and to languages
    let result = state
        .translation
        .service
        .run_primary(
            crate::models::translation::TranslateChannel::Ui,
            &text,
            &to,
            &from,
        )
        .await?;
    Ok(result)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedLine {
    pub line_number: usize,
    pub original: String,
    pub translated: String,
}

#[tauri::command]
pub async fn translate_embedded(
    state: State<'_, AppState>,
    text: String,
    from: String,
    to: String,
    // Optional channel: "ocr" | "ui" | … (default ui for main embedded translator)
    channel: Option<String>,
) -> Result<Vec<EmbeddedLine>, AppError> {
    security::validate_text_length(&text, security::MAX_TRANSLATION_TEXT_LENGTH)?;
    security::validate_language_code(&from)?;
    security::validate_language_code(&to)?;

    if text.trim().is_empty() {
        return Ok(vec![]);
    }

    let lines: Vec<(usize, &str)> = text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| (i, l.trim()))
        .collect();
    let ch = match channel
        .as_deref()
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("ocr") => crate::models::translation::TranslateChannel::Ocr,
        Some("selection") => crate::models::translation::TranslateChannel::Selection,
        Some("document") => crate::models::translation::TranslateChannel::Document,
        Some("subtitle") => crate::models::translation::TranslateChannel::Subtitle,
        _ => crate::models::translation::TranslateChannel::Ui,
    };
    let batch_results = state
        .translation
        .service
        .run_batch(ch, &lines, &from, &to, 3)
        .await;

    // Convert to EmbeddedLine format
    let results = batch_results
        .into_iter()
        .map(|r| EmbeddedLine {
            line_number: r.index + 1,
            original: r.original,
            translated: r.translated,
        })
        .collect();

    Ok(results)
}

#[tauri::command]
pub async fn detect_language(text: String) -> Result<DetectionResult, AppError> {
    Ok(lang_detect::detect_language(&text))
}

#[tauri::command]
pub async fn lookup_dictionary(
    text: String,
    state: State<'_, AppState>,
) -> Result<Vec<DictionaryResult>, AppError> {
    let trimmed = text.trim();
    if !dictionary::is_single_word(trimmed) {
        return Ok(vec![]);
    }

    // English: local ECDICT first (same source as hover dict), then Youdao.
    if !dictionary::is_cjk(trimmed) {
        if let Some(pool) = state.ecdict_pool.as_ref() {
            if let Ok(body) = lookup_ecdict_for_dictionary(trimmed, pool).await {
                if dictionary::has_real_meanings(&body) {
                    return Ok(body);
                }
            }
        }
        let dict = dictionary::Dictionary::new();
        return dict
            .lookup(trimmed)
            .await
            .map_err(|e| AppError::Internal(format!("Dictionary lookup failed: {}", e)));
    }

    // CJK: Youdao CE when available (no ECDICT path).
    let dict = dictionary::Dictionary::new();
    dict.lookup_chinese(trimmed)
        .await
        .map_err(|e| AppError::Internal(format!("Dictionary lookup failed: {}", e)))
}

/// ECDICT → DictionaryResult (shared shape with hover overlay).
async fn lookup_ecdict_for_dictionary(
    word: &str,
    pool: &sqlx::SqlitePool,
) -> Result<Vec<DictionaryResult>, String> {
    use crate::models::dictionary::{Definition, Meaning};
    use sqlx::Row;

    let key = word.trim().to_lowercase();
    let row = match sqlx::query(
        "SELECT word, phonetic, definition, translation, pos FROM stardict WHERE word = ?1 COLLATE NOCASE LIMIT 1",
    )
    .bind(&key)
    .fetch_optional(pool)
    .await
    {
        Ok(r) => r,
        Err(_) => sqlx::query(
            "SELECT word, phonetic, definition, translation FROM stardict WHERE word = ?1 COLLATE NOCASE LIMIT 1",
        )
        .bind(&key)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?,
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
    let push_def = |meanings: &mut Vec<Meaning>, pos: &str, def_text: &str| {
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
    };

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
            push_def(&mut meanings, pos, def);
        }
    }
    if meanings.is_empty() {
        if let Some(def) = definition {
            for line in def.split('\n').take(4) {
                push_def(&mut meanings, default_pos.as_str(), line);
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

// We need to make AppState cloneable for the clipboard monitor
impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            translation: crate::app_context::TranslationContext {
                service: self.translation.service.clone(),
                engine_router: self.translation.engine_router.clone(),
                cache: self.translation.cache.clone(),
                glossary: self.translation.glossary.clone(),
                metrics: self.translation.metrics.clone(),
            },
            document: crate::app_context::DocumentContext {
                history: self.document.history.clone(),
                wordbook: self.document.wordbook.clone(),
                post_processor: self.document.post_processor.clone(),
                pre_processor: self.document.pre_processor.clone(),
            },
            overlay: crate::app_context::OverlayContext {
                follow_controller: self.overlay.follow_controller.clone(),
                http_server: self.overlay.http_server.clone(),
            },
            hook: crate::app_context::HookContext {
                hook_monitor: self.hook.hook_monitor.clone(),
                profiles: self.hook.profiles.clone(),
            },
            system: crate::app_context::SystemContext {
                config: self.system.config.clone(),
                selection_manager: self.system.selection_manager.clone(),
                app_detector: self.system.app_detector.clone(),
            },
            // OnceCell fields: create new empty cells for clones
            selection_translation: tokio::sync::OnceCell::new(),
            input_replacement: tokio::sync::OnceCell::new(),
            selection_auto_watch: tokio::sync::OnceCell::new(),
            // Batch manager: share the same Arc
            batch: self.batch.clone(),
            // Speech recognition state
            speech_state: self.speech_state.clone(),
            // Database
            ecdict_pool: self.ecdict_pool.clone(),
            event_store: self.event_store.clone(),
        }
    }
}

#[tauri::command]
pub async fn polish_translation(
    state: State<'_, AppState>,
    source_text: String,
    translated_text: String,
    from_lang: String,
    to_lang: String,
) -> Result<String, AppError> {
    security::validate_text_length(&source_text, security::MAX_TRANSLATION_TEXT_LENGTH)?;
    security::validate_text_length(&translated_text, security::MAX_TRANSLATION_TEXT_LENGTH)?;
    security::validate_language_code(&from_lang)?;
    security::validate_language_code(&to_lang)?;

    if translated_text.trim().is_empty() {
        return Err(AppError::EmptyText);
    }

    // Build polish prompt
    let lang_name = |code: &str| -> String {
        match code {
            "zh" => "中文".to_string(),
            "en" => "English".to_string(),
            "ja" => "日本語".to_string(),
            "ko" => "한국어".to_string(),
            "fr" => "Français".to_string(),
            "de" => "Deutsch".to_string(),
            "es" => "Español".to_string(),
            "ru" => "Русский".to_string(),
            _ => code.to_string(),
        }
    };

    let prompt = format!(
        r#"请对以下翻译进行润色，使其更加自然流畅、符合{}的表达习惯。

原文（{}）：
{}

当前译文：
{}

要求：
1. 保持原文含义不变
2. 使译文更加自然流畅
3. 修正可能的语法或表达问题
4. 只返回润色后的译文，不要添加任何解释"#,
        lang_name(&to_lang),
        lang_name(&from_lang),
        source_text,
        translated_text
    );

    let result = state
        .translation
        .service
        .run_primary(
            crate::models::translation::TranslateChannel::Ui,
            &prompt,
            &from_lang,
            &to_lang,
        )
        .await?;
    Ok(result)
}

/// Query Translation Memory for a match
#[tauri::command]
pub async fn query_tm(
    state: State<'_, AppState>,
    text: String,
    from: String,
    to: String,
) -> Result<Option<crate::models::memory::TmMatch>, AppError> {
    let config = state.system.config.lock().await;
    let threshold = config.tm_threshold;
    drop(config);

    let history = state.document.history.lock().await;
    Ok(history.fuzzy_match(&text, &from, &to, threshold))
}

/// Translate with all enabled engines in parallel for comparison
#[tauri::command]
pub async fn compare_translate(
    state: State<'_, AppState>,
    request: TranslateRequest,
) -> Result<TranslateResponse, AppError> {
    security::validate_text_length(&request.text, security::MAX_TRANSLATION_TEXT_LENGTH)?;
    security::validate_language_code(&request.from)?;
    security::validate_language_code(&request.to)?;

    let outcome = state
        .translation
        .service
        .run(crate::models::translation::TranslateRequest {
            channel: crate::models::translation::TranslateChannel::Ui,
            mode: crate::models::translation::TranslationMode::Compare,
            text: request.text,
            from: request.from,
            to: request.to,
            ..Default::default()
        })
        .await?;
    outcome
        .into_full()
        .ok_or_else(|| AppError::Internal("compare produced non-full outcome".into()))
}
