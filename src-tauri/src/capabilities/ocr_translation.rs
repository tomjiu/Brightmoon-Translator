use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::models::error::TranslationError;
use crate::models::translation::TranslateResponse;

/// Result from an OCR translation operation
#[derive(Debug, Clone)]
pub struct OcrTranslationResult {
    /// OCR-recognized text
    pub recognized_text: String,
    /// Translation response
    pub response: TranslateResponse,
    /// OCR confidence (0.0 - 1.0)
    pub confidence: f32,
    /// Source image dimensions
    pub image_width: u32,
    pub image_height: u32,
}

/// Options for OCR translation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrTranslateOptions {
    /// Override source language (None = auto-detect)
    pub from: Option<String>,
    /// Override target language (None = use default)
    pub to: Option<String>,
    /// Whether to show the overlay
    pub show_overlay: bool,
    /// Whether to auto-copy the result
    pub auto_copy: bool,
}

impl Default for OcrTranslateOptions {
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            show_overlay: true,
            auto_copy: false,
        }
    }
}

/// OCR Translation capability.
/// Composes: screen capture -> OCR -> translation -> overlay presentation.
///
/// This is the interface for "screenshot a region and translate the text".
#[async_trait]
pub trait OcrTranslation: Send + Sync {
    /// Capture a screen region, OCR it, and translate
    async fn capture_and_translate(
        &self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        options: OcrTranslateOptions,
    ) -> Result<OcrTranslationResult, TranslationError>;

    /// OCR and translate from an existing image (base64 PNG)
    async fn translate_image(
        &self,
        image_base64: &str,
        options: OcrTranslateOptions,
    ) -> Result<OcrTranslationResult, TranslationError>;

    /// Capture full screen, OCR, and translate
    async fn fullscreen_translate(
        &self,
        options: OcrTranslateOptions,
    ) -> Result<OcrTranslationResult, TranslationError>;
}
