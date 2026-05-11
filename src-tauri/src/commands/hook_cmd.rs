use crate::capabilities::MonitoredText;
use crate::AppState;
use std::collections::VecDeque;
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::Mutex;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect};

/// Check if text is worth translating.
fn is_translatable(text: &str, recent: &VecDeque<String>) -> bool {
    let trimmed = text.trim();
    if trimmed.len() < 3 {
        return false;
    }

    let char_count = trimmed.chars().count();
    if char_count < 2 {
        return false;
    }

    let meaningful = trimmed
        .chars()
        .filter(|c| c.is_alphabetic() || is_cjk(*c))
        .count();

    if meaningful * 10 < char_count * 3 {
        return false;
    }

    if recent.contains(&trimmed.to_string()) {
        return false;
    }

    true
}

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}' |
        '\u{3400}'..='\u{4DBF}' |
        '\u{F900}'..='\u{FAFF}' |
        '\u{3040}'..='\u{309F}' |
        '\u{30A0}'..='\u{30FF}' |
        '\u{AC00}'..='\u{D7AF}'
    )
}

/// Get the foreground window's bounding rectangle.
/// Returns [x, y, width, height] in physical pixels.
#[tauri::command]
pub async fn get_foreground_window_rect() -> Result<[i32; 4], String> {
    tokio::task::spawn_blocking(|| unsafe {
        let hwnd = GetForegroundWindow();
        let mut rect = windows::Win32::Foundation::RECT::default();
        GetWindowRect(hwnd, &mut rect).map_err(|e| e.to_string())?;
        Ok([
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
        ])
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Start hook monitor for foreground window text
#[tauri::command]
pub async fn start_hook_monitor(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let mut monitor = state.hook_monitor.lock().await;

    if monitor.is_running().await {
        return Ok("Monitor already running".to_string());
    }

    let config = state.config.lock().await;
    let target_lang = config.default_to.clone();
    let source_lang = config.default_from.clone();
    drop(config);

    let translation_service = state.translation_service.clone();
    let recent_texts: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));

    monitor
        .start(move |text: MonitoredText| {
            let translation_service = translation_service.clone();
            let target_lang = target_lang.clone();
            let source_lang = source_lang.clone();
            let app_handle = app_handle.clone();
            let recent_texts = recent_texts.clone();

            tokio::spawn(async move {
                // Check if text is worth translating
                {
                    let recent = recent_texts.lock().await;
                    if !is_translatable(&text.text, &recent) {
                        return;
                    }
                }

                // Dedup
                {
                    let mut recent = recent_texts.lock().await;
                    recent.push_back(text.text.trim().to_string());
                    while recent.len() > 20 {
                        recent.pop_front();
                    }
                }

                // Translate
                match translation_service
                    .translate(&text.text, &source_lang, &target_lang)
                    .await
                {
                    Ok(response) => {
                        if let Some(result) = response.results.first() {
                            let _ = app_handle.emit(
                                "hook-text-translated",
                                serde_json::json!({
                                    "window_title": text.window_title,
                                    "process_name": text.process_name,
                                    "original": text.text,
                                    "translated": result.text,
                                    "engine": result.engine,
                                    "timestamp": text.timestamp,
                                    "source": text.source,
                                }),
                            );
                        }
                    }
                    Err(e) => {
                        log::warn!("[HookMonitor] Translation failed: {}", e);
                    }
                }
            });
        })
        .await?;

    Ok("Monitor started".to_string())
}

#[tauri::command]
pub async fn stop_hook_monitor(state: State<'_, AppState>) -> Result<String, String> {
    let monitor = state.hook_monitor.lock().await;
    monitor.stop().await;
    Ok("Monitor stopped".to_string())
}

#[tauri::command]
pub async fn get_hook_monitor_status(state: State<'_, AppState>) -> Result<bool, String> {
    let monitor = state.hook_monitor.lock().await;
    Ok(monitor.is_running().await)
}
