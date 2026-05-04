use crate::dictionary::{self, DictionaryResult};
use crate::engine::TranslateResponse;
use crate::lang_detect::{self, DetectionResult};
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
) -> Result<TranslateResponse, String> {
    // Use TranslationService for the full pipeline
    let response = state
        .translation_service
        .translate(&request.text, &request.from, &request.to)
        .await?;

    // Auto-copy result if enabled
    let config = state.config.lock().await;
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
) -> Result<String, String> {
    // Create channel for streaming tokens
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(100);

    // Spawn task to forward tokens to Tauri event
    let app_handle = app.clone();
    let forward_handle = tokio::spawn(async move {
        let mut full_text = String::new();
        while let Some(chunk) = rx.recv().await {
            full_text.push_str(&chunk);
            let _ = app_handle.emit("stream-chunk", serde_json::json!({
                "chunk": chunk,
                "done": false,
            }));
        }
        // Emit completion
        let _ = app_handle.emit("stream-chunk", serde_json::json!({
            "chunk": "",
            "done": true,
        }));
        full_text
    });

    // Stream translation using TranslationService
    let result = state
        .translation_service
        .translate_stream(&request.text, &request.from, &request.to, tx)
        .await;

    // Wait for forwarding to complete
    let _full_text = forward_handle.await.map_err(|e| e.to_string())?;

    result
}

#[tauri::command]
pub async fn start_clipboard_monitor(
    app: tauri::AppHandle,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    use std::thread;
    use std::time::Duration;

    if CLIPBOARD_MONITORING.load(Ordering::Relaxed) {
        return Ok(());
    }

    CLIPBOARD_MONITORING.store(true, Ordering::Relaxed);

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
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("Text is empty".to_string());
    }

    // Get config
    let config = state.config.lock().await;
    let from = config.default_from.clone();
    let to = config.default_to.clone();
    drop(config);

    // Translate using service
    let response = state
        .translation_service
        .translate(&text, &from, &to)
        .await?;

    if let Some(first) = response.results.first() {
        // Emit result to frontend for overlay display
        let _ = app.emit("selection-translated", serde_json::json!({
            "source": text,
            "translated": first.text,
            "engine": first.engine,
        }));
    }

    Ok(())
}

#[tauri::command]
pub async fn replace_translate(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("Text is empty".to_string());
    }

    let config = state.config.lock().await;
    let from = config.default_from.clone();
    let to = config.default_to.clone();
    drop(config);

    // Use service for translation
    let response = state
        .translation_service
        .translate_primary(&text, &from, &to)
        .await?;

    // Emit event to frontend to simulate typing
    let _ = app.emit("replace-typing", &response);

    Ok(response)
}

#[tauri::command]
pub async fn replace_text_in_app(
    text: String,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::CString;

        // Windows API declarations
        #[repr(C)]
        struct INPUT {
            type_: u32,
            union_data: [u8; 24], // sizeof(union) = max(sizeof(KEYBDINPUT), sizeof(MOUSEINPUT), sizeof(HARDWAREINPUT))
        }

        #[repr(C)]
        struct KEYBDINPUT {
            wVk: u16,
            wScan: u16,
            dwFlags: u32,
            time: u32,
            dwExtraInfo: usize,
        }

        const INPUT_KEYBOARD: u32 = 1;
        const KEYEVENTF_KEYUP: u32 = 0x0002;
        const KEYEVENTF_CTRL: u32 = 0x0000;
        const VK_CONTROL: u16 = 0x11;
        const VK_V: u16 = 0x56;

        extern "system" {
            fn SendInput(cInputs: u32, pInputs: *const INPUT, cbSize: i32) -> u32;
            fn OpenClipboard(hWndNewOwner: *mut std::ffi::c_void) -> i32;
            fn CloseClipboard() -> i32;
            fn EmptyClipboard() -> i32;
            fn SetClipboardData(uFormat: u32, hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            fn GetClipboardData(uFormat: u32) -> *mut std::ffi::c_void;
            fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
            fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;
            fn GlobalSize(hMem: *mut std::ffi::c_void) -> usize;
        }

        const CF_UNICODETEXT: u32 = 13;
        const GMEM_MOVEABLE: u32 = 0x0002;

        unsafe {
            // Save current clipboard content
            let saved_text = if OpenClipboard(std::ptr::null_mut()) != 0 {
                let h_data = GetClipboardData(CF_UNICODETEXT);
                let saved = if !h_data.is_null() {
                    let p_data = GlobalLock(h_data);
                    if !p_data.is_null() {
                        let size = GlobalSize(h_data);
                        let slice = std::slice::from_raw_parts(p_data as *const u8, size);
                        let saved = slice.to_vec();
                        GlobalUnlock(h_data);
                        Some(saved)
                    } else {
                        None
                    }
                } else {
                    None
                };
                CloseClipboard();
                saved
            } else {
                None
            };

            // Set new clipboard content
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                EmptyClipboard();

                // Convert text to UTF-16
                let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                let size = wide.len() * 2;

                let h_mem = GlobalAlloc(GMEM_MOVEABLE, size);
                if !h_mem.is_null() {
                    let p_mem = GlobalLock(h_mem);
                    if !p_mem.is_null() {
                        std::ptr::copy_nonoverlapping(wide.as_ptr(), p_mem as *mut u16, wide.len());
                        GlobalUnlock(h_mem);
                        SetClipboardData(CF_UNICODETEXT, h_mem);
                    }
                }

                CloseClipboard();
            }

            // Simulate Ctrl+V
            fn make_input(vk: u16, flags: u32) -> INPUT {
                let mut input = INPUT {
                    type_: INPUT_KEYBOARD,
                    union_data: [0u8; 24],
                };
                let ki = KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                };
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        &ki as *const _ as *const u8,
                        input.union_data.as_mut_ptr(),
                        std::mem::size_of::<KEYBDINPUT>(),
                    );
                }
                input
            }

            let inputs = [
                make_input(VK_CONTROL, 0),           // Ctrl down
                make_input(VK_V, 0),                 // V down
                make_input(VK_V, KEYEVENTF_KEYUP),   // V up
                make_input(VK_CONTROL, KEYEVENTF_KEYUP), // Ctrl up
            ];

            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );

            // Small delay to ensure paste completes
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Restore original clipboard
            if let Some(saved) = saved_text {
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    EmptyClipboard();

                    let h_mem = GlobalAlloc(GMEM_MOVEABLE, saved.len());
                    if !h_mem.is_null() {
                        let p_mem = GlobalLock(h_mem);
                        if !p_mem.is_null() {
                            std::ptr::copy_nonoverlapping(saved.as_ptr(), p_mem as *mut u8, saved.len());
                            GlobalUnlock(h_mem);
                            SetClipboardData(CF_UNICODETEXT, h_mem);
                        }
                    }

                    CloseClipboard();
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn back_translate(
    state: State<'_, AppState>,
    text: String,
    from: String,
    to: String,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("Text is empty".to_string());
    }

    // Translate back: swap from and to languages
    state
        .translation_service
        .translate_primary(&text, &to, &from)
        .await
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
) -> Result<Vec<EmbeddedLine>, String> {
    if text.trim().is_empty() {
        return Ok(vec![]);
    }

    // Use batch translation with concurrency of 3
    let batch_results = state
        .translation_service
        .translate_batch(
            &text.lines()
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
pub async fn detect_language(text: String) -> Result<DetectionResult, String> {
    Ok(lang_detect::detect_language(&text))
}

#[tauri::command]
pub async fn lookup_dictionary(text: String) -> Result<Vec<DictionaryResult>, String> {
    let trimmed = text.trim();
    if !dictionary::is_single_word(trimmed) {
        return Ok(vec![]);
    }

    let dict = dictionary::Dictionary::new();

    // Use Chinese dictionary for CJK text
    if dictionary::is_cjk(trimmed) {
        dict.lookup_chinese(trimmed)
            .await
            .map_err(|e| format!("Dictionary lookup failed: {}", e))
    } else {
        dict.lookup(trimmed)
            .await
            .map_err(|e| format!("Dictionary lookup failed: {}", e))
    }
}

// We need to make AppState cloneable for the clipboard monitor
impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            history: self.history.clone(),
            wordbook: self.wordbook.clone(),
            post_processor: self.post_processor.clone(),
            engine_router: self.engine_router.clone(),
            cache: self.cache.clone(),
            glossary: self.glossary.clone(),
            translation_service: self.translation_service.clone(),
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
) -> Result<String, String> {
    if translated_text.trim().is_empty() {
        return Err("Translation is empty".to_string());
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
    state
        .translation_service
        .translate_primary(&prompt, &from_lang, &to_lang)
        .await
        .map_err(|e| format!("Polish failed: {}", e))
}
