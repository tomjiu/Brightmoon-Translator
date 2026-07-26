use crate::security;
use crate::subtitle::{self, SubtitleDocument, TranslatedSubtitle};
use crate::AppState;
use tauri::{Emitter, State, Window};

#[tauri::command]
pub async fn open_subtitle(file_path: String) -> Result<SubtitleDocument, String> {
    security::validate_file_path(&file_path)?;
    subtitle::extract_text_from_subtitle(&file_path)
}

#[tauri::command]
pub async fn translate_subtitle(
    state: State<'_, AppState>,
    window: Window,
    file_path: String,
    from_lang: String,
    to_lang: String,
) -> Result<TranslatedSubtitle, String> {
    security::validate_file_path(&file_path)?;
    security::validate_language_code(&from_lang)?;
    security::validate_language_code(&to_lang)?;
    let mut doc = subtitle::extract_text_from_subtitle(&file_path)?;

    // Collect non-empty entries for batch translation
    let entries_to_translate: Vec<(usize, &str)> = doc
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.original_text.trim().is_empty())
        .map(|(i, e)| (i, e.original_text.trim()))
        .collect();

    let total = doc.entries.len();

    // Batch via façade; progress emitted after each wave of results
    let batch_results = state
        .translation
        .service
        .run_batch(
            crate::models::translation::TranslateChannel::Subtitle,
            &entries_to_translate,
            &from_lang,
            &to_lang,
            3,
        )
        .await;
    let _ = window.emit(
        "subtitle-progress",
        serde_json::json!({
            "current": batch_results.len().min(total),
            "total": total,
            "text": format!("Translating... {}/{}", batch_results.len().min(total), total),
        }),
    );

    // Apply results back to entries
    for result in batch_results {
        if let Some(entry) = doc.entries.get_mut(result.index) {
            entry.translated_text = result.translated;
        }
    }

    // Emit completion event
    let _ = window.emit(
        "subtitle-progress",
        serde_json::json!({
            "current": total,
            "total": total,
            "text": "Done",
        }),
    );

    Ok(TranslatedSubtitle {
        entries: doc.entries,
        total_entries: doc.total_entries,
        format: doc.format,
    })
}

#[tauri::command]
pub async fn export_subtitle_file(
    entries: Vec<subtitle::SubtitleEntry>,
    format: String,
    output_path: String,
    bilingual: bool,
) -> Result<String, String> {
    security::validate_output_path(&output_path)?;

    if entries.is_empty() {
        return Err("No subtitle entries to export".into());
    }

    let format = match format.to_lowercase().as_str() {
        "srt" | "vtt" | "lrc" | "ass" | "ssa" => format.to_lowercase(),
        other if other.is_empty() => "srt".into(),
        _ => "srt".into(),
    };

    let doc = SubtitleDocument {
        total_entries: entries.len(),
        entries,
        format,
    };
    let content = subtitle::export_subtitle(&doc, bilingual);

    std::fs::write(&output_path, content)
        .map_err(|e| format!("Failed to write subtitle file: {}", e))?;

    Ok(output_path)
}

#[tauri::command]
pub async fn translate_subtitle_text(
    state: State<'_, AppState>,
    text: String,
    from_lang: String,
    to_lang: String,
) -> Result<String, String> {
    state
        .translation
        .service
        .run_primary(
            crate::models::translation::TranslateChannel::Subtitle,
            &text,
            &from_lang,
            &to_lang,
        )
        .await
        .map_err(|e| e.to_string())
}
