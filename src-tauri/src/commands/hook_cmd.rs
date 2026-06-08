use crate::AppState;
use tauri::State;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect};

/// Get the foreground window's bounding rectangle.
/// Returns [x, y, width, height] in physical pixels.
#[tauri::command]
pub async fn get_foreground_window_rect() -> Result<[i32; 4], String> {
    // SAFETY: GetForegroundWindow and GetWindowRect are standard Win32 APIs.
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
    let mut monitor = state.hook.hook_monitor.lock().await;

    if monitor.is_running().await {
        return Ok("Monitor already running".to_string());
    }

    let config = state.system.config.lock().await;
    let target_lang = config.default_to.clone();
    let source_lang = config.default_from.clone();
    let enabled_sources = config.hook.enabled_sources.clone();
    let uia_interval = config.hook.uia_interval_ms;
    let ocr_interval = config.hook.ocr_interval_ms;
    drop(config);

    monitor
        .start_with_translation(
            &enabled_sources,
            uia_interval,
            ocr_interval,
            source_lang,
            target_lang,
            state.translation.service.clone(),
            app_handle,
        )
        .await?;

    Ok("Monitor started".to_string())
}

#[tauri::command]
pub async fn stop_hook_monitor(state: State<'_, AppState>) -> Result<String, String> {
    let monitor = state.hook.hook_monitor.lock().await;
    monitor.stop().await;
    Ok("Monitor stopped".to_string())
}

#[tauri::command]
pub async fn get_hook_monitor_status(state: State<'_, AppState>) -> Result<bool, String> {
    let monitor = state.hook.hook_monitor.lock().await;
    Ok(monitor.is_running().await)
}
