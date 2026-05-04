use crate::subtitle::{self, SubtitleDocument, TranslatedSubtitle};
use crate::AppState;
use tauri::{Emitter, State, Window};

#[tauri::command]
pub async fn open_subtitle(file_path: String) -> Result<SubtitleDocument, String> {
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

    // Use batch translation with progress
    let window_clone = window.clone();
    let batch_results = state
        .translation_service
        .translate_embedded_batch(
            &entries_to_translate
                .iter()
                .map(|(_, text)| *text)
                .collect::<Vec<_>>()
                .join("\n"),
            &from_lang,
            &to_lang,
            3, // concurrency
            |completed, _total| {
                let _ = window_clone.emit("subtitle-progress", serde_json::json!({
                    "current": completed,
                    "total": total,
                    "text": format!("Translating... {}/{}", completed, total),
                }));
            },
        )
        .await;

    // Apply results back to entries
    for result in batch_results {
        if let Some(entry) = doc.entries.get_mut(result.index) {
            entry.translated_text = result.translated;
        }
    }

    // Emit completion event
    let _ = window.emit("subtitle-progress", serde_json::json!({
        "current": total,
        "total": total,
        "text": "Done",
    }));

    Ok(TranslatedSubtitle {
        entries: doc.entries,
        total_entries: doc.total_entries,
        format: doc.format,
    })
}

#[tauri::command]
pub async fn export_subtitle_file(
    file_path: String,
    output_path: String,
    bilingual: bool,
) -> Result<String, String> {
    let doc = subtitle::extract_text_from_subtitle(&file_path)?;
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
        .translation_service
        .translate_primary(&text, &from_lang, &to_lang)
        .await
        .map_err(|e| e.to_string())
}
