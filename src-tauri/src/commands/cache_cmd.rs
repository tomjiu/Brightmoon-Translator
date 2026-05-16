use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn clear_cache(state: State<'_, AppState>) -> Result<(), String> {
    state.translation.cache.clear().await;
    Ok(())
}

#[tauri::command]
pub async fn cache_size(state: State<'_, AppState>) -> Result<usize, String> {
    Ok(state.translation.cache.size().await)
}
