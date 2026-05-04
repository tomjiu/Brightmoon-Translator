use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::models::error::TranslationError;
use crate::models::translation::TranslateResponse;
use crate::overlay::OverlayLevel;

/// Result from a selection translation operation
#[derive(Debug, Clone)]
pub struct SelectionTranslationResult {
    /// Original selected text
    pub source_text: String,
    /// Source application info
    pub source_app: String,
    /// Translation response
    pub response: TranslateResponse,
    /// Overlay level used
    pub overlay_level: OverlayLevel,
    /// Which selection provider produced the source text
    pub selection_provider: String,
}

/// Options for selection translation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionTranslateOptions {
    /// Override source language (None = auto-detect)
    pub from: Option<String>,
    /// Override target language (None = use default)
    pub to: Option<String>,
    /// Override overlay level (None = use config default)
    pub overlay_level: Option<u8>,
    /// Whether to show the overlay
    pub show_overlay: bool,
}

impl Default for SelectionTranslateOptions {
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            overlay_level: None,
            show_overlay: true,
        }
    }
}

/// Selection Translation capability.
/// Composes: text selection -> translation -> overlay presentation.
///
/// This is the primary interface for "select text anywhere and translate it".
/// Implementations vary by platform (desktop UIA/clipboard, browser DOM, etc.)
#[async_trait]
pub trait SelectionTranslation: Send + Sync {
    /// Translate the current selection and optionally show overlay
    async fn translate_selection(
        &self,
        options: SelectionTranslateOptions,
    ) -> Result<SelectionTranslationResult, TranslationError>;

    /// Translate specific text as if it were selected
    async fn translate_text(
        &self,
        text: &str,
        options: SelectionTranslateOptions,
    ) -> Result<SelectionTranslationResult, TranslationError>;
}
