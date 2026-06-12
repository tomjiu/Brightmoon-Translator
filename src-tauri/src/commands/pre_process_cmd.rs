use crate::pre_process::PreProcessRule;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_pre_process_config(
    state: State<'_, AppState>,
) -> Result<crate::pre_process::PreProcessConfig, String> {
    Ok(state.document.pre_processor.get_config())
}

#[tauri::command]
pub async fn update_pre_process_config(
    state: State<'_, AppState>,
    config: crate::pre_process::PreProcessConfig,
) -> Result<(), String> {
    state.document.pre_processor.update_config(config);
    Ok(())
}

#[tauri::command]
pub async fn add_pre_process_rule(
    state: State<'_, AppState>,
    pattern: String,
    replacement: String,
    is_regex: bool,
    lang_pair: Option<String>,
) -> Result<(), String> {
    let rule = PreProcessRule {
        id: uuid::Uuid::new_v4().to_string(),
        pattern,
        replacement,
        enabled: true,
        is_regex,
        lang_pair: lang_pair.or_else(|| Some("all".to_string())),
    };
    state.document.pre_processor.add_rule(rule);
    Ok(())
}

#[tauri::command]
pub async fn remove_pre_process_rule(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.document.pre_processor.remove_rule(&id);
    Ok(())
}

#[tauri::command]
pub async fn update_pre_process_rule(
    state: State<'_, AppState>,
    id: String,
    pattern: String,
    replacement: String,
    enabled: bool,
    is_regex: bool,
    lang_pair: Option<String>,
) -> Result<(), String> {
    let rule = PreProcessRule {
        id: id.clone(),
        pattern,
        replacement,
        enabled,
        is_regex,
        lang_pair: lang_pair.or_else(|| Some("all".to_string())),
    };
    state.document.pre_processor.update_rule(&id, rule);
    Ok(())
}

#[tauri::command]
pub async fn test_pre_process(
    state: State<'_, AppState>,
    text: String,
    lang_pair: Option<String>,
) -> Result<String, String> {
    Ok(state
        .document
        .pre_processor
        .process(&text, lang_pair.as_deref()))
}
