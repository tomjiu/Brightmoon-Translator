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

/// S5-1: layout mode for bilingual PDF export.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum BilingualPdfLayout {
    /// Original and translation side-by-side in two columns.
    #[default]
    SideBySide,
    /// Original paragraph followed by translation paragraph.
    Interleaved,
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
                    .is_some_and(|c| c.is_ascii_alphabetic())
        })
        .count();
    if scattered > 5 {
        return true;
    }

    // 2. High ratio of 2–3 char alphabetic fragments.
    // Exclude common English function words (the/for/on/of...) so ordinary
    // text isn't misclassified; only unusual short fragments count as garbling.
    const COMMON_SHORT_WORDS: &[&str] = &[
        "a", "an", "as", "at", "be", "by", "do", "go", "he", "if", "in", "is", "it", "me",
        "my", "no", "of", "on", "or", "so", "to", "up", "us", "we", "am", "and", "are",
        "but", "can", "for", "get", "had", "has", "her", "him", "his", "how", "its", "let",
        "may", "new", "not", "now", "old", "out", "own", "per", "put", "say", "she", "the",
        "too", "two", "use", "was", "who", "you", "all", "any", "did", "few", "off", "our",
        "see", "try", "one", "day", "yet",
    ];
    let fragments = words
        .iter()
        .filter(|w| {
            (2..=3).contains(&w.len())
                && w.chars().all(|c| c.is_ascii_alphabetic())
                && !COMMON_SHORT_WORDS.contains(&w.to_ascii_lowercase().as_str())
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
    let is_scanned = force_scanned.unwrap_or({
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

/// Maximum PDF size we are willing to slurp into memory for pdf-extract.
/// `pdf_extract::extract_text_from_mem` requires the whole file in memory, so
/// streaming wouldn't help; instead we refuse pathologically large files to
/// avoid OOM. 256 MB is well above any normal text PDF and matches the
/// upper bound of scanned-book PDFs we have tested.
const PDF_MAX_INMEMORY_BYTES: u64 = 256 * 1024 * 1024;

/// Original pdf-extract path only (no quality gate).
///
/// S2-6: this function is synchronous and reads the entire file into memory.
/// It is safe to call from the tokio runtime because every caller wraps it
/// in `tokio::task::spawn_blocking` (see `pdf_cmd::open_pdf` /
/// `translate_pdf`). We guard against OOM on huge scanned PDFs with a size
/// check before the read — `pdf_extract::extract_text_from_mem` needs the
/// whole file in memory anyway, so streaming would not help here.
pub fn extract_text_via_pdf_extract(file_path: &str) -> Result<PdfDocument, String> {
    let metadata = std::fs::metadata(file_path)
        .map_err(|e| format!("Failed to stat PDF file: {e}"))?;
    if metadata.len() > PDF_MAX_INMEMORY_BYTES {
        return Err(format!(
            "PDF file is too large ({} bytes > {} limit). Please use a scanned-PDF OCR sidecar (ocrmypdf) for large files.",
            metadata.len(),
            PDF_MAX_INMEMORY_BYTES
        ));
    }
    let data = std::fs::read(file_path).map_err(|e| format!("Failed to read PDF file: {e}"))?;

    // Catch panics from pdf-extract on unsupported PDF features
    let text = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pdf_extract::extract_text_from_mem(&data)
    })) {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => {
            return Err(format!("Failed to extract text from PDF: {e}"));
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
        .map_or(page_count, |m| m.min(page_count));

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

/// S5-12: resolve a PDF sidecar CLI to a canonical absolute path.
///
/// Replaces the old `command_exists` which spawned `cmd --help` to probe
/// existence — that executed arbitrary code from `sidecar.*_cmd` and was
/// a PATH-hijack / cwd-injection vector. We now use the `which` crate to
/// resolve the command through PATH (or accept an absolute path directly),
/// and return the canonicalized path so the later `Command::new` uses the
/// resolved binary instead of re-searching PATH at spawn time.
///
/// Returns `Err(message)` when the command cannot be found, with a message
/// suitable for surfacing to the user.
fn resolve_sidecar_cmd(cmd: &str) -> Result<PathBuf, String> {
    if cmd.is_empty() {
        return Err("empty command".into());
    }
    which::which(cmd)
        .map_err(|e| format!("'{cmd}' not found on PATH: {e}"))
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

    // S5-12: each branch resolves the CLI to an absolute path via `which`
    // before spawning. The resolved path is what we pass to `Command::new`
    // so a later PATH mutation between resolve and spawn can't redirect us
    // to a different binary.
    let (program, args, result_glob): (PathBuf, Vec<String>, Option<PathBuf>) = match engine {
        "mineru" => {
            let cmd = if sidecar.mineru_cmd.is_empty() {
                "magic-pdf".to_string()
            } else {
                sidecar.mineru_cmd.clone()
            };
            let resolved = resolve_sidecar_cmd(&cmd)
                .map_err(|e| format!("mineru CLI not found: {e}"))?;
            (
                resolved,
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
            let resolved = resolve_sidecar_cmd(&cmd)
                .map_err(|e| format!("marker CLI not found: {e}"))?;
            let out_md = tmp.join("out.md");
            (
                resolved,
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
            let resolved = resolve_sidecar_cmd(&cmd)
                .map_err(|e| format!("ocrmypdf CLI not found: {e}"))?;
            let out_pdf = tmp.join("ocr_output.pdf");
            (
                resolved,
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

    tracing::info!(
        "[PDF] running sidecar {} (resolved: {}) on {}",
        engine,
        program.display(),
        file_path
    );

    let mut child = Command::new(&program)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn {engine} ({}): {e}", program.display()))?;

    // Soft timeout ~120s
    let deadline = std::time::Instant::now() + Duration::from_mins(2);
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
                .is_some_and(|x| x.eq_ignore_ascii_case("md"))
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
        .map_err(|e| format!("StorageFile: {e}"))?
        .get()
        .map_err(|e| format!("StorageFile await: {e}"))?;

    let pdf_doc = PdfDocument::LoadFromFileAsync(&file)
        .map_err(|e| format!("LoadPdf: {e}"))?
        .get()
        .map_err(|e| format!("LoadPdf await: {e}"))?;

    let page_count = pdf_doc
        .PageCount()
        .map_err(|e| format!("PageCount: {e}"))?;

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
        .map_err(|e| format!("StorageFile: {e}"))?
        .get()
        .map_err(|e| format!("StorageFile await: {e}"))?;

    let pdf_doc = PdfDocument::LoadFromFileAsync(&file)
        .map_err(|e| format!("LoadPdf: {e}"))?
        .get()
        .map_err(|e| format!("LoadPdf await: {e}"))?;

    let page = pdf_doc
        .GetPage(page_index)
        .map_err(|e| format!("GetPage({page_index}): {e}"))?;

    let stream = InMemoryRandomAccessStream::new().map_err(|e| format!("InMemoryStream: {e}"))?;

    let render_options =
        PdfPageRenderOptions::new().map_err(|e| format!("RenderOptions: {e}"))?;

    let width = 2048u32;
    let height = 2896u32;
    render_options
        .SetDestinationWidth(width)
        .map_err(|e| format!("SetDestinationWidth: {e}"))?;
    render_options
        .SetDestinationHeight(height)
        .map_err(|e| format!("SetDestinationHeight: {e}"))?;

    page.RenderToStreamAsync(&stream)
        .map_err(|e| format!("RenderPage: {e}"))?
        .get()
        .map_err(|e| format!("RenderPage await: {e}"))?;

    let size = stream.Size().map_err(|e| format!("StreamSize: {e}"))? as usize;
    let input = windows::Storage::Streams::InputStreamOptions::None;
    // Seek to start
    stream
        .Seek(0)
        .map_err(|e| format!("StreamSeek: {e}"))?;

    let reader = windows::Storage::Streams::DataReader::CreateDataReader(&stream)
        .map_err(|e| format!("DataReader: {e}"))?;
    let _ = input;

    reader
        .LoadAsync(size as u32)
        .map_err(|e| format!("LoadAsync: {e}"))?
        .get()
        .map_err(|e| format!("LoadAsync await: {e}"))?;

    let mut bytes = vec![0u8; size];
    reader
        .ReadBytes(&mut bytes)
        .map_err(|e| format!("ReadBytes: {e}"))?;

    let img = image::load_from_memory(&bytes).map_err(|e| format!("LoadBmp: {e}"))?;

    let mut png_buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png_buf, image::ImageFormat::Png)
        .map_err(|e| format!("EncodePng: {e}"))?;

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
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

// ── S5-1: bilingual PDF writer ─────────────────────────────────────────────

/// Resolve a system CJK-capable font file path. We prefer Windows' Microsoft
/// `YaHei` (msyh.ttc) and Microsoft `YaHei` UI, then fall back to a few common
/// candidates. Returns `None` only if no candidate exists — the caller then
/// falls back to the built-in Helvetica (Latin only) so export still works
/// for ASCII-heavy documents.
fn resolve_cjk_font() -> Option<PathBuf> {
    let windir = std::env::var_os("WINDIR").unwrap_or_else(|| std::ffi::OsString::from("C:\\Windows"));
    let fonts_dir = Path::new(&windir).join("Fonts");
    // Order matters: prefer YaHei UI (lighter, good CJK + Latin), then the
    // classic YaHei, then SimSun as last resort.
    let candidates = [
        "msyh.ttc",
        "msyh.ttf",
        "msyhbd.ttc",
        "simsun.ttc",
        "simhei.ttf",
        "Deng.ttf",
    ];
    for name in &candidates {
        let p = fonts_dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// S5-1: Write a bilingual PDF file. Each page of the source document is
/// rendered as one or two columns (original + translation) in the output
/// PDF. Uses system CJK fonts when available; otherwise falls back to
/// Helvetica (Latin only).
///
/// Layout:
/// - `SideBySide`: two columns per page (original left, translation right).
/// - `Interleaved`: original paragraph followed by translation paragraph,
///   single column.
pub fn write_bilingual_pdf(
    output_path: &str,
    pages: &[TranslatedPage],
    layout: BilingualPdfLayout,
) -> Result<(), String> {
    use printpdf::{BuiltinFont, PdfDocument as PdfDoc, Mm};

    let (doc, page1, layer1) = PdfDoc::new(
        "Moon Translator - Bilingual PDF",
        Mm(210.0), // A4 width
        Mm(297.0), // A4 height
        "Layer 1",
    );

    // Load CJK font if available; otherwise fall back to Helvetica.
    let font = if let Some(font_path) = resolve_cjk_font() {
        let font_data = std::fs::read(&font_path)
            .map_err(|e| format!("Failed to read font {}: {e}", font_path.display()))?;
        // printpdf expects Vec<u8> for both TTF and TTC.
        // msyh.ttc is a TrueType Collection; printpdf's TTF loader reads
        // only the first face, which is YaHei Regular — exactly what we want.
        //
        // P7: attempt to subset the font to only the glyphs used in the
        // translated text. This can shrink msyh.ttf (17 MB) to ~50 KB,
        // keeping the bilingual PDF small. Falls back to the full font
        // if subsetting fails or printpdf rejects the subsetted font.
        let all_text: Vec<&str> = pages
            .iter()
            .flat_map(|p| [p.original_text.as_str(), p.translated_text.as_str()])
            .collect();
        let subset_text = crate::font_subset::collect_text_chars(&all_text);
        let subset_bytes: Option<Vec<u8>> = match crate::font_subset::subset_font_for_text(
            &font_data,
            &subset_text,
        ) {
            Ok(result) => {
                tracing::info!(
                    "[P7] using subsetted font ({} KB → {} KB)",
                    font_data.len() / 1024,
                    result.font_bytes.len() / 1024
                );
                Some(result.font_bytes)
            }
            Err(e) => {
                tracing::warn!(
                    "[P7] font subsetting failed, using full font ({} KB): {}",
                    font_data.len() / 1024,
                    e
                );
                None
            }
        };
        // Try the subsetted font first; fall back to full font if printpdf
        // rejects it (subsetter removes cmap, which some loaders require).
        if let Some(sub) = subset_bytes {
            match doc.add_external_font(sub.as_slice()) {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!(
                        "[P7] printpdf rejected subsetted font, using full font: {}",
                        e
                    );
                    doc.add_external_font(font_data.as_slice())
                        .map_err(|e| format!("Failed to load CJK font: {e}"))?
                }
            }
        } else {
            doc.add_external_font(font_data.as_slice())
                .map_err(|e| format!("Failed to load CJK font: {e}"))?
        }
    } else {
        tracing::warn!("[PDF export] No CJK font found; falling back to Helvetica (Latin only)");
        doc.add_builtin_font(BuiltinFont::Helvetica)
            .map_err(|e| format!("Failed to load builtin font: {e}"))?
    };

    let page_margin = Mm(15.0);
    let page_width = Mm(210.0);
    let page_height = Mm(297.0);
    let content_width = page_width - page_margin * 2.0;
    let line_height = Mm(5.5);
    let font_size = 10.0;

    let col_gap = Mm(6.0);
    let col_width = (content_width - col_gap) / 2.0;

    let mut current_layer = doc.get_page(page1).get_layer(layer1);
    // Use f32 internally for y to simplify arithmetic; convert to Mm on draw.
    // Initial value is overwritten by start_new_page on the first iteration.
    #[allow(unused_assignments)]
    let mut y: f32 = 0.0;
    let mut first_page = true;

    let start_new_page = |doc: &printpdf::PdfDocumentReference,
                          first_page: &mut bool,
                          current_layer: &mut printpdf::PdfLayerReference|
     -> f32 {
        if *first_page {
            *first_page = false;
            (page_height - page_margin).0
        } else {
            let (p, l) = doc.add_page(page_width, page_height, "Layer 1");
            *current_layer = doc.get_page(p).get_layer(l);
            (page_height - page_margin).0
        }
    };

    #[allow(unused_assignments)]
    for page in pages {
        // Page header: "Page N"
        y = start_new_page(&doc, &mut first_page, &mut current_layer);
        current_layer.use_text(
            format!("Page {}", page.page_number),
            font_size + 2.0,
            page_margin,
            Mm(y),
            &font,
        );
        y -= line_height.0 * 1.6;

        let orig_lines = wrap_text(&page.original_text, font_size);
        let trans_lines = wrap_text(&page.translated_text, font_size);

        match layout {
            BilingualPdfLayout::SideBySide => {
                let max_lines = orig_lines.len().max(trans_lines.len());
                for i in 0..max_lines {
                    if y < page_margin.0 + line_height.0 {
                        y = start_new_page(&doc, &mut first_page, &mut current_layer);
                    }
                    if let Some(line) = orig_lines.get(i) {
                        current_layer.use_text(
                            line,
                            font_size,
                            page_margin,
                            Mm(y),
                            &font,
                        );
                    }
                    if let Some(line) = trans_lines.get(i) {
                        current_layer.use_text(
                            line,
                            font_size,
                            page_margin + col_width + col_gap,
                            Mm(y),
                            &font,
                        );
                    }
                    y -= line_height.0;
                }
            },
            BilingualPdfLayout::Interleaved => {
                let orig_full = wrap_text(&page.original_text, font_size);
                let trans_full = wrap_text(&page.translated_text, font_size);

                // Original (label + lines)
                if y < page_margin.0 + line_height.0 * 2.0 {
                    y = start_new_page(&doc, &mut first_page, &mut current_layer);
                }
                current_layer.use_text("[原文]", font_size - 1.0, page_margin, Mm(y), &font);
                y -= line_height.0;
                for line in &orig_full {
                    if y < page_margin.0 + line_height.0 {
                        y = start_new_page(&doc, &mut first_page, &mut current_layer);
                    }
                    current_layer.use_text(line, font_size, page_margin, Mm(y), &font);
                    y -= line_height.0;
                }
                y -= line_height.0 * 0.4;

                // Translation (label + lines)
                if y < page_margin.0 + line_height.0 * 2.0 {
                    y = start_new_page(&doc, &mut first_page, &mut current_layer);
                }
                current_layer.use_text("[译文]", font_size - 1.0, page_margin, Mm(y), &font);
                y -= line_height.0;
                for line in &trans_full {
                    if y < page_margin.0 + line_height.0 {
                        y = start_new_page(&doc, &mut first_page, &mut current_layer);
                    }
                    current_layer.use_text(line, font_size, page_margin, Mm(y), &font);
                    y -= line_height.0;
                }
                y -= line_height.0 * 0.8;
            },
        }
    }

    let out = std::fs::File::create(output_path)
        .map_err(|e| format!("Failed to create output PDF: {e}"))?;
    doc.save(&mut std::io::BufWriter::new(out))
        .map_err(|e| format!("Failed to save PDF: {e}"))?;
    tracing::info!("[PDF export] Bilingual PDF saved: {}", output_path);
    Ok(())
}

/// Naive text wrapper: splits on existing newlines, then hard-wraps long
/// lines by character count. CJK chars are counted as 1 unit; this is a
/// rough heuristic — printpdf doesn't measure glyph advance, so we can't
/// do real width-based wrapping without a shaping engine. Good enough for
/// readable bilingual output.
fn wrap_text(text: &str, font_size: f32) -> Vec<String> {
    if text.trim().is_empty() {
        return vec![String::new()];
    }
    // Rough chars-per-line estimate: A4 col width ~90mm, font_size 10pt.
    // Average char width ≈ font_size * 0.5pt ≈ 1.76mm for Latin, CJK ≈ font_size pt ≈ 3.53mm.
    // Use a conservative mixed estimate: ~38 chars per 90mm column at 10pt.
    let chars_per_line = ((90.0 / (font_size * 0.5 * 0.3528)) as usize).max(20);
    let mut out = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            out.push(String::new());
            continue;
        }
        let chars: Vec<char> = paragraph.chars().collect();
        let mut start = 0;
        while start < chars.len() {
            let end = (start + chars_per_line).min(chars.len());
            // Try to break on a space if possible (Latin text)
            let mut break_at = end;
            if end < chars.len() {
                // Look back up to 15 chars for a space
                for i in (end.saturating_sub(15)..end).rev() {
                    if chars[i] == ' ' {
                        break_at = i + 1;
                        break;
                    }
                }
            }
            let line: String = chars[start..break_at].iter().collect();
            out.push(line);
            start = break_at;
            // Skip the space we broke on
            if start < chars.len() && chars[start] == ' ' {
                start += 1;
            }
        }
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// P9: Write a translated PDF file in Mono or Dual mode.
///
/// - `Mono`: each page contains only the translated text (replaces original).
/// - `Dual`: each original page is followed by an interleaved translation page,
///   producing a bilingual PDF where original and translation alternate.
///
/// Uses the same CJK font resolution and `wrap_text` helper as `write_bilingual_pdf`.
pub fn write_translated_pdf(
    output_path: &str,
    pages: &[TranslatedPage],
    mode: crate::pdf_il::PdfOutputMode,
) -> Result<(), String> {
    use crate::pdf_il::PdfOutputMode;
    use printpdf::{BuiltinFont, PdfDocument as PdfDoc, Mm};

    let title = match mode {
        PdfOutputMode::Mono => "Moon Translator - Translated PDF",
        PdfOutputMode::Dual => "Moon Translator - Dual PDF",
    };
    let (doc, page1, layer1) = PdfDoc::new(
        title,
        Mm(210.0),
        Mm(297.0),
        "Layer 1",
    );

    let font = if let Some(font_path) = resolve_cjk_font() {
        let font_data = std::fs::read(&font_path)
            .map_err(|e| format!("Failed to read font {}: {e}", font_path.display()))?;
        doc.add_external_font(font_data.as_slice())
            .map_err(|e| format!("Failed to load CJK font: {e}"))?
    } else {
        tracing::warn!("[PDF export] No CJK font found; falling back to Helvetica (Latin only)");
        doc.add_builtin_font(BuiltinFont::Helvetica)
            .map_err(|e| format!("Failed to load builtin font: {e}"))?
    };

    let page_margin = Mm(15.0);
    let page_width = Mm(210.0);
    let page_height = Mm(297.0);
    let line_height = Mm(5.5);
    let font_size = 10.0;

    let mut current_layer = doc.get_page(page1).get_layer(layer1);
    #[allow(unused_assignments)]
    let mut y: f32 = 0.0;
    let mut first_page = true;

    let start_new_page = |doc: &printpdf::PdfDocumentReference,
                          first_page: &mut bool,
                          current_layer: &mut printpdf::PdfLayerReference|
     -> f32 {
        if *first_page {
            *first_page = false;
            (page_height - page_margin).0
        } else {
            let (p, l) = doc.add_page(page_width, page_height, "Layer 1");
            *current_layer = doc.get_page(p).get_layer(l);
            (page_height - page_margin).0
        }
    };

    let render_text_block = |doc: &printpdf::PdfDocumentReference,
                             first_page: &mut bool,
                             current_layer: &mut printpdf::PdfLayerReference,
                             y: &mut f32,
                             header: &str,
                             text: &str| {
        // Page header
        *y = start_new_page(doc, first_page, current_layer);
        if !header.is_empty() {
            current_layer.use_text(header, font_size + 2.0, page_margin, Mm(*y), &font);
            *y -= line_height.0 * 1.6;
        }
        let lines = wrap_text(text, font_size);
        for line in &lines {
            if *y < page_margin.0 + line_height.0 {
                *y = start_new_page(doc, first_page, current_layer);
            }
            current_layer.use_text(line, font_size, page_margin, Mm(*y), &font);
            *y -= line_height.0;
        }
    };

    match mode {
        PdfOutputMode::Mono => {
            // Each page: translated text only (original replaced)
            for page in pages {
                render_text_block(
                    &doc,
                    &mut first_page,
                    &mut current_layer,
                    &mut y,
                    &format!("Page {}", page.page_number),
                    &page.translated_text,
                );
            }
        },
        PdfOutputMode::Dual => {
            // Each original page followed by its translation page (interleaved)
            for page in pages {
                // Original page
                render_text_block(
                    &doc,
                    &mut first_page,
                    &mut current_layer,
                    &mut y,
                    &format!("Page {} (Original)", page.page_number),
                    &page.original_text,
                );
                // Force a new page for the translation
                first_page = false;
                render_text_block(
                    &doc,
                    &mut first_page,
                    &mut current_layer,
                    &mut y,
                    &format!("Page {} (Translation)", page.page_number),
                    &page.translated_text,
                );
            }
        },
    }

    let out = std::fs::File::create(output_path)
        .map_err(|e| format!("Failed to create output PDF: {e}"))?;
    doc.save(&mut std::io::BufWriter::new(out))
        .map_err(|e| format!("Failed to save PDF: {e}"))?;
    tracing::info!("[PDF export] Translated PDF ({:?}) saved: {}", mode, output_path);
    Ok(())
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

    // ── S5-12: sidecar CLI resolution ────────────────────────────────────

    #[test]
    fn resolve_empty_cmd_fails() {
        assert!(resolve_sidecar_cmd("").is_err());
    }

    #[test]
    fn resolve_nonexistent_cmd_fails() {
        // Pick a name unlikely to exist on any real system.
        let err = resolve_sidecar_cmd("moontranslator_definitely_not_a_real_binary_xyz");
        assert!(err.is_err(), "expected Err for nonexistent binary");
        let msg = err.unwrap_err();
        assert!(
            msg.contains("not found"),
            "error message should mention 'not found': got {msg}"
        );
    }

    /// A bare name that does exist on PATH should resolve to an absolute path.
    /// We use `cargo` (always present when running `cargo test`) and only
    /// assert the path is absolute — we don't assert a specific location.
    #[test]
    fn resolve_known_cmd_returns_absolute_path() {
        let resolved = resolve_sidecar_cmd("cargo");
        // On some minimal CI images cargo may not be on PATH at test time,
        // so treat absence as a skip rather than a failure.
        match resolved {
            Ok(path) => {
                assert!(
                    path.is_absolute(),
                    "resolved path should be absolute, got {}",
                    path.display()
                );
            },
            Err(e) => {
                eprintln!("[S5-12] cargo not on PATH — skipping: {e}");
            },
        }
    }

    /// An absolute path to a real file should resolve successfully; an
    /// absolute path to a missing file should fail. This covers the
    /// `sidecar.*_cmd` case where the user passes a full path.
    #[test]
    fn resolve_absolute_path() {
        // Use this very source file as a known-existing absolute path.
        let this_file = env!("CARGO_MANIFEST_DIR").to_string() + "/src/pdf.rs";
        let path = PathBuf::from(&this_file);
        assert!(path.is_file(), "test precondition: {this_file} must exist");

        // `which` accepts absolute paths directly when the file exists.
        // (It doesn't care about the executable bit on Windows.)
        let resolved = resolve_sidecar_cmd(&this_file);
        assert!(resolved.is_ok(), "absolute existing path should resolve");

        let missing = this_file + ".does_not_exist";
        assert!(
            resolve_sidecar_cmd(&missing).is_err(),
            "absolute missing path should fail"
        );
    }

    // ── S5-1: bilingual PDF writer ──────────────────────────────────────

    #[test]
    fn test_write_bilingual_pdf_side_by_side() {
        let tmp = std::env::temp_dir().join("moontranslator_s5-1_test_sbs.pdf");
        let pages = vec![
            TranslatedPage {
                page_number: 1,
                original_text: "Hello world.\nThis is a test of the bilingual PDF export.".to_string(),
                translated_text: "你好，世界。\n这是双语 PDF 导出的测试。".to_string(),
            },
            TranslatedPage {
                page_number: 2,
                original_text: "The quick brown fox jumps over the lazy dog.".to_string(),
                translated_text: "敏捷的棕色狐狸跳过那只懒狗。".to_string(),
            },
        ];
        let result = write_bilingual_pdf(tmp.to_str().unwrap(), &pages, BilingualPdfLayout::SideBySide);
        assert!(result.is_ok(), "write_bilingual_pdf SideBySide failed: {:?}", result.err());
        assert!(tmp.is_file(), "output PDF file not created");
        // Verify it's a valid PDF (magic header)
        let header = std::fs::read(&tmp).unwrap();
        assert!(header.starts_with(b"%PDF-"), "output is not a valid PDF");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_write_bilingual_pdf_interleaved() {
        let tmp = std::env::temp_dir().join("moontranslator_s5-1_test_interleaved.pdf");
        let pages = vec![TranslatedPage {
            page_number: 1,
            original_text: "Interleaved layout test.".to_string(),
            translated_text: "交错布局测试。".to_string(),
        }];
        let result = write_bilingual_pdf(tmp.to_str().unwrap(), &pages, BilingualPdfLayout::Interleaved);
        assert!(result.is_ok(), "write_bilingual_pdf Interleaved failed: {:?}", result.err());
        assert!(tmp.is_file(), "output PDF file not created");
        let header = std::fs::read(&tmp).unwrap();
        assert!(header.starts_with(b"%PDF-"), "output is not a valid PDF");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_wrap_text_basic() {
        let lines = wrap_text("Hello world this is a test", 10.0);
        assert!(!lines.is_empty());
        // Long line should be wrapped
        let long = "a".repeat(100);
        let wrapped = wrap_text(&long, 10.0);
        assert!(wrapped.len() > 1, "long line should be wrapped into multiple lines");
    }

    #[test]
    fn test_wrap_text_empty() {
        let lines = wrap_text("", 10.0);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].is_empty());
    }

    #[test]
    fn test_wrap_text_preserves_newlines() {
        let lines = wrap_text("line1\nline2\nline3", 10.0);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line1");
        assert_eq!(lines[1], "line2");
        assert_eq!(lines[2], "line3");
    }

    // ==================== P9 Tests ====================

    #[test]
    fn p9_write_translated_pdf_mono() {
        let tmp = std::env::temp_dir().join("moontranslator_p9_mono.pdf");
        let pages = vec![
            TranslatedPage {
                page_number: 1,
                original_text: "Hello world.".to_string(),
                translated_text: "你好世界。".to_string(),
            },
            TranslatedPage {
                page_number: 2,
                original_text: "Second page.".to_string(),
                translated_text: "第二页。".to_string(),
            },
        ];
        let result = write_translated_pdf(
            tmp.to_str().unwrap(),
            &pages,
            crate::pdf_il::PdfOutputMode::Mono,
        );
        assert!(result.is_ok(), "write_translated_pdf Mono failed: {:?}", result.err());
        assert!(tmp.is_file(), "output PDF file not created");
        let header = std::fs::read(&tmp).unwrap();
        assert!(header.starts_with(b"%PDF-"), "output is not a valid PDF");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn p9_write_translated_pdf_dual() {
        let tmp = std::env::temp_dir().join("moontranslator_p9_dual.pdf");
        let pages = vec![TranslatedPage {
            page_number: 1,
            original_text: "Original text.".to_string(),
            translated_text: "译文文本。".to_string(),
        }];
        let result = write_translated_pdf(
            tmp.to_str().unwrap(),
            &pages,
            crate::pdf_il::PdfOutputMode::Dual,
        );
        assert!(result.is_ok(), "write_translated_pdf Dual failed: {:?}", result.err());
        assert!(tmp.is_file(), "output PDF file not created");
        let header = std::fs::read(&tmp).unwrap();
        assert!(header.starts_with(b"%PDF-"), "output is not a valid PDF");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn p9_write_translated_pdf_empty_pages() {
        let tmp = std::env::temp_dir().join("moontranslator_p9_empty.pdf");
        let pages: Vec<TranslatedPage> = vec![];
        let result = write_translated_pdf(
            tmp.to_str().unwrap(),
            &pages,
            crate::pdf_il::PdfOutputMode::Mono,
        );
        // Empty pages should still produce a valid (empty) PDF
        assert!(result.is_ok(), "write_translated_pdf empty failed: {:?}", result.err());
        assert!(tmp.is_file(), "output PDF file not created");
        let _ = std::fs::remove_file(&tmp);
    }
}
