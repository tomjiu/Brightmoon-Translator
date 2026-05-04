use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::adapters::DomSelection;
use crate::models::error::TranslationError;
use crate::models::translation::TranslateResponse;

/// Options for browser translation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTranslateOptions {
    /// Override source language (None = auto-detect)
    pub from: Option<String>,
    /// Override target language (None = use default)
    pub to: Option<String>,
    /// Translation mode
    pub mode: BrowserTranslationMode,
    /// Whether to show an in-page overlay
    pub show_overlay: bool,
    /// Whether to replace text inline
    pub replace_inline: bool,
}

impl Default for BrowserTranslateOptions {
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            mode: BrowserTranslationMode::Selection,
            show_overlay: true,
            replace_inline: false,
        }
    }
}

/// Browser translation modes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserTranslationMode {
    /// Translate selected text only
    Selection,
    /// Translate entire page
    FullPage,
    /// Translate hovered element
    Hover,
}

/// Source context for browser translation
#[derive(Debug, Clone)]
pub enum BrowserTranslationSource {
    /// A DOM text selection with bounds info
    Selection(DomSelection),
    /// Full page translation (no single selection)
    FullPage {
        url: String,
        title: String,
        segment_count: usize,
    },
    /// Hovered element
    Hover(DomSelection),
}

/// Result from browser translation
#[derive(Debug, Clone)]
pub struct BrowserTranslationResult {
    /// Source context that was translated
    pub source: BrowserTranslationSource,
    /// Translation response
    pub response: TranslateResponse,
    /// Whether overlay was shown
    pub overlay_shown: bool,
    /// Whether text was replaced inline
    pub replaced_inline: bool,
}

/// Browser Translation capability.
/// Composes: DOM selection -> translation -> in-page overlay/replace.
///
/// This is the interface for browser extension translation.
/// The browser extension communicates with the translation core
/// through this trait.
#[async_trait]
pub trait BrowserTranslation: Send + Sync {
    /// Translate the current DOM selection
    async fn translate_selection(
        &self,
        selection: DomSelection,
        options: BrowserTranslateOptions,
    ) -> Result<BrowserTranslationResult, TranslationError>;

    /// Translate an entire page (extract all text nodes)
    async fn translate_page(
        &self,
        url: &str,
        options: BrowserTranslateOptions,
    ) -> Result<BrowserTranslationResult, TranslationError>;

    /// Translate text from a hover event
    async fn translate_hover(
        &self,
        selection: DomSelection,
        options: BrowserTranslateOptions,
    ) -> Result<BrowserTranslationResult, TranslationError>;
}
