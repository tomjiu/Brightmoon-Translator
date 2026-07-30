use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfPage {
    pub page_number: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfDocument {
    pub pages: Vec<PdfPage>,
    pub total_pages: usize,
    pub is_scanned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedPage {
    pub page_number: usize,
    pub original_text: String,
    pub translated_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedPdf {
    pub pages: Vec<TranslatedPage>,
    pub total_pages: usize,
    pub is_scanned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedPdfOcrResult {
    pub pages: Vec<PdfPage>,
    pub total_pages: usize,
    pub processed_pages: usize,
}

/// Sidecar CLI paths for external PDF extractors.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfExtractionSidecarConfig {
    #[serde(default)]
    pub mineru_cmd: String,
    #[serde(default)]
    pub marker_cmd: String,
    #[serde(default)]
    pub ocrmypdf_cmd: String,
}

/// Options for PDF text extraction (engine + optional sidecar paths).
#[derive(Debug, Clone)]
pub struct PdfExtractOptions {
    /// "pdf-extract" | "ocr" | "mineru" | "marker" | "ocrmypdf"
    pub engine: String,
    pub sidecar: PdfExtractionSidecarConfig,
    /// Cap OCR pages on auto-fallback (None = all pages).
    pub max_ocr_pages: Option<u32>,
    pub ocr_lang: Option<String>,
}

impl Default for PdfExtractOptions {
    fn default() -> Self {
        Self {
            engine: "pdf-extract".into(),
            sidecar: PdfExtractionSidecarConfig::default(),
            max_ocr_pages: Some(40),
            ocr_lang: None,
        }
    }
}

/// Threshold: if average chars per page is below this, consider the PDF as scanned.
const SCANNED_PDF_CHAR_THRESHOLD: usize = 50;

/// Detect garbled / letter-spaced digital PDF text (e.g. Fluent Python "P y t h o n").
pub fn is_text_garbled(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return false;
    }

    // 1. Scattered single Latin letters (letter-spacing artifacts)
    let scattered = words
        .iter()
        .filter(|w| {
            w.len() == 1
                && w.chars()
                    .next()
                    .map(|c| c.is_ascii_alphabetic())
                    .unwrap_or(false)
        })
        .count();
    if scattered > 5 {
        return true;
    }

    // 2. High ratio of 2–3 char alphabetic fragments
    let fragments = words
        .iter()
        .filter(|w| {
            (2..=3).contains(&w.len()) && w.chars().all(|c| c.is_ascii_alphabetic())
        })
        .count();
    let total_words = words.len().max(1);
    if fragments as f64 / total_words as f64 > 0.15 {
        return true;
    }

    // 3. Replacement / control chars (excluding whitespace)
    let bad = text
        .chars()
        .filter(|c| {
            *c == '\u{FFFD}'
                || (c.is_control() && *c != '\n' && *c != '\r' && *c != '\t' && *c != '\u{0c}')
        })
        .count();
    if bad > 0 {
        return true;
    }

    false
}

fn pages_from_raw_text(text: &str) -> Vec<PdfPage> {
    let page_texts: Vec<&str> = text.split('\x0c').collect();
    let mut pages = Vec::new();

    for (i, page_text) in page_texts.iter().enumerate() {
        let trimmed = page_text.trim();
        if !trimmed.is_empty() {
            pages.push(PdfPage {
                page_number: i + 1,
                text: trimmed.to_string(),
            });
        }
    }

    if pages.is_empty() && !text.trim().is_empty() {
        pages.push(PdfPage {
            page_number: 1,
            text: text.trim().to_string(),
        });
    }

    pages
}

fn finalize_document(pages: Vec<PdfPage>, force_scanned: Option<bool>) -> PdfDocument {
    let total_chars: usize = pages.iter().map(|p| p.text.len()).sum();
    let page_count = pages.len().max(1);
    let avg_chars_per_page = total_chars / page_count;
    let is_scanned = force_scanned.unwrap_or_else(|| {
        pages.is_empty()
            || (avg_chars_per_page < SCANNED_PDF_CHAR_THRESHOLD && total_chars < 200)
    });

    tracing::info!(
        "[PDF] Extracted {} chars across {} pages, avg {} chars/page, is_scanned={}",
        total_chars,
        pages.len(),
        avg_chars_per_page,
        is_scanned
    );

    if pages.is_empty() {
        return PdfDocument {
            pages: Vec::new(),
            total_pages: 0,
            is_scanned: true,
        };
    }

    let total_pages = pages.len();
    PdfDocument {
        pages,
        total_pages,
        is_scanned,
    }
}

/// Original pdf-extract path only (no quality gate).
pub fn extract_text_via_pdf_extract(file_path: &str) -> Result<PdfDocument, String> {
    let data = std::fs::read(file_path).map_err(|e| format!("Failed to read PDF file: {}", e))?;

    // Catch panics from pdf-extract on unsupported PDF features
    let text = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pdf_extract::extract_text_from_mem(&data)
    })) {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => {
            return Err(format!("Failed to extract text from PDF: {}", e));
        }
        Err(_) => {
            return Err(
                "pdf-extract panicked on this file (unsupported PDF feature)".to_string(),
            );
        }
    };

    let pages = pages_from_raw_text(&text);
    Ok(finalize_document(pages, None))
}

/// Render each page to PNG and OCR (Windows). Used for scanned + garbled digital PDFs.
#[cfg(target_os = "windows")]
pub fn extract_pages_via_ocr(
    file_path: &str,
    lang: Option<&str>,
    max_pages: Option<u32>,
) -> Result<PdfDocument, String> {
    let page_count = get_pdf_page_count(file_path)?;
    if page_count == 0 {
        return Err("PDF has no pages".to_string());
    }

    let limit = max_pages
        .map(|m| m.min(page_count))
        .unwrap_or(page_count);

    tracing::info!(
        "[PDF] OCR extract: {} pages (of {}), lang={:?}",
        limit,
        page_count,
        lang
    );

    let mut pages = Vec::new();
    for i in 0..limit {
        tracing::info!("[PDF] OCR page {}/{}", i + 1, limit);
        match render_pdf_page_to_png(file_path, i) {
            Ok(png_bytes) => match crate::ocr_engine::run_winrt_ocr(&png_bytes, lang) {
                Ok(Some(text)) => {
                    pages.push(PdfPage {
                        page_number: (i + 1) as usize,
                        text,
                    });
                }
                Ok(None) => {
                    pages.push(PdfPage {
                        page_number: (i + 1) as usize,
                        text: String::new(),
                    });
                }
                Err(e) => {
                    tracing::warn!("[PDF] page {} OCR failed: {}", i + 1, e);
                    pages.push(PdfPage {
                        page_number: (i + 1) as usize,
                        text: String::new(),
                    });
                }
            },
            Err(e) => {
                tracing::warn!("[PDF] page {} render failed: {}", i + 1, e);
                pages.push(PdfPage {
                    page_number: (i + 1) as usize,
                    text: String::new(),
                });
            }
        }
    }

    // Digital PDF rendered for OCR is not a "scanned" book
    Ok(finalize_document(pages, Some(false)))
}

#[cfg(not(target_os = "windows"))]
pub fn extract_pages_via_ocr(
    _file_path: &str,
    _lang: Option<&str>,
    _max_pages: Option<u32>,
) -> Result<PdfDocument, String> {
    Err("PDF page OCR fallback requires Windows".to_string())
}

fn command_exists(cmd: &str) -> bool {
    if cmd.is_empty() {
        return false;
    }
    let path = Path::new(cmd);
    if path.is_file() {
        return true;
    }
    // bare name on PATH
    Command::new(cmd)
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Run external PDF extractor CLI; returns plain text / markdown body.
pub fn run_pdf_sidecar(
    file_path: &str,
    engine: &str,
    sidecar: &PdfExtractionSidecarConfig,
) -> Result<String, String> {
    let tmp = std::env::temp_dir().join(format!(
        "moon_pdf_{}_{}",
        engine,
        std::process::id()
    ));
    let _ = std::fs::create_dir_all(&tmp);

    let (program, args, result_glob): (String, Vec<String>, Option<PathBuf>) = match engine {
        "mineru" => {
            let cmd = if sidecar.mineru_cmd.is_empty() {
                "magic-pdf".to_string()
            } else {
                sidecar.mineru_cmd.clone()
            };
            if !command_exists(&cmd) {
                return Err(format!("mineru CLI not found ({cmd})"));
            }
            (
                cmd,
                vec![
                    "-p".into(),
                    file_path.into(),
                    "-o".into(),
                    tmp.to_string_lossy().into(),
                ],
                Some(tmp.clone()),
            )
        }
        "marker" => {
            let cmd = if sidecar.marker_cmd.is_empty() {
                "marker_single".to_string()
            } else {
                sidecar.marker_cmd.clone()
            };
            if !command_exists(&cmd) {
                return Err(format!("marker CLI not found ({cmd})"));
            }
            let out_md = tmp.join("out.md");
            (
                cmd,
                vec![
                    file_path.into(),
                    "--output_format".into(),
                    "markdown".into(),
                    "--output_dir".into(),
                    tmp.to_string_lossy().into(),
                ],
                Some(out_md),
            )
        }
        "ocrmypdf" => {
            let cmd = if sidecar.ocrmypdf_cmd.is_empty() {
                "ocrmypdf".to_string()
            } else {
                sidecar.ocrmypdf_cmd.clone()
            };
            if !command_exists(&cmd) {
                return Err(format!("ocrmypdf CLI not found ({cmd})"));
            }
            let out_pdf = tmp.join("ocr_output.pdf");
            (
                cmd,
                vec![
                    "--force-ocr".into(),
                    "--output-type".into(),
                    "pdf".into(),
                    file_path.into(),
                    out_pdf.to_string_lossy().into(),
                ],
                Some(out_pdf),
            )
        }
        other => return Err(format!("Unknown PDF sidecar engine: {other}")),
    };

    tracing::info!("[PDF] running sidecar {} on {}", engine, file_path);

    let mut child = Command::new(&program)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn {engine} ({program}): {e}"))?;

    // Soft timeout ~120s
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    let err = child
                        .wait_with_output()
                        .ok()
                        .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
                        .unwrap_or_default();
                    return Err(format!("{engine} exited non-zero: {err}"));
                }
                break;
            }
            Ok(None) => {
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    return Err(format!("{engine} timed out after 120s"));
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(format!("{engine} wait error: {e}")),
        }
    }

    // Consume remaining output
    let _ = child.wait_with_output();

    match engine {
        "ocrmypdf" => {
            let out_pdf = tmp.join("ocr_output.pdf");
            if !out_pdf.exists() {
                return Err("ocrmypdf produced no output PDF".into());
            }
            // Re-extract text from OCR'd PDF via pdf-extract (should have text layer)
            let doc = extract_text_via_pdf_extract(out_pdf.to_str().unwrap_or(""))?;
            Ok(doc
                .pages
                .into_iter()
                .map(|p| p.text)
                .collect::<Vec<_>>()
                .join("\n\x0c\n"))
        }
        "mineru" | "marker" => {
            // Find first .md in output dir
            let text = find_first_markdown(&tmp)
                .or_else(|| {
                    result_glob
                        .as_ref()
                        .and_then(|p| std::fs::read_to_string(p).ok())
                })
                .ok_or_else(|| format!("{engine} produced no markdown output"))?;
            Ok(text)
        }
        _ => Err("unreachable".into()),
    }
}

fn find_first_markdown(dir: &Path) -> Option<String> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries = std::fs::read_dir(&d).ok()?;
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| x.eq_ignore_ascii_case("md"))
                .unwrap_or(false)
            {
                if let Ok(s) = std::fs::read_to_string(&p) {
                    if !s.trim().is_empty() {
                        return Some(s);
                    }
                }
            }
        }
    }
    None
}

fn parse_sidecar_text_to_doc(text: &str) -> PdfDocument {
    // Prefer form-feed; else split on double newlines into pseudo-pages of ~3k chars
    if text.contains('\x0c') {
        return finalize_document(pages_from_raw_text(text), Some(false));
    }
    let chunks: Vec<&str> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if chunks.is_empty() {
        return finalize_document(pages_from_raw_text(text), Some(false));
    }
    // Merge small chunks into ~page-sized blocks
    let mut pages = Vec::new();
    let mut buf = String::new();
    let mut n = 1usize;
    for c in chunks {
        if buf.len() + c.len() > 3500 && !buf.is_empty() {
            pages.push(PdfPage {
                page_number: n,
                text: buf.clone(),
            });
            n += 1;
            buf.clear();
        }
        if !buf.is_empty() {
            buf.push_str("\n\n");
        }
        buf.push_str(c);
    }
    if !buf.is_empty() {
        pages.push(PdfPage {
            page_number: n,
            text: buf,
        });
    }
    finalize_document(pages, Some(false))
}

/// Main entry: quality-aware extraction with automatic OCR / sidecar fallback.
pub fn extract_text_from_pdf(file_path: &str) -> Result<PdfDocument, String> {
    extract_text_from_pdf_with_options(file_path, &PdfExtractOptions::default())
}

pub fn extract_text_from_pdf_with_options(
    file_path: &str,
    opts: &PdfExtractOptions,
) -> Result<PdfDocument, String> {
    let engine = opts.engine.as_str();
    match engine {
        "ocr" => {
            return extract_pages_via_ocr(
                file_path,
                opts.ocr_lang.as_deref(),
                opts.max_ocr_pages,
            );
        }
        "mineru" | "marker" | "ocrmypdf" => {
            match run_pdf_sidecar(file_path, engine, &opts.sidecar) {
                Ok(text) => return Ok(parse_sidecar_text_to_doc(&text)),
                Err(e) => {
                    tracing::warn!(
                        "[PDF] sidecar {} failed: {} — falling back to OCR",
                        engine,
                        e
                    );
                    return extract_pages_via_ocr(
                        file_path,
                        opts.ocr_lang.as_deref(),
                        opts.max_ocr_pages,
                    );
                }
            }
        }
        _ => {} // pdf-extract (default)
    }

    // Default: pdf-extract + garbble/quality gate → OCR
    match extract_text_via_pdf_extract(file_path) {
        Ok(doc) => {
            let joined: String = doc
                .pages
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");

            if doc.is_scanned || joined.trim().is_empty() {
                tracing::info!("[PDF] empty/scanned digital extract — trying OCR");
                match extract_pages_via_ocr(
                    file_path,
                    opts.ocr_lang.as_deref(),
                    opts.max_ocr_pages,
                ) {
                    Ok(ocr_doc) if !ocr_doc.pages.is_empty() => return Ok(ocr_doc),
                    Ok(_) => return Ok(doc),
                    Err(e) => {
                        tracing::warn!("[PDF] OCR fallback failed: {}", e);
                        return Ok(doc);
                    }
                }
            }

            if is_text_garbled(&joined) {
                tracing::warn!("[PDF] text garbled, falling back to OCR");
                match extract_pages_via_ocr(
                    file_path,
                    opts.ocr_lang.as_deref(),
                    opts.max_ocr_pages,
                ) {
                    Ok(ocr_doc) if ocr_doc.pages.iter().any(|p| !p.text.trim().is_empty()) => {
                        return Ok(ocr_doc);
                    }
                    Ok(_) => {
                        tracing::warn!("[PDF] OCR fallback empty — keeping pdf-extract result");
                        return Ok(doc);
                    }
                    Err(e) => {
                        tracing::warn!("[PDF] OCR fallback failed: {} — keeping pdf-extract", e);
                        return Ok(doc);
                    }
                }
            }

            Ok(doc)
        }
        Err(e) => {
            tracing::warn!(
                "[PDF] pdf-extract failed: {} — falling back to OCR",
                e
            );
            extract_pages_via_ocr(file_path, opts.ocr_lang.as_deref(), opts.max_ocr_pages)
        }
    }
}

/// Get the number of pages in a PDF using Windows.Data.Pdf.
#[cfg(target_os = "windows")]
pub fn get_pdf_page_count(file_path: &str) -> Result<u32, String> {
    use windows::core::HSTRING;
    use windows::Data::Pdf::PdfDocument;
    use windows::Storage::StorageFile;

    let path_str = file_path.to_string();

    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(&path_str))
        .map_err(|e| format!("StorageFile: {}", e))?
        .get()
        .map_err(|e| format!("StorageFile await: {}", e))?;

    let pdf_doc = PdfDocument::LoadFromFileAsync(&file)
        .map_err(|e| format!("LoadPdf: {}", e))?
        .get()
        .map_err(|e| format!("LoadPdf await: {}", e))?;

    let page_count = pdf_doc
        .PageCount()
        .map_err(|e| format!("PageCount: {}", e))?;

    Ok(page_count)
}

#[cfg(not(target_os = "windows"))]
pub fn get_pdf_page_count(_file_path: &str) -> Result<u32, String> {
    Err("PDF page count requires Windows".into())
}

/// Render a single PDF page to PNG bytes using Windows.Data.Pdf.
#[cfg(target_os = "windows")]
pub fn render_pdf_page_to_png(file_path: &str, page_index: u32) -> Result<Vec<u8>, String> {
    use windows::core::HSTRING;
    use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
    use windows::Storage::StorageFile;
    use windows::Storage::Streams::InMemoryRandomAccessStream;

    let path_str = file_path.to_string();

    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(&path_str))
        .map_err(|e| format!("StorageFile: {}", e))?
        .get()
        .map_err(|e| format!("StorageFile await: {}", e))?;

    let pdf_doc = PdfDocument::LoadFromFileAsync(&file)
        .map_err(|e| format!("LoadPdf: {}", e))?
        .get()
        .map_err(|e| format!("LoadPdf await: {}", e))?;

    let page = pdf_doc
        .GetPage(page_index)
        .map_err(|e| format!("GetPage({}): {}", page_index, e))?;

    let stream = InMemoryRandomAccessStream::new().map_err(|e| format!("InMemoryStream: {}", e))?;

    let render_options =
        PdfPageRenderOptions::new().map_err(|e| format!("RenderOptions: {}", e))?;

    let width = 2048u32;
    let height = 2896u32;
    render_options
        .SetDestinationWidth(width)
        .map_err(|e| format!("SetDestinationWidth: {}", e))?;
    render_options
        .SetDestinationHeight(height)
        .map_err(|e| format!("SetDestinationHeight: {}", e))?;

    page.RenderToStreamAsync(&stream)
        .map_err(|e| format!("RenderPage: {}", e))?
        .get()
        .map_err(|e| format!("RenderPage await: {}", e))?;

    let size = stream.Size().map_err(|e| format!("StreamSize: {}", e))? as usize;
    let input = windows::Storage::Streams::InputStreamOptions::None;
    // Seek to start
    stream
        .Seek(0)
        .map_err(|e| format!("StreamSeek: {}", e))?;

    let reader = windows::Storage::Streams::DataReader::CreateDataReader(&stream)
        .map_err(|e| format!("DataReader: {}", e))?;
    let _ = input;

    reader
        .LoadAsync(size as u32)
        .map_err(|e| format!("LoadAsync: {}", e))?
        .get()
        .map_err(|e| format!("LoadAsync await: {}", e))?;

    let mut bytes = vec![0u8; size];
    reader
        .ReadBytes(&mut bytes)
        .map_err(|e| format!("ReadBytes: {}", e))?;

    let img = image::load_from_memory(&bytes).map_err(|e| format!("LoadBmp: {}", e))?;

    let mut png_buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png_buf, image::ImageFormat::Png)
        .map_err(|e| format!("EncodePng: {}", e))?;

    Ok(png_buf.into_inner())
}

#[cfg(not(target_os = "windows"))]
pub fn render_pdf_page_to_png(_file_path: &str, _page_index: u32) -> Result<Vec<u8>, String> {
    Err("PDF page render requires Windows".into())
}

/// Check if a file is a PDF.
pub fn is_pdf_file(file_path: &str) -> bool {
    let path = std::path::Path::new(file_path);
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbled_detects_letter_spacing() {
        let t = "C o v e r s P y t h o n Clear Concise Effective Programming Luciano";
        assert!(is_text_garbled(t));
    }

    #[test]
    fn clean_english_not_garbled() {
        let t = "Before Starting A few years ago, I shared the Sword for Offer problem solutions on LeetCode, receiving encouragement from many readers.";
        assert!(!is_text_garbled(t));
    }

    #[test]
    fn replacement_char_is_garbled() {
        assert!(is_text_garbled("hello \u{FFFD} world more text here ok"));
    }
}
