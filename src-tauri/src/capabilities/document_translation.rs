use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::models::error::TranslationError;

/// Supported document formats
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DocumentFormat {
    Pdf,
    Epub,
    Subtitle,
}

/// Options for document translation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTranslateOptions {
    /// Source language (None = auto-detect)
    pub from: Option<String>,
    /// Target language (None = use default)
    pub to: Option<String>,
    /// Concurrency for batch translation (default: 3)
    pub concurrency: usize,
    /// Whether to preserve original formatting
    pub preserve_format: bool,
}

impl Default for DocumentTranslateOptions {
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            concurrency: 3,
            preserve_format: true,
        }
    }
}

/// A translated segment in a document
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedSegment {
    /// Segment index (line number, page number, etc.)
    pub index: usize,
    /// Original text
    pub original: String,
    /// Translated text
    pub translated: String,
}

/// Progress callback type
pub type ProgressCallback = Box<dyn Fn(usize, usize) + Send + Sync>;

/// Document Translation capability.
/// Composes: document parsing -> batch translation -> reassembly.
///
/// This is the interface for translating PDF, EPUB, and subtitle files.
#[async_trait]
pub trait DocumentTranslation: Send + Sync {
    /// Get supported format
    fn format(&self) -> DocumentFormat;

    /// Open and parse a document file
    async fn open(&self, file_path: &str) -> Result<DocumentContent, TranslationError>;

    /// Translate parsed document content
    async fn translate(
        &self,
        content: &DocumentContent,
        options: DocumentTranslateOptions,
    ) -> Result<Vec<TranslatedSegment>, TranslationError>;

    /// Open and translate in one step
    async fn open_and_translate(
        &self,
        file_path: &str,
        options: DocumentTranslateOptions,
    ) -> Result<Vec<TranslatedSegment>, TranslationError> {
        let content = self.open(file_path).await?;
        self.translate(&content, options).await
    }
}

/// Parsed document content ready for translation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentContent {
    /// Document format
    pub format: DocumentFormat,
    /// Source file path
    pub file_path: String,
    /// Extracted text segments (lines, paragraphs, etc.)
    pub segments: Vec<String>,
    /// Total segment count
    pub total_segments: usize,
}
