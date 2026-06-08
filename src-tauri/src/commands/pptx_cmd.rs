use crate::pptx::{self, PptxDocument, PptxTranslationResult, TranslatedPptx, TranslatedTextBlock, TranslatedSlide};
use crate::security;
use crate::AppState;
use tauri::{Emitter, State, Window};

#[tauri::command]
pub async fn open_pptx(file_path: String) -> Result<PptxDocument, String> {
    security::validate_file_path(&file_path)?;
    pptx::extract_text_from_pptx(&file_path)
}

#[tauri::command]
pub async fn translate_pptx(
    state: State<'_, AppState>,
    window: Window,
    input_path: String,
    output_path: String,
    from_lang: String,
    to_lang: String,
) -> Result<PptxTranslationResult, String> {
    security::validate_file_path(&input_path)?;
    security::validate_output_path(&output_path)?;
    security::validate_language_code(&from_lang)?;
    security::validate_language_code(&to_lang)?;
    // Extract text
    let doc = pptx::extract_text_from_pptx(&input_path)?;

    if doc.slides.is_empty() {
        return Ok(PptxTranslationResult {
            input_path,
            output_path,
            slides_translated: 0,
            text_blocks_translated: 0,
            words_translated: 0,
            success: true,
            error_message: Some("No translatable content found".to_string()),
        });
    }

    // Emit progress event
    let _ = window.emit("pptx-progress", serde_json::json!({
        "stage": "extracting",
        "totalSlides": doc.total_slides,
        "totalTextBlocks": doc.total_text_blocks,
        "totalWords": doc.total_words,
    }));

    // Prepare text blocks for batch translation
    let mut blocks_to_translate: Vec<(usize, &str)> = Vec::new();
    for slide in &doc.slides {
        for block in &slide.text_blocks {
            if !block.text.trim().is_empty() {
                blocks_to_translate.push((blocks_to_translate.len(), block.text.trim()));
            }
        }
    }

    // Emit translation start
    let _ = window.emit("pptx-progress", serde_json::json!({
        "stage": "translating",
        "blocksToTranslate": blocks_to_translate.len(),
    }));

    // Use batch translation
    let batch_results = state
        .translation
        .service
        .translate_batch(&blocks_to_translate, &from_lang, &to_lang, 2)
        .await;

    // Emit write progress
    let _ = window.emit("pptx-progress", serde_json::json!({
        "stage": "writing",
    }));

    // Build translations mapping
    let translations: Vec<(String, String)> = batch_results
        .into_iter()
        .map(|r| {
            // Find the block ID for this index
            let mut block_idx = 0;
            for slide in &doc.slides {
                for block in &slide.text_blocks {
                    if !block.text.trim().is_empty() {
                        if block_idx == r.index {
                            return (block.id.clone(), r.translated);
                        }
                        block_idx += 1;
                    }
                }
            }
            (format!("block_{}", r.index), r.translated)
        })
        .collect();

    // Write translated PPTX
    let result = pptx::write_translated_pptx(&input_path, &output_path, &translations)?;

    // Emit completion
    let _ = window.emit("pptx-progress", serde_json::json!({
        "stage": "completed",
        "slidesTranslated": result.slides_translated,
        "textBlocksTranslated": result.text_blocks_translated,
        "wordsTranslated": result.words_translated,
    }));

    Ok(result)
}

#[tauri::command]
pub async fn translate_pptx_preview(
    state: State<'_, AppState>,
    input_path: String,
    from_lang: String,
    to_lang: String,
) -> Result<TranslatedPptx, String> {
    security::validate_file_path(&input_path)?;
    security::validate_language_code(&from_lang)?;
    security::validate_language_code(&to_lang)?;
    let doc = pptx::extract_text_from_pptx(&input_path)?;

    // Prepare text blocks for batch translation
    let mut blocks_to_translate: Vec<(usize, &str)> = Vec::new();
    for slide in &doc.slides {
        for block in &slide.text_blocks {
            if !block.text.trim().is_empty() {
                blocks_to_translate.push((blocks_to_translate.len(), block.text.trim()));
            }
        }
    }

    // Use batch translation
    let batch_results = state
        .translation
        .service
        .translate_batch(&blocks_to_translate, &from_lang, &to_lang, 2)
        .await;

    // Build translated slides
    let mut translated_slides: Vec<TranslatedSlide> = Vec::new();
    let mut result_iter = batch_results.into_iter();

    for slide in &doc.slides {
        let mut text_blocks: Vec<TranslatedTextBlock> = Vec::new();

        for block in &slide.text_blocks {
            let translated_text = if !block.text.trim().is_empty() {
                if let Some(result) = result_iter.next() {
                    result.translated
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            text_blocks.push(TranslatedTextBlock {
                id: block.id.clone(),
                original_text: block.text.clone(),
                translated_text,
                slide_index: block.slide_index,
            });
        }

        translated_slides.push(TranslatedSlide {
            index: slide.index,
            name: slide.name.clone(),
            text_blocks,
        });
    }

    Ok(TranslatedPptx {
        title: doc.title,
        slides: translated_slides,
        total_slides: doc.total_slides,
        total_text_blocks: doc.total_text_blocks,
        total_words: doc.total_words,
    })
}
