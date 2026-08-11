use crate::engine::offline::{DownloadProgress, OfflineEngine};
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

/// Offline model info returned to frontend (registry-driven).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfflineModelInfo {
    pub id: String,
    pub from: String,
    pub to: String,
    pub display_name: String,
    pub size_label: String,
    pub size_bytes: u64,
    pub downloaded: bool,
    pub sha256: String,
}

/// Get the full catalog of downloadable model pairs with download state.
#[tauri::command]
pub async fn get_offline_models(
    state: State<'_, AppState>,
) -> Result<Vec<OfflineModelInfo>, String> {
    let engine = engine_from_config(&state).await;
    let mut result = Vec::new();

    for spec in OfflineEngine::catalog_entries() {
        let downloaded = engine.is_model_downloaded(&spec.from, &spec.to);
        result.push(OfflineModelInfo {
            id: spec.id.clone(),
            from: spec.from.clone(),
            to: spec.to.clone(),
            display_name: spec.display_name.clone(),
            size_label: spec.size_label.clone(),
            size_bytes: spec.size_bytes,
            downloaded,
            sha256: spec.sha256.clone(),
        });
    }

    Ok(result)
}

/// Download a model pair by `from`/`to`, streaming progress events as
/// `offline-download-progress` (payload: `DownloadProgress`).
#[tauri::command]
pub async fn download_offline_model(
    state: State<'_, AppState>,
    app: AppHandle,
    from: String,
    to: String,
) -> Result<(), String> {
    let engine = engine_from_config(&state).await;
    let pair_id = format!("{from}-{to}");

    engine
        .download_model(&pair_id, Some(|p: DownloadProgress| {
            let _ = app.emit("offline-download-progress", &p);
        }))
        .await
        .map_err(|e| e.to_string())?;

    let mut config = state.system.config.lock().await;
    if !config.engines.offline.downloaded_models.contains(&pair_id) {
        config.engines.offline.downloaded_models.push(pair_id);
    }
    config.save();

    Ok(())
}

/// Delete a downloaded model pair by `from`/`to`.
#[tauri::command]
pub async fn delete_offline_model(
    state: State<'_, AppState>,
    from: String,
    to: String,
) -> Result<(), String> {
    let engine = engine_from_config(&state).await;

    engine
        .delete_model(&from, &to)
        .await
        .map_err(|e| e.to_string())?;

    let model_id = format!("{from}-{to}");
    let mut config = state.system.config.lock().await;
    config
        .engines
        .offline
        .downloaded_models
        .retain(|m| m != &model_id);
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

/// Get offline engine status
#[tauri::command]
pub async fn get_offline_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let engine = engine_from_config(&state).await;
    let available_pairs = engine.available_pairs().await;

    Ok(serde_json::json!({
        "enabled": state.system.config.lock().await.engines.offline.enabled,
        "autoSwitch": state.system.config.lock().await.engines.offline.auto_switch,
        "loadedModels": available_pairs,
        "modelDir": engine.model_dir().display().to_string(),
    }))
}

/// Build an engine from the configured model dir.
async fn engine_from_config(state: &State<'_, AppState>) -> OfflineEngine {
    let config = state.system.config.lock().await;
    let model_dir = if config.engines.offline.model_dir.is_empty() {
        None
    } else {
        Some(config.engines.offline.model_dir.clone())
    };
    drop(config);
    OfflineEngine::new(model_dir.as_deref())
}
