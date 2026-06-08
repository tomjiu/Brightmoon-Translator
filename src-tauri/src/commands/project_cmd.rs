use crate::project::{ProjectFile, ProjectStore, TranslationProject, TranslationSegment, ProjectExportData};
use crate::AppState;
use serde::Deserialize;
use tauri::State;

/// Input for creating a new project
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectInput {
    pub name: String,
    pub description: Option<String>,
    pub source_lang: Option<String>,
    pub target_lang: Option<String>,
}

/// Input for updating a project
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub source_lang: Option<String>,
    pub target_lang: Option<String>,
    pub status: Option<String>,
}

/// Input for adding a file to project
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddFileInput {
    pub file_name: String,
    pub file_path: String,
    pub file_type: String,
    pub file_size: i64,
}

/// Input for adding segments
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSegmentsInput {
    pub segments: Vec<SegmentInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentInput {
    pub index: String,
    pub source_text: String,
}

/// Input for updating a segment
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSegmentInput {
    pub translated_text: String,
    pub status: Option<String>,
}

// ==================== Project Commands ====================

#[tauri::command]
pub async fn create_project(
    _state: State<'_, AppState>,
    input: CreateProjectInput,
) -> Result<TranslationProject, String> {
    let store = ProjectStore::load();
    store.create_project(
        &input.name,
        input.description.as_deref().unwrap_or(""),
        input.source_lang.as_deref().unwrap_or("auto"),
        input.target_lang.as_deref().unwrap_or("zh"),
    )
}

#[tauri::command]
pub async fn get_project(
    _state: State<'_, AppState>,
    id: String,
) -> Result<TranslationProject, String> {
    let store = ProjectStore::load();
    store.get_project(&id)
}

#[tauri::command]
pub async fn get_all_projects(
    _state: State<'_, AppState>,
) -> Result<Vec<TranslationProject>, String> {
    let store = ProjectStore::load();
    store.get_all_projects()
}

#[tauri::command]
pub async fn update_project(
    _state: State<'_, AppState>,
    id: String,
    input: UpdateProjectInput,
) -> Result<TranslationProject, String> {
    let store = ProjectStore::load();
    store.update_project(
        &id,
        input.name.as_deref(),
        input.description.as_deref(),
        input.source_lang.as_deref(),
        input.target_lang.as_deref(),
        input.status.as_deref(),
    )
}

#[tauri::command]
pub async fn delete_project(
    _state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let store = ProjectStore::load();
    store.delete_project(&id)
}

// ==================== File Commands ====================

#[tauri::command]
pub async fn add_file_to_project(
    _state: State<'_, AppState>,
    project_id: String,
    input: AddFileInput,
) -> Result<ProjectFile, String> {
    let store = ProjectStore::load();
    store.add_file_to_project(
        &project_id,
        &input.file_name,
        &input.file_path,
        &input.file_type,
        input.file_size,
    )
}

#[tauri::command]
pub async fn get_project_files(
    _state: State<'_, AppState>,
    project_id: String,
) -> Result<Vec<ProjectFile>, String> {
    let store = ProjectStore::load();
    store.get_project_files(&project_id)
}

#[tauri::command]
pub async fn delete_file(
    _state: State<'_, AppState>,
    file_id: String,
) -> Result<(), String> {
    let store = ProjectStore::load();
    store.delete_file(&file_id)
}

#[tauri::command]
pub async fn update_file_status(
    _state: State<'_, AppState>,
    file_id: String,
    status: String,
    total_segments: Option<i32>,
    translated_segments: Option<i32>,
) -> Result<ProjectFile, String> {
    let store = ProjectStore::load();
    store.update_file_status(&file_id, &status, total_segments, translated_segments)
}

// ==================== Segment Commands ====================

#[tauri::command]
pub async fn add_segments(
    _state: State<'_, AppState>,
    file_id: String,
    input: AddSegmentsInput,
) -> Result<Vec<TranslationSegment>, String> {
    let store = ProjectStore::load();
    let segments: Vec<(String, String)> = input
        .segments
        .into_iter()
        .map(|s| (s.index, s.source_text))
        .collect();
    store.add_segments(&file_id, segments)
}

#[tauri::command]
pub async fn get_file_segments(
    _state: State<'_, AppState>,
    file_id: String,
) -> Result<Vec<TranslationSegment>, String> {
    let store = ProjectStore::load();
    store.get_file_segments(&file_id)
}

#[tauri::command]
pub async fn update_segment(
    _state: State<'_, AppState>,
    segment_id: String,
    input: UpdateSegmentInput,
) -> Result<TranslationSegment, String> {
    let store = ProjectStore::load();
    store.update_segment(
        &segment_id,
        &input.translated_text,
        input.status.as_deref().unwrap_or("translated"),
    )
}

// ==================== Export Commands ====================

#[tauri::command]
pub async fn export_project(
    _state: State<'_, AppState>,
    project_id: String,
) -> Result<ProjectExportData, String> {
    let store = ProjectStore::load();
    store.get_project_export_data(&project_id)
}

#[tauri::command]
pub async fn export_project_json(
    _state: State<'_, AppState>,
    project_id: String,
) -> Result<String, String> {
    let store = ProjectStore::load();
    let data = store.get_project_export_data(&project_id)?;
    serde_json::to_string_pretty(&data).map_err(|e| format!("Failed to serialize: {}", e))
}
