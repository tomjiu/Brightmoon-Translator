use crate::engine::offline::OfflineEngine;
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

/// Offline model info returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineModelInfo {
    pub id: String,
    pub name: String,
    pub source_lang: String,
    pub target_lang: String,
    pub version: String,
    pub size_bytes: u64,
    pub downloaded: bool,
    pub download_url: String,
    pub local_size_bytes: Option<u64>,
}

/// Get list of all available offline models
#[tauri::command]
pub async fn get_offline_models(state: State<'_, AppState>) -> Result<Vec<OfflineModelInfo>, String> {
    let config = state.system.config.lock().await;
    let model_dir = if config.engines.offline.model_dir.is_empty() {
        None
    } else {
        Some(config.engines.offline.model_dir.as_str())
    };
    let engine = OfflineEngine::new(model_dir);
    drop(config);

    let available = OfflineEngine::available_models();
    let mut result = Vec::new();

    for model in available {
        let downloaded = engine.is_model_downloaded(&model.source_lang, &model.target_lang);
        let local_size = engine.model_size(&model.source_lang, &model.target_lang);

        result.push(OfflineModelInfo {
            id: model.id,
            name: model.name,
            source_lang: model.source_lang,
            target_lang: model.target_lang,
            version: model.version,
            size_bytes: model.size_bytes,
            downloaded,
            download_url: model.download_url,
            local_size_bytes: local_size,
        });
    }

    Ok(result)
}

/// Download an offline model
#[tauri::command]
pub async fn download_offline_model(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<(), String> {
    let config = state.system.config.lock().await;
    let model_dir = if config.engines.offline.model_dir.is_empty() {
        None
    } else {
        Some(config.engines.offline.model_dir.as_str())
    };
    let engine = OfflineEngine::new(model_dir);
    drop(config);

    engine
        .download_model(&model_id)
        .await
        .map_err(|e| e.to_string())?;

    // Update config to record downloaded model
    let mut config = state.system.config.lock().await;
    if !config.engines.offline.downloaded_models.contains(&model_id) {
        config.engines.offline.downloaded_models.push(model_id);
    }
    config.save();

    Ok(())
}

/// Delete an offline model
#[tauri::command]
pub async fn delete_offline_model(
    state: State<'_, AppState>,
    source_lang: String,
    target_lang: String,
) -> Result<(), String> {
    let config = state.system.config.lock().await;
    let model_dir = if config.engines.offline.model_dir.is_empty() {
        None
    } else {
        Some(config.engines.offline.model_dir.as_str())
    };
    let engine = OfflineEngine::new(model_dir);
    drop(config);

    engine
        .delete_model(&source_lang, &target_lang)
        .await
        .map_err(|e| e.to_string())?;

    // Update config to remove model from downloaded list
    let model_id = format!("{}-{}", source_lang, target_lang);
    let mut config = state.system.config.lock().await;
    config.engines.offline.downloaded_models.retain(|m| m != &model_id);
    config.save();

    Ok(())
}

/// Toggle offline engine enabled/disabled
#[tauri::command]
pub async fn toggle_offline_engine(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let mut config = state.system.config.lock().await;
    config.engines.offline.enabled = enabled;
    config.save();
    Ok(())
}

/// Update offline engine settings
#[tauri::command]
pub async fn update_offline_settings(
    state: State<'_, AppState>,
    auto_switch: Option<bool>,
    model_dir: Option<String>,
) -> Result<(), String> {
    let mut config = state.system.config.lock().await;

    if let Some(auto) = auto_switch {
        config.engines.offline.auto_switch = auto;
    }

    if let Some(dir) = model_dir {
        config.engines.offline.model_dir = dir;
    }

    config.save();
    Ok(())
}

/// Generate sample offline models for testing
#[tauri::command]
pub async fn generate_sample_offline_models(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let config = state.system.config.lock().await;
    let model_dir = if config.engines.offline.model_dir.is_empty() {
        None
    } else {
        Some(config.engines.offline.model_dir.as_str())
    };
    let engine = OfflineEngine::new(model_dir);
    drop(config);

    // Create model directory if it doesn't exist
    let dir = engine.model_dir().clone();
    if !dir.exists() {
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| e.to_string())?;
    }

    let pairs = vec![("en", "zh"), ("zh", "en"), ("ja", "zh"), ("en", "ja")];
    let mut generated = Vec::new();

    for (source, target) in pairs {
        let model = crate::engine::offline::generate_sample_model(source, target);
        let model_id = format!("{}-{}", source, target);
        let model_path = dir.join(format!("{}.json", model_id));

        match serde_json::to_string_pretty(&model) {
            Ok(json) => {
                if let Err(e) = tokio::fs::write(&model_path, json).await {
                    tracing::warn!("Failed to write sample model {}: {}", model_id, e);
                } else {
                    generated.push(model_id.clone());

                    // Update config
                    let mut config = state.system.config.lock().await;
                    if !config.engines.offline.downloaded_models.contains(&model_id) {
                        config.engines.offline.downloaded_models.push(model_id);
                    }
                    config.save();
                }
            }
            Err(e) => {
                tracing::warn!("Failed to serialize sample model {}: {}", model_id, e);
            }
        }
    }

    Ok(generated)
}

/// Get offline engine status
#[tauri::command]
pub async fn get_offline_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let config = state.system.config.lock().await;
    let model_dir = if config.engines.offline.model_dir.is_empty() {
        None
    } else {
        Some(config.engines.offline.model_dir.as_str())
    };
    let engine = OfflineEngine::new(model_dir);
    drop(config);

    let available_pairs = engine.available_pairs().await;

    Ok(serde_json::json!({
        "enabled": state.system.config.lock().await.engines.offline.enabled,
        "autoSwitch": state.system.config.lock().await.engines.offline.auto_switch,
        "loadedModels": available_pairs,
        "modelDir": engine.model_dir().display().to_string(),
    }))
}
