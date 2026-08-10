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
        GetWindowRect(hwnd, &raw mut rect).map_err(|e| e.to_string())?;
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

/// Start hook monitor for foreground window text.
/// Applies the active hook profile (or auto-matched by foreground process/title)
/// when one is configured; otherwise uses global hook config.
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
    let mut target_lang = config.default_to.clone();
    let mut source_lang = config.default_from.clone();
    let mut enabled_sources = config.hook.enabled_sources.clone();
    let mut uia_interval = config.hook.uia_interval_ms;
    let mut ocr_interval = config.hook.ocr_interval_ms;
    drop(config);

    // Resolve profile: auto-match foreground app first, else active profile.
    let foreground = crate::capabilities::platform::windows::detect_foreground_app();
    let matched = foreground.as_ref().and_then(|fg| {
        state
            .hook
            .profiles
            .auto_match(&fg.app_name, &fg.window_title)
    });
    let profile = matched.or_else(|| state.hook.profiles.get_active());

    if let Some(profile) = profile {
        tracing::info!(
            "[Hook] Applying profile '{}' (id={})",
            profile.name,
            profile.id
        );
        // Mark as last-used when applied at start
        state.hook.profiles.activate(Some(&profile.id));

        enabled_sources = profile.hook_config.enabled_sources.clone();
        uia_interval = profile.hook_config.uia_interval_ms;
        ocr_interval = profile.hook_config.ocr_interval_ms;
        if let Some(ref s) = profile.source_lang {
            if !s.is_empty() {
                source_lang = s.clone();
            }
        }
        if let Some(ref t) = profile.target_lang {
            if !t.is_empty() {
                target_lang = t.clone();
            }
        }
    }

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
