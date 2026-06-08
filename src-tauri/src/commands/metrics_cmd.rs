use crate::metrics::{HourlyStats, MetricsEvent, MetricsSummary, MetricsTimeline};
use crate::AppState;
use tauri::State;

/// Get aggregated metrics summary
#[tauri::command]
pub async fn get_metrics_summary(
    state: State<'_, AppState>,
) -> Result<MetricsSummary, String> {
    Ok(state.translation.metrics.summary().await)
}

/// Get recent timeline data for charts
#[tauri::command]
pub async fn get_metrics_timeline(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<MetricsTimeline>, String> {
    let limit = limit.unwrap_or(500);
    Ok(state.translation.metrics.get_timeline(limit).await)
}

/// Get hourly aggregated stats
#[tauri::command]
pub async fn get_metrics_hourly_stats(
    state: State<'_, AppState>,
    hours: Option<i64>,
) -> Result<Vec<HourlyStats>, String> {
    let hours = hours.unwrap_or(24);
    Ok(state.translation.metrics.get_hourly_stats(hours).await)
}

/// Export all metrics as CSV
#[tauri::command]
pub async fn export_metrics_csv(
    state: State<'_, AppState>,
) -> Result<String, String> {
    Ok(state.translation.metrics.export_csv().await)
}

/// Export all metrics as JSON
#[tauri::command]
pub async fn export_metrics_json(
    state: State<'_, AppState>,
) -> Result<Vec<MetricsEvent>, String> {
    Ok(state.translation.metrics.export_json().await)
}

/// Clear all metrics data
#[tauri::command]
pub async fn clear_metrics(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.translation.metrics.clear().await;
    Ok(())
}

/// Prune old metrics data
#[tauri::command]
pub async fn prune_metrics(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.translation.metrics.prune().await;
    Ok(())
}
