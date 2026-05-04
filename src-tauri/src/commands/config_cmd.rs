use crate::config::AppConfig;
use crate::engine::Router;
use crate::AppState;
use tauri::State;


#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = state.config.lock().await;
    Ok(config.clone())
}

#[tauri::command]
pub async fn save_config(state: State<'_, AppState>, config: AppConfig) -> Result<(), String> {
    config.save();
    let mut current = state.config.lock().await;
    *current = config.clone();
    drop(current);

    // Rebuild engine router with new config (safe RwLock write)
    let new_router = Router::new(&config);
    let mut router = state.engine_router.write().await;
    *router = new_router;

    Ok(())
}

#[tauri::command]
pub async fn save_window_position(
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let mut config = state.config.lock().await;
    config.window_x = Some(x);
    config.window_y = Some(y);
    config.window_width = Some(width);
    config.window_height = Some(height);
    config.save();
    Ok(())
}

#[tauri::command]
pub async fn get_window_position(state: State<'_, AppState>) -> Result<Option<(f64, f64, f64, f64)>, String> {
    let config = state.config.lock().await;
    if let (Some(x), Some(y), Some(w), Some(h)) = (config.window_x, config.window_y, config.window_width, config.window_height) {
        Ok(Some((x, y, w, h)))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn get_api_server_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let config = state.config.lock().await;
    Ok(serde_json::json!({
        "enabled": config.api_server_enabled,
        "port": config.api_server_port,
    }))
}

#[tauri::command]
pub async fn export_config_json(state: State<'_, AppState>) -> Result<String, String> {
    let config = state.config.lock().await;
    serde_json::to_string_pretty(&*config).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_config_json(state: State<'_, AppState>, json: String) -> Result<(), String> {
    let imported: AppConfig = serde_json::from_str(&json).map_err(|e| format!("Invalid config JSON: {}", e))?;
    imported.save();
    let mut current = state.config.lock().await;
    *current = imported.clone();
    drop(current);

    // Rebuild engine router (safe RwLock write)
    let new_router = Router::new(&imported);
    let mut router = state.engine_router.write().await;
    *router = new_router;

    Ok(())
}

#[tauri::command]
pub async fn get_translation_blacklist(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let config = state.config.lock().await;
    Ok(config.translation_blacklist.clone())
}

#[tauri::command]
pub async fn update_translation_blacklist(
    state: State<'_, AppState>,
    blacklist: Vec<String>,
) -> Result<(), String> {
    let mut config = state.config.lock().await;
    config.translation_blacklist = blacklist;
    config.save();
    Ok(())
}
