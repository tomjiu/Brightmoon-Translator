use crate::pdf::{
    self, BilingualPdfLayout, PdfDocument, PdfExtractOptions, PdfExtractionSidecarConfig, PdfPage,
    ScannedPdfOcrResult, TranslatedPage, TranslatedPdf,
};
use crate::pdf_il::{open_pdf_translation_cache, PdfOutputMode};
use crate::security;
use crate::AppState;
use tauri::{AppHandle, Emitter, State, Window};

async fn pdf_opts_from_state(state: &AppState) -> PdfExtractOptions {
    let cfg = state.system.config.lock().await;
    PdfExtractOptions {
        engine: cfg.pdf_extraction_engine.clone(),
        sidecar: PdfExtractionSidecarConfig {
            mineru_cmd: cfg.pdf_extraction_sidecar.mineru_cmd.clone(),
            marker_cmd: cfg.pdf_extraction_sidecar.marker_cmd.clone(),
            ocrmypdf_cmd: cfg.pdf_extraction_sidecar.ocrmypdf_cmd.clone(),
        },
        max_ocr_pages: Some(40),
        ocr_lang: None,
    }
}

#[tauri::command]
pub async fn open_pdf(
    state: State<'_, AppState>,
    file_path: String,
) -> Result<PdfDocument, String> {
    security::validate_file_path(&file_path)?;
    let opts = pdf_opts_from_state(&state).await;
    let path = file_path.clone();
    let mut doc = tokio::task::spawn_blocking(move || {
        pdf::extract_text_from_pdf_with_options(&path, &opts)
    })
    .await
    .map_err(|e| format!("PDF extract join error: {e}"))??;

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
    app: AppHandle,
    file_path: String,
    from_lang: String,
    to_lang: String,
) -> Result<TranslatedPdf, String> {
    security::validate_file_path(&file_path)?;
    security::validate_language_code(&from_lang)?;
    security::validate_language_code(&to_lang)?;
    let opts = pdf_opts_from_state(&state).await;
    let path = file_path.clone();
    let doc = tokio::task::spawn_blocking(move || {
        pdf::extract_text_from_pdf_with_options(&path, &opts)
    })
    .await
    .map_err(|e| format!("PDF extract join error: {e}"))??;

    // P10: Open the persistent translation cache (best-effort — failures
    // just skip caching, translation still works).
    let cache = open_pdf_translation_cache(&app).ok();
    let cache_engine = "pdf-batch";

    // Build translated pages skeleton
    let mut translated_pages: Vec<TranslatedPage> = doc
        .pages
        .iter()
        .map(|p| TranslatedPage {
            page_number: p.page_number,
            original_text: p.text.clone(),
            translated_text: String::new(),
        })
        .collect();

    // P10: Check cache for each non-empty page; collect only cache-miss
    // pages for actual translation.
    let mut pages_to_translate: Vec<(usize, &str)> = Vec::new();
    let mut cache_hits = 0u32;
    for (i, page) in doc.pages.iter().enumerate() {
        let trimmed = page.text.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(ref cache) = cache {
            if let Ok(Some(entry)) = cache.lookup(cache_engine, &from_lang, &to_lang, trimmed) {
                if let Some(tp) = translated_pages.get_mut(i) {
                    tp.translated_text = entry.translated_text;
                    cache_hits += 1;
                    continue;
                }
            }
        }
        pages_to_translate.push((i, trimmed));
    }

    if cache_hits > 0 {
        tracing::info!(
            "[P10] PDF cache: {} hits, {} misses (translating)",
            cache_hits,
            pages_to_translate.len()
        );
    }

    // Translate only cache-miss pages
    let batch_results = if pages_to_translate.is_empty() {
        Vec::new()
    } else {
        state
            .translation
            .service
            .run_batch(
                crate::models::translation::TranslateChannel::Document,
                &pages_to_translate,
                &from_lang,
                &to_lang,
                2,
            )
            .await
    };

    // Apply results + P10: store new translations in cache
    for result in batch_results {
        if let Some(page) = translated_pages.get_mut(result.index) {
            page.translated_text.clone_from(&result.translated);
            // Store in cache (best-effort)
            if let Some(ref cache) = cache {
                let _ = cache.store(
                    cache_engine,
                    &from_lang,
                    &to_lang,
                    &result.original,
                    &result.translated,
                );
            }
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

/// S5-1: Save a bilingual PDF file. Generates a new PDF containing both
/// original and translated text for each page, using system-installed CJK
/// fonts (no font embedding needed). Layout is side-by-side by default.
#[tauri::command]
pub async fn save_bilingual_pdf(
    output_path: String,
    translated_pages: Vec<TranslatedPage>,
    layout: Option<BilingualPdfLayout>,
) -> Result<(), String> {
    security::validate_output_path(&output_path)?;
    let layout = layout.unwrap_or_default();
    let out = output_path.clone();
    let pages = translated_pages.clone();
    tokio::task::spawn_blocking(move || pdf::write_bilingual_pdf(&out, &pages, layout))
        .await
        .map_err(|e| format!("PDF write join error: {e}"))?
}

/// P9: Save a translated PDF in Mono or Dual mode.
///
/// - `Mono`: each page contains only the translated text (replaces original).
/// - `Dual`: each original page is followed by an interleaved translation
///   page, producing a bilingual PDF where original and translation alternate.
///
/// Uses system-installed CJK fonts (same resolution path as `save_bilingual_pdf`).
#[tauri::command]
pub async fn save_translated_pdf(
    output_path: String,
    translated_pages: Vec<TranslatedPage>,
    mode: Option<PdfOutputMode>,
) -> Result<(), String> {
    security::validate_output_path(&output_path)?;
    let mode = mode.unwrap_or(PdfOutputMode::Mono);
    let out = output_path.clone();
    let pages = translated_pages.clone();
    tokio::task::spawn_blocking(move || pdf::write_translated_pdf(&out, &pages, mode))
        .await
        .map_err(|e| format!("PDF write join error: {e}"))?
}

// ── P10: SQLite PDF translation cache commands ──────────────────────────
//
// Persistent cache keyed by (engine, source_lang, target_lang, text_hash).
// Avoids re-translating the same PDF pages across sessions.

/// P10: Total number of cached PDF translations.
#[tauri::command]
pub async fn pdf_cache_count(app: AppHandle) -> Result<usize, String> {
    let cache = open_pdf_translation_cache(&app)?;
    cache.count()
}

/// P10: Clear all cached PDF translations. Returns count deleted.
#[tauri::command]
pub async fn pdf_cache_clear(app: AppHandle) -> Result<usize, String> {
    let cache = open_pdf_translation_cache(&app)?;
    cache.clear()
}

/// P10: Evict cache entries older than `max_age_seconds`.
#[tauri::command]
pub async fn pdf_cache_evict(app: AppHandle, max_age_seconds: i64) -> Result<usize, String> {
    let cache = open_pdf_translation_cache(&app)?;
    cache.evict_older_than(max_age_seconds)
}

/// P10: Look up a cached translation for a single text.
/// Returns the cached translation text, or null if not found.
#[tauri::command]
pub async fn pdf_cache_lookup(
    app: AppHandle,
    engine: String,
    source_lang: String,
    target_lang: String,
    text: String,
) -> Result<Option<String>, String> {
    let cache = open_pdf_translation_cache(&app)?;
    let entry = cache.lookup(&engine, &source_lang, &target_lang, &text)?;
    Ok(entry.map(|e| e.translated_text))
}
