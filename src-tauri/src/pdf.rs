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
}

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

    let total_pages = pages.len();

    Ok(PdfDocument {
        pages,
        total_pages,
    })
}
