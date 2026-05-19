/**
 * Batch Translation Commands
 *
 * Tauri commands for batch translation queue management
 */

use crate::batch::{BatchConfig, BatchProgress, BatchTask, BatchJobStatus};
use crate::memory::TmExportData;
use crate::AppState;
use tauri::State;

/// Submit texts for batch translation
#[tauri::command]
pub async fn batch_submit(
    texts: Vec<String>,
    config: Option<BatchConfig>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let batch_config = config.unwrap_or_default();
    let job_id = state.batch.submit(texts, batch_config).await?;

    // Start processing in background
    let service = state.translation.service.clone();
    let batch = state.batch.clone();
    tokio::spawn(async move {
        if let Err(e) = batch.process(service).await {
            tracing::error!("[Batch] Processing error: {}", e);
        }
    });

    Ok(job_id)
}

/// Cancel current batch job
#[tauri::command]
pub async fn batch_cancel(state: State<'_, AppState>) -> Result<(), String> {
    state.batch.cancel().await;
    Ok(())
}

/// Pause current batch job
#[tauri::command]
pub async fn batch_pause(state: State<'_, AppState>) -> Result<(), String> {
    state.batch.pause().await
}

/// Resume paused batch job
#[tauri::command]
pub async fn batch_resume(state: State<'_, AppState>) -> Result<(), String> {
    state.batch.resume().await
}

/// Retry failed batch tasks
#[tauri::command]
pub async fn batch_retry_failed(state: State<'_, AppState>) -> Result<(), String> {
    let service = state.translation.service.clone();
    state.batch.retry_failed(service).await
}

/// Get batch progress
#[tauri::command]
pub async fn batch_get_progress(state: State<'_, AppState>) -> Result<BatchProgress, String> {
    Ok(state.batch.get_progress().await)
}

/// Get batch results
#[tauri::command]
pub async fn batch_get_results(state: State<'_, AppState>) -> Result<Vec<BatchTask>, String> {
    Ok(state.batch.get_results().await)
}

/// Get batch job status
#[tauri::command]
pub async fn batch_get_status(state: State<'_, AppState>) -> Result<BatchJobStatus, String> {
    Ok(state.batch.get_status().await)
}

/// Reset batch manager
#[tauri::command]
pub async fn batch_reset(state: State<'_, AppState>) -> Result<(), String> {
    state.batch.reset().await;
    Ok(())
}

/// Export translation memory as JSON
#[tauri::command]
pub async fn tm_export(
    from_lang: Option<String>,
    to_lang: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let history = state.document.history.lock().await;
    let data = history.export_tm(from_lang.as_deref(), to_lang.as_deref());
    serde_json::to_string_pretty(&data).map_err(|e| e.to_string())
}

/// Import translation memory from JSON
#[tauri::command]
pub async fn tm_import(
    json: String,
    deduplicate: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(usize, usize), String> {
    let data: TmExportData =
        serde_json::from_str(&json).map_err(|e| format!("Invalid TM JSON: {}", e))?;
    let history = state.document.history.lock().await;
    let result = history.import_tm(&data, deduplicate.unwrap_or(true));
    Ok(result)
}

/// Get TM statistics
#[tauri::command]
pub async fn tm_get_stats(state: State<'_, AppState>) -> Result<crate::memory::TmStats, String> {
    let history = state.document.history.lock().await;
    Ok(history.get_tm_stats())
}

/// Search TM entries
#[tauri::command]
pub async fn tm_search(
    query: String,
    from_lang: Option<String>,
    to_lang: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    state: State<'_, AppState>,
) -> Result<(Vec<crate::memory::TmExportEntry>, usize), String> {
    let history = state.document.history.lock().await;
    Ok(history.search_tm(
        &query,
        from_lang.as_deref(),
        to_lang.as_deref(),
        limit.unwrap_or(50),
        offset.unwrap_or(0),
    ))
}
