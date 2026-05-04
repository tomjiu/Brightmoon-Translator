use crate::memory::HistoryItem;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_history(state: State<'_, AppState>) -> Result<Vec<HistoryItem>, String> {
    let history = state.history.lock().await;
    Ok(history.get_all())
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    let history = state.history.lock().await;
    history.clear();
    Ok(())
}

#[tauri::command]
pub async fn delete_history_item(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let history = state.history.lock().await;
    history.remove(&id);
    Ok(())
}

#[tauri::command]
pub async fn batch_delete_history(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    let history = state.history.lock().await;
    history.batch_remove(&ids);
    Ok(())
}
