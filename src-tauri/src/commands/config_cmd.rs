use crate::config::AppConfig;
use crate::engine::Router;
use crate::error::AppError;
use crate::hotkey;
use crate::AppState;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, AppError> {
    let config = state.system.config.lock().await;
    Ok(config.clone())
}

/// Returns the default AppConfig. Used by the frontend to get authoritative
/// defaults instead of maintaining a duplicated TypeScript default object.
#[tauri::command]
pub async fn get_default_config() -> Result<AppConfig, AppError> {
    Ok(AppConfig::default())
}

#[tauri::command]
pub async fn save_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<(), AppError> {
    config.save();
    let mut current = state.system.config.lock().await;
    *current = config.clone();
    drop(current);

    // Rebuild engine router with new config (safe RwLock write)
    let new_router = Router::new(&config);
    let mut router = state.translation.engine_router.write().await;
    *router = new_router;
    drop(router);

    // Hot-reload selection UX (auto-on-select / hover / pop button)
    if let Some(watch) = state.selection_auto_watch.get() {
        watch.update_config(config.selection_ux.clone()).await;
    }

    // Re-register global hotkeys so settings apply without restart
    hotkey::reregister(&app, &config.hotkeys);

    Ok(())
}

#[tauri::command]
pub async fn save_window_position(
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), AppError> {
    let mut config = state.system.config.lock().await;
    config.window_x = Some(x);
    config.window_y = Some(y);
    config.window_width = Some(width);
    config.window_height = Some(height);
    config.save();
    Ok(())
}

#[tauri::command]
pub async fn get_window_position(
    state: State<'_, AppState>,
) -> Result<Option<(f64, f64, f64, f64)>, AppError> {
    let config = state.system.config.lock().await;
    if let (Some(x), Some(y), Some(w), Some(h)) = (
        config.window_x,
        config.window_y,
        config.window_width,
        config.window_height,
    ) {
        Ok(Some((x, y, w, h)))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn get_api_server_status(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, AppError> {
    let config = state.system.config.lock().await;
    Ok(serde_json::json!({
        "enabled": config.api_server_enabled,
        "port": config.api_server_port,
    }))
}

#[tauri::command]
pub async fn export_config_json(state: State<'_, AppState>) -> Result<String, AppError> {
    let config = state.system.config.lock().await;
    // Export with masked API keys to prevent secret leakage
    let masked = config.masked_copy();
    let json = serde_json::to_string_pretty(&masked)?;
    Ok(json)
}

#[tauri::command]
pub async fn import_config_json(state: State<'_, AppState>, json: String) -> Result<(), AppError> {
    let imported: AppConfig = serde_json::from_str(&json)?;
    imported.save();
    let mut current = state.system.config.lock().await;
    *current = imported.clone();
    drop(current);

    // Rebuild engine router (safe RwLock write)
    let new_router = Router::new(&imported);
    let mut router = state.translation.engine_router.write().await;
    *router = new_router;

    Ok(())
}

#[tauri::command]
pub async fn get_translation_blacklist(
    state: State<'_, AppState>,
) -> Result<Vec<String>, AppError> {
    let config = state.system.config.lock().await;
    Ok(config.translation_blacklist.clone())
}

#[tauri::command]
pub async fn update_translation_blacklist(
    state: State<'_, AppState>,
    blacklist: Vec<String>,
) -> Result<(), AppError> {
    let mut config = state.system.config.lock().await;
    config.translation_blacklist = blacklist;
    config.save();
    Ok(())
}
