use crate::pdf::{self, PdfDocument, PdfPage, ScannedPdfOcrResult, TranslatedPage, TranslatedPdf};
use crate::security;
use crate::AppState;
use tauri::{Emitter, State, Window};

#[tauri::command]
pub async fn open_pdf(file_path: String) -> Result<PdfDocument, String> {
    security::validate_file_path(&file_path)?;
    let mut doc = pdf::extract_text_from_pdf(&file_path)?;

    // For scanned PDFs, get the actual page count via Windows.Data.Pdf
    if doc.is_scanned || doc.total_pages == 0 {
        #[cfg(target_os = "windows")]
        {
            match pdf::get_pdf_page_count(&file_path) {
                Ok(count) => {
                    doc.total_pages = count as usize;
                    tracing::info!("[PDF] Scanned PDF detected with {} pages", count);
                },
                Err(e) => {
                    tracing::warn!("[PDF] Failed to get page count: {}", e);
                },
            }
        }
    }

    Ok(doc)
}

#[tauri::command]
pub async fn translate_pdf(
    state: State<'_, AppState>,
    file_path: String,
    from_lang: String,
    to_lang: String,
) -> Result<TranslatedPdf, String> {
    security::validate_file_path(&file_path)?;
    security::validate_language_code(&from_lang)?;
    security::validate_language_code(&to_lang)?;
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
        .translation
        .service
        .run_batch(
            crate::models::translation::TranslateChannel::Document,
            &pages_to_translate,
            &from_lang,
            &to_lang,
            2,
        )
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
        is_scanned: doc.is_scanned,
    })
}

/// OCR a scanned PDF: render each page and run OCR to extract text.
#[tauri::command]
pub async fn ocr_scanned_pdf(
    file_path: String,
    lang: Option<String>,
    window: Window,
) -> Result<ScannedPdfOcrResult, String> {
    security::validate_file_path(&file_path)?;

    #[cfg(target_os = "windows")]
    {
        let page_count = pdf::get_pdf_page_count(&file_path)?;
        if page_count == 0 {
            return Err("PDF has no pages".to_string());
        }

        tracing::info!(
            "[PDF OCR] Starting OCR on scanned PDF: {} pages, lang={:?}",
            page_count,
            lang
        );

        let mut pages = Vec::new();
        let mut processed = 0usize;

        for i in 0..page_count {
            // Emit progress event
            let _ = window.emit(
                "pdf-ocr-progress",
                serde_json::json!({
                    "current": i + 1,
                    "total": page_count,
                }),
            );

            tracing::info!("[PDF OCR] Processing page {}/{}", i + 1, page_count);

            // Render page to image
            match pdf::render_pdf_page_to_png(&file_path, i) {
                Ok(png_bytes) => {
                    // Run OCR on rendered page image
                    match crate::ocr_engine::run_winrt_ocr(&png_bytes, lang.as_deref()) {
                        Ok(Some(text)) => {
                            tracing::info!(
                                "[PDF OCR] Page {} OCR success: {} chars",
                                i + 1,
                                text.len()
                            );
                            pages.push(PdfPage {
                                page_number: (i + 1) as usize,
                                text,
                            });
                            processed += 1;
                        },
                        Ok(None) => {
                            tracing::info!("[PDF OCR] Page {} OCR returned empty", i + 1);
                            pages.push(PdfPage {
                                page_number: (i + 1) as usize,
                                text: String::new(),
                            });
                            processed += 1;
                        },
                        Err(e) => {
                            tracing::warn!("[PDF OCR] Page {} OCR failed: {}", i + 1, e);
                            pages.push(PdfPage {
                                page_number: (i + 1) as usize,
                                text: String::new(),
                            });
                        },
                    }
                },
                Err(e) => {
                    tracing::warn!("[PDF OCR] Page {} render failed: {}", i + 1, e);
                    pages.push(PdfPage {
                        page_number: (i + 1) as usize,
                        text: String::new(),
                    });
                },
            }
        }

        // Emit completion event
        let _ = window.emit(
            "pdf-ocr-progress",
            serde_json::json!({
                "current": page_count,
                "total": page_count,
                "done": true,
            }),
        );

        tracing::info!(
            "[PDF OCR] Completed: {}/{} pages processed",
            processed,
            page_count
        );

        Ok(ScannedPdfOcrResult {
            pages,
            total_pages: page_count as usize,
            processed_pages: processed,
        })
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (file_path, lang, window);
        Err("Scanned PDF OCR requires Windows".to_string())
    }
}
