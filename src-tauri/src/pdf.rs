use serde::{Deserialize, Serialize};

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

/// Threshold: if average chars per page is below this, consider the PDF as scanned.
const SCANNED_PDF_CHAR_THRESHOLD: usize = 50;

pub fn extract_text_from_pdf(file_path: &str) -> Result<PdfDocument, String> {
    let data = std::fs::read(file_path).map_err(|e| format!("Failed to read PDF file: {}", e))?;

    let text = pdf_extract::extract_text_from_mem(&data)
        .map_err(|e| format!("Failed to extract text from PDF: {}", e))?;

    // Split text into pages (approximate by double newlines or form feeds)
    let page_texts: Vec<&str> = text.split('\x0C').collect();
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

    // If no pages were found (no form feeds), treat entire text as one page
    if pages.is_empty() && !text.trim().is_empty() {
        pages.push(PdfPage {
            page_number: 1,
            text: text.trim().to_string(),
        });
    }

    // Detect if PDF is scanned (very little or no text)
    let total_chars: usize = pages.iter().map(|p| p.text.len()).sum();
    let page_count = pages.len().max(1);
    let avg_chars_per_page = total_chars / page_count;
    let is_scanned = avg_chars_per_page < SCANNED_PDF_CHAR_THRESHOLD && total_chars < 200;

    tracing::info!(
        "[PDF] Extracted {} chars across {} pages, avg {} chars/page, is_scanned={}",
        total_chars,
        pages.len(),
        avg_chars_per_page,
        is_scanned
    );

    if pages.is_empty() {
        // Empty PDF - likely scanned
        return Ok(PdfDocument {
            pages: Vec::new(),
            total_pages: 0,
            is_scanned: true,
        });
    }

    let total_pages = pages.len();
    Ok(PdfDocument {
        pages,
        total_pages,
        is_scanned,
    })
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

    let page_count = pdf_doc.PageCount()
        .map_err(|e| format!("PageCount: {}", e))?;

    Ok(page_count)
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

    let page = pdf_doc.GetPage(page_index)
        .map_err(|e| format!("GetPage({}): {}", page_index, e))?;

    let stream = InMemoryRandomAccessStream::new()
        .map_err(|e| format!("InMemoryStream: {}", e))?;

    let render_options = PdfPageRenderOptions::new()
        .map_err(|e| format!("RenderOptions: {}", e))?;

    // Set bitmap dimensions for good OCR quality (2x scale, capped at 4096px)
    let width = 2048u32;
    let height = 2896u32; // A4 aspect ratio at 2x
    render_options.SetDestinationWidth(width)
        .map_err(|e| format!("SetDestinationWidth: {}", e))?;
    render_options.SetDestinationHeight(height)
        .map_err(|e| format!("SetDestinationHeight: {}", e))?;

    page.RenderToStreamAsync(&stream)
        .map_err(|e| format!("RenderPage: {}", e))?
        .get()
        .map_err(|e| format!("RenderPage await: {}", e))?;

    // Read stream into bytes
    let size = stream.Size().map_err(|e| format!("StreamSize: {}", e))? as usize;
    let reader = windows::Storage::Streams::DataReader::CreateDataReader(&stream)
        .map_err(|e| format!("DataReader: {}", e))?;

    reader.LoadAsync(size as u32)
        .map_err(|e| format!("LoadAsync: {}", e))?
        .get()
        .map_err(|e| format!("LoadAsync await: {}", e))?;

    let mut bytes = vec![0u8; size];
    reader.ReadBytes(&mut bytes)
        .map_err(|e| format!("ReadBytes: {}", e))?;

    // The rendered stream is a BMP. Convert to PNG.
    let img = image::load_from_memory(&bytes)
        .map_err(|e| format!("LoadBmp: {}", e))?;

    let mut png_buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png_buf, image::ImageFormat::Png)
        .map_err(|e| format!("EncodePng: {}", e))?;

    Ok(png_buf.into_inner())
}

/// Check if a file is a PDF.
pub fn is_pdf_file(file_path: &str) -> bool {
    let path = std::path::Path::new(file_path);
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}
