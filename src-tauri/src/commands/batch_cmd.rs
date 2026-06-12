/**
 * Batch Translation Commands
 *
 * Tauri commands for batch translation queue management
 */
use crate::batch::{BatchConfig, BatchJobStatus, BatchProgress, BatchTask};
use crate::memory::TmExportData;
use crate::security;
use crate::tmx;
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
    // Limit JSON input size to prevent DoS
    security::validate_text_length(&json, 10_000_000)?; // 10MB limit for TM import

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
    // Validate language codes if provided
    if let Some(ref from) = from_lang {
        security::validate_language_code(from)?;
    }
    if let Some(ref to) = to_lang {
        security::validate_language_code(to)?;
    }

    // Clamp limit to reasonable range
    let safe_limit = limit.unwrap_or(50).min(500);
    let safe_offset = offset.unwrap_or(0);

    let history = state.document.history.lock().await;
    Ok(history.search_tm(
        &query,
        from_lang.as_deref(),
        to_lang.as_deref(),
        safe_limit,
        safe_offset,
    ))
}

/// Export translation memory as TMX format
#[tauri::command]
pub async fn tm_export_tmx(
    from_lang: Option<String>,
    to_lang: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let history = state.document.history.lock().await;
    let data = history.export_tm(from_lang.as_deref(), to_lang.as_deref());

    // Convert TM entries to TMX units
    let units: Vec<tmx::TmxTranslationUnit> = data
        .entries
        .iter()
        .map(|entry| tmx::TmxTranslationUnit {
            source_text: entry.source.clone(),
            target_text: entry.target.clone(),
            source_lang: entry.from_lang.clone(),
            target_lang: entry.to_lang.clone(),
            creation_date: Some(
                chrono::DateTime::from_timestamp_millis(entry.timestamp)
                    .map(|dt| dt.format("%Y%m%dT%H%M%SZ").to_string())
                    .unwrap_or_default(),
            ),
            change_date: None,
            creation_user: Some(entry.engine.clone()),
            note: None,
        })
        .collect();

    let source_lang = from_lang.as_deref().unwrap_or("en");
    tmx::export_tmx(&units, source_lang, "MoonTranslator")
        .map_err(|e| format!("TMX export error: {}", e))
}

/// Import translation memory from TMX format
#[tauri::command]
pub async fn tm_import_tmx(
    xml: String,
    deduplicate: Option<bool>,
    state: State<'_, AppState>,
) -> Result<(usize, usize), String> {
    security::validate_text_length(&xml, 10_000_000)?; // 10MB limit

    let data = tmx::parse_tmx(&xml).map_err(|e| format!("TMX parse error: {}", e))?;

    // Convert TMX units to TM export format
    let entries: Vec<crate::memory::TmExportEntry> = data
        .units
        .iter()
        .map(|unit| crate::memory::TmExportEntry {
            source: unit.source_text.clone(),
            target: unit.target_text.clone(),
            from_lang: unit.source_lang.clone(),
            to_lang: unit.target_lang.clone(),
            engine: unit
                .creation_user
                .clone()
                .unwrap_or_else(|| "tmx-import".to_string()),
            timestamp: unit
                .creation_date
                .as_ref()
                .and_then(|d| parse_tmx_date(d))
                .unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
        })
        .collect();

    let tm_data = TmExportData {
        version: 1,
        entries,
        exported_at: chrono::Utc::now().timestamp_millis(),
    };

    let history = state.document.history.lock().await;
    Ok(history.import_tm(&tm_data, deduplicate.unwrap_or(true)))
}

/// Delete a single TM entry by matching source/target/lang fields
#[tauri::command]
pub async fn tm_delete(
    source: String,
    target: String,
    from_lang: String,
    to_lang: String,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let history = state.document.history.lock().await;
    Ok(history.delete_tm(&source, &target, &from_lang, &to_lang))
}

/// Bulk delete TM entries
#[tauri::command]
pub async fn tm_batch_delete(
    entries: Vec<(String, String, String, String)>,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    if entries.len() > 1000 {
        return Err("Too many entries to delete at once (max 1000)".to_string());
    }
    let history = state.document.history.lock().await;
    Ok(history.batch_delete_tm(&entries))
}

/// Parse TMX date format (YYYYMMDDTHHMMSSZ) to timestamp millis
fn parse_tmx_date(date_str: &str) -> Option<i64> {
    // Try TMX format: 20240101T120000Z
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, "%Y%m%dT%H%M%SZ") {
        return Some(dt.and_utc().timestamp_millis());
    }
    // Try ISO format: 2024-01-01T12:00:00Z
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date_str) {
        return Some(dt.timestamp_millis());
    }
    None
}
