use crate::docx::{self, DocxDocument, DocxTranslationResult, TranslatedDocx, TranslatedParagraph};
use crate::security;
use crate::AppState;
use tauri::{Emitter, State, Window};

#[tauri::command]
pub async fn open_docx(file_path: String) -> Result<DocxDocument, String> {
    security::validate_file_path(&file_path)?;
    docx::extract_text_from_docx(&file_path)
}

#[tauri::command]
pub async fn translate_docx(
    state: State<'_, AppState>,
    window: Window,
    input_path: String,
    output_path: String,
    from_lang: String,
    to_lang: String,
) -> Result<DocxTranslationResult, String> {
    security::validate_file_path(&input_path)?;
    security::validate_output_path(&output_path)?;
    security::validate_language_code(&from_lang)?;
    security::validate_language_code(&to_lang)?;
    // Extract text
    let doc = docx::extract_text_from_docx(&input_path)?;

    if doc.paragraphs.is_empty() {
        return Ok(DocxTranslationResult {
            input_path,
            output_path,
            paragraphs_translated: 0,
            words_translated: 0,
            success: true,
            error_message: Some("No translatable content found".to_string()),
        });
    }

    // Emit progress event
    let _ = window.emit(
        "docx-progress",
        serde_json::json!({
            "stage": "extracting",
            "totalParagraphs": doc.total_paragraphs,
            "totalWords": doc.total_words,
        }),
    );

    // Prepare paragraphs for batch translation
    let paragraphs_to_translate: Vec<(usize, &str)> = doc
        .paragraphs
        .iter()
        .filter(|p| !p.text.trim().is_empty())
        .map(|p| (p.index, p.text.trim()))
        .collect();

    // Emit translation start
    let _ = window.emit(
        "docx-progress",
        serde_json::json!({
            "stage": "translating",
            "paragraphsToTranslate": paragraphs_to_translate.len(),
        }),
    );

    // Use batch translation
    let batch_results = state
        .translation
        .service
        .run_batch(
            crate::models::translation::TranslateChannel::Document,
            &paragraphs_to_translate,
            &from_lang,
            &to_lang,
            2,
        )
        .await;

    // Emit write progress
    let _ = window.emit(
        "docx-progress",
        serde_json::json!({
            "stage": "writing",
        }),
    );

    // Write translated DOCX
    let translations: Vec<(usize, String)> = batch_results
        .into_iter()
        .map(|r| (r.index, r.translated))
        .collect();

    let result = docx::write_translated_docx(&input_path, &output_path, &translations)?;

    // Emit completion
    let _ = window.emit(
        "docx-progress",
        serde_json::json!({
            "stage": "completed",
            "paragraphsTranslated": result.paragraphs_translated,
            "wordsTranslated": result.words_translated,
        }),
    );

    Ok(result)
}

#[tauri::command]
pub async fn translate_docx_preview(
    state: State<'_, AppState>,
    input_path: String,
    from_lang: String,
    to_lang: String,
) -> Result<TranslatedDocx, String> {
    security::validate_file_path(&input_path)?;
    security::validate_language_code(&from_lang)?;
    security::validate_language_code(&to_lang)?;
    let doc = docx::extract_text_from_docx(&input_path)?;

    // Prepare paragraphs for batch translation
    let paragraphs_to_translate: Vec<(usize, &str)> = doc
        .paragraphs
        .iter()
        .filter(|p| !p.text.trim().is_empty())
        .map(|p| (p.index, p.text.trim()))
        .collect();

    // Use batch translation
    let batch_results = state
        .translation
        .service
        .run_batch(
            crate::models::translation::TranslateChannel::Document,
            &paragraphs_to_translate,
            &from_lang,
            &to_lang,
            2,
        )
        .await;

    // Build translated paragraphs
    let mut translated_paragraphs: Vec<TranslatedParagraph> = doc
        .paragraphs
        .iter()
        .map(|p| TranslatedParagraph {
            index: p.index,
            original_text: p.text.clone(),
            translated_text: String::new(),
            style: p.style.clone(),
            is_heading: p.is_heading,
            heading_level: p.heading_level,
        })
        .collect();

    // Apply translations
    for result in batch_results {
        if let Some(para) = translated_paragraphs.get_mut(result.index) {
            para.translated_text = result.translated;
        }
    }

    Ok(TranslatedDocx {
        title: doc.title,
        paragraphs: translated_paragraphs,
        total_paragraphs: doc.total_paragraphs,
        total_words: doc.total_words,
    })
}
