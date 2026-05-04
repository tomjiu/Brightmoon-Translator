use crate::engine::llm::TranslationContext;
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
    let total = doc.entries.len();
    let mut context: Vec<TranslationContext> = Vec::new();

    for (i, entry) in doc.entries.iter_mut().enumerate() {
        if entry.original_text.trim().is_empty() {
            continue;
        }

        // Emit progress event
        let _ = window.emit("subtitle-progress", serde_json::json!({
            "current": i + 1,
            "total": total,
            "text": &entry.original_text,
        }));

        // Translate using primary engine with context
        let router = state.engine_router.read().await;
        let translated = router
            .translate_primary_with_context(
                &entry.original_text,
                &from_lang,
                &to_lang,
                &context,
            )
            .await
            .unwrap_or_else(|e| {
                eprintln!("Failed to translate subtitle {}: {}", entry.index, e);
                String::new()
            });
        drop(router);

        entry.translated_text = translated.clone();

        // Add to context for next translation (keep last 5)
        context.push(TranslationContext {
            source: entry.original_text.clone(),
            translation: translated,
        });
        if context.len() > 5 {
            context.remove(0);
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
    let router = state.engine_router.read().await;
    router
        .translate_primary(&text, &from_lang, &to_lang)
        .await
        .map_err(|e| format!("Translation failed: {}", e))
}
