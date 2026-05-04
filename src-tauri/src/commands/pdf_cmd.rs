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

    let mut translated_pages = Vec::new();

    for page in &doc.pages {
        if page.text.trim().is_empty() {
            translated_pages.push(TranslatedPage {
                page_number: page.page_number,
                original_text: page.text.clone(),
                translated_text: String::new(),
            });
            continue;
        }

        // Translate using primary engine
        let router = state.engine_router.read().await;
        let translated = router
            .translate_primary(&page.text, &from_lang, &to_lang)
            .await
            .unwrap_or_else(|e| {
                eprintln!("Failed to translate page {}: {}", page.page_number, e);
                String::new()
            });
        drop(router);

        translated_pages.push(TranslatedPage {
            page_number: page.page_number,
            original_text: page.text.clone(),
            translated_text: translated,
        });
    }

    Ok(TranslatedPdf {
        pages: translated_pages,
        total_pages: doc.total_pages,
    })
}
