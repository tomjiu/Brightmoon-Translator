use crate::post_process::{PostProcessConfig, ReplacementRule};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_post_process_config(state: State<'_, AppState>) -> Result<PostProcessConfig, String> {
    let processor = state.post_processor.lock().await;
    Ok(processor.get_config())
}

#[tauri::command]
pub async fn update_post_process_config(
    state: State<'_, AppState>,
    config: PostProcessConfig,
) -> Result<(), String> {
    let processor = state.post_processor.lock().await;
    processor.update_config(config);
    Ok(())
}

#[tauri::command]
pub async fn add_replacement_rule(
    state: State<'_, AppState>,
    pattern: String,
    replacement: String,
    is_regex: Option<bool>,
) -> Result<(), String> {
    let processor = state.post_processor.lock().await;
    let id = uuid::Uuid::new_v4().to_string();
    let rule = ReplacementRule {
        id,
        pattern,
        replacement,
        enabled: true,
        is_regex: is_regex.unwrap_or(false),
    };
    processor.add_rule(rule);
    Ok(())
}

#[tauri::command]
pub async fn remove_replacement_rule(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let processor = state.post_processor.lock().await;
    processor.remove_rule(&id);
    Ok(())
}

#[tauri::command]
pub async fn update_replacement_rule(
    state: State<'_, AppState>,
    id: String,
    pattern: String,
    replacement: String,
    enabled: bool,
    is_regex: bool,
) -> Result<(), String> {
    let processor = state.post_processor.lock().await;
    let rule = ReplacementRule {
        id: id.clone(),
        pattern,
        replacement,
        enabled,
        is_regex,
    };
    processor.update_rule(&id, rule);
    Ok(())
}

#[tauri::command]
pub async fn test_post_process(
    state: State<'_, AppState>,
    text: String,
) -> Result<String, String> {
    let processor = state.post_processor.lock().await;
    Ok(processor.process(&text))
}
