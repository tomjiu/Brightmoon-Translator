use crate::capabilities::input_replacement::ReplacementResult;
use crate::dictionary::{self, DictionaryResult};
use crate::engine::TranslateResponse;
use crate::error::AppError;
use crate::lang_detect::{self, DetectionResult};
use crate::security;
use crate::AppState;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, State};

// Shared clipboard monitoring state
static CLIPBOARD_MONITORING: AtomicBool = AtomicBool::new(false);

#[derive(Deserialize)]
pub struct TranslateRequest {
    pub text: String,
    pub from: String,
    pub to: String,
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

    // Use TranslationService for the full pipeline
    let response = state
        .translation
        .service
        .translate(&request.text, &request.from, &request.to)
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

#[tauri::command]
pub async fn start_clipboard_monitor(
    app: tauri::AppHandle,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    use std::thread;
    use std::time::Duration;

    // Atomic check-and-set to prevent duplicate monitoring threads
    match CLIPBOARD_MONITORING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst) {
        Ok(false) => {
            // Successfully set from false to true, proceed to spawn thread
        },
        _ => {
            // Already monitoring
            return Ok(());
        },
    }

    let app_handle = app.clone();

    thread::spawn(move || {
        loop {
            if !CLIPBOARD_MONITORING.load(Ordering::Relaxed) {
                break;
            }

            // Read clipboard using arboard crate or Windows API
            // For now, emit event to frontend to read clipboard
            let _ = app_handle.emit("read-clipboard", ());

            thread::sleep(Duration::from_millis(500));
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_clipboard_monitor() -> Result<(), String> {
    CLIPBOARD_MONITORING.store(false, Ordering::Relaxed);
    Ok(())
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

    // Translate using service
    let response = state
        .translation
        .service
        .translate(&text, &from, &to)
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
/// Uses the InputReplacement capability: selection → translate → clipboard paste.
/// No frontend clipboard read needed — the capability handles everything.
#[tauri::command]
pub async fn replace_translate(state: State<'_, AppState>) -> Result<ReplacementResult, AppError> {
    let config = state.system.config.lock().await;
    let from = config.default_from.clone();
    let to = config.default_to.clone();
    drop(config);

    let cap = state.input_replacement.get().ok_or_else(|| {
        AppError::Internal("InputReplacement capability not initialized".to_string())
    })?;

    let result = cap
        .replace_translate(&from, &to)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(result)
}

/// Replace text in the foreground application via the InputReplacement capability.
#[tauri::command]
pub async fn replace_text_in_app(state: State<'_, AppState>, text: String) -> Result<(), AppError> {
    let cap = state.input_replacement.get().ok_or_else(|| {
        AppError::Internal("InputReplacement capability not initialized".to_string())
    })?;

    cap.replace_text(&text)
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
        .translate_primary(&text, &to, &from)
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
) -> Result<Vec<EmbeddedLine>, AppError> {
    security::validate_text_length(&text, security::MAX_TRANSLATION_TEXT_LENGTH)?;
    security::validate_language_code(&from)?;
    security::validate_language_code(&to)?;

    if text.trim().is_empty() {
        return Ok(vec![]);
    }

    // Use batch translation with concurrency of 3
    let batch_results = state
        .translation
        .service
        .translate_batch(
            &text
                .lines()
                .enumerate()
                .filter(|(_, l)| !l.trim().is_empty())
                .map(|(i, l)| (i, l.trim()))
                .collect::<Vec<_>>(),
            &from,
            &to,
            3, // concurrency
        )
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
pub async fn lookup_dictionary(text: String) -> Result<Vec<DictionaryResult>, AppError> {
    let trimmed = text.trim();
    if !dictionary::is_single_word(trimmed) {
        return Ok(vec![]);
    }

    let dict = dictionary::Dictionary::new();

    // Use Chinese dictionary for CJK text
    if dictionary::is_cjk(trimmed) {
        dict.lookup_chinese(trimmed)
            .await
            .map_err(|e| AppError::Internal(format!("Dictionary lookup failed: {}", e)))
    } else {
        dict.lookup(trimmed)
            .await
            .map_err(|e| AppError::Internal(format!("Dictionary lookup failed: {}", e)))
    }
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
            // Batch manager: share the same Arc
            batch: self.batch.clone(),
            // Speech recognition state
            speech_state: self.speech_state.clone(),
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

    // Use service to polish
    let result = state
        .translation
        .service
        .translate_primary(&prompt, &from_lang, &to_lang)
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

    let router = state.translation.engine_router.read().await;
    let response = router
        .translate_parallel_compare(&request.text, &request.from, &request.to)
        .await;
    Ok(response)
}
