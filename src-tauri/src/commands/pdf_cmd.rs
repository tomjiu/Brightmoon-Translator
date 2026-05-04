use crate::pdf::{self, PdfDocument, TranslatedPage, TranslatedPdf};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn open_pdf(file_path: String) -> Result<PdfDocument, String> {
    pdf::extract_text_from_pdf(&file_path)
}

#[tauri::command]
pub async fn translate_pdf(
    state: State<'_, AppState>,
    file_path: String,
    from_lang: String,
    to_lang: String,
) -> Result<TranslatedPdf, String> {
    let doc = pdf::extract_text_from_pdf(&file_path)?;

    // Collect non-empty pages for batch translation
    let pages_to_translate: Vec<(usize, &str)> = doc
        .pages
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.text.trim().is_empty())
        .map(|(i, p)| (i, p.text.trim()))
        .collect();

    // Use batch translation
    let batch_results = state
        .translation_service
        .translate_batch(&pages_to_translate, &from_lang, &to_lang, 2)
        .await;

    // Build translated pages
    let mut translated_pages: Vec<TranslatedPage> = doc
        .pages
        .iter()
        .map(|p| TranslatedPage {
            page_number: p.page_number,
            original_text: p.text.clone(),
            translated_text: String::new(),
        })
        .collect();

    // Apply results
    for result in batch_results {
        if let Some(page) = translated_pages.get_mut(result.index) {
            page.translated_text = result.translated;
        }
    }

    Ok(TranslatedPdf {
        pages: translated_pages,
        total_pages: doc.total_pages,
    })
}
