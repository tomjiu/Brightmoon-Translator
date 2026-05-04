use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::adapters::DomSelection;
use crate::config::AppConfig;
use crate::models::browser_protocol::*;
use crate::models::error::TranslationError;
use crate::models::translation::{TranslateResponse, TranslationResult};
use crate::services::TranslationService;

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

// ─── Protocol bridge helpers ──────────────────────────────────────────

/// Convert a `BrowserSelectionPayload` (protocol) into a `DomSelection` (internal).
pub fn selection_payload_to_dom(payload: &BrowserSelectionPayload) -> DomSelection {
    DomSelection {
        text: payload.text.clone(),
        selector: payload.selector.clone(),
        bounds: payload.bounds.as_ref().map(|b| super::adapters::DomBounds {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        }),
        url: payload.url.clone(),
        title: payload.title.clone(),
    }
}

/// Convert a `BrowserHoverPayload` (protocol) into a `DomSelection` (internal).
pub fn hover_payload_to_dom(payload: &BrowserHoverPayload) -> DomSelection {
    DomSelection {
        text: payload.text.clone(),
        selector: payload.selector.clone(),
        bounds: payload.bounds.as_ref().map(|b| super::adapters::DomBounds {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        }),
        url: payload.url.clone(),
        title: payload.title.clone(),
    }
}

/// Mock browser request handler that demonstrates the protocol bridge.
/// Proves that `BrowserTranslateRequest` → translation → `BrowserTranslateResponse`
/// is a complete, serializable round-trip.
///
/// In production this would call `TranslationService`; here we echo the text
/// to prove the protocol types compose correctly.
pub fn mock_handle_browser_request(
    request: &BrowserTranslateRequest,
) -> Result<BrowserTranslateResponse, BrowserTranslateError> {
    // Extract text from the payload based on mode
    let text = match &request.payload {
        BrowserTranslatePayload::Selection(p) => p.text.clone(),
        BrowserTranslatePayload::FullPage(p) => {
            p.segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ")
        }
        BrowserTranslatePayload::Hover(p) => p.text.clone(),
    };

    if text.trim().is_empty() {
        return Err(BrowserTranslateError {
            error: TranslationError::InvalidInput("No text to translate".to_string()),
            message: "No text to translate".to_string(),
        });
    }

    // Mock: echo back the text as "translated"
    let response = TranslateResponse {
        results: vec![TranslationResult {
            engine: "mock".to_string(),
            text: format!("[translated] {}", text),
        }],
        detected_language: Some("en".to_string()),
    };

    // For full-page mode, build per-segment translations
    let segment_translations = if request.mode == BrowserTranslateMode::FullPage {
        if let BrowserTranslatePayload::FullPage(page) = &request.payload {
            Some(
                page.segments
                    .iter()
                    .map(|s| SegmentTranslation {
                        selector: s.selector.clone(),
                        original: s.text.clone(),
                        translated: format!("[translated] {}", s.text),
                        index: s.index,
                    })
                    .collect(),
            )
        } else {
            None
        }
    } else {
        None
    };

    Ok(BrowserTranslateResponse {
        mode: request.mode.clone(),
        response,
        overlay_shown: request.show_overlay,
        replaced_inline: request.replace_inline,
        segment_translations,
    })
}

/// Production browser request handler.
/// Dispatches a `BrowserTranslateRequest` through the real `TranslationService`
/// pipeline (glossary, blacklist, cache, engine routing, history).
pub async fn handle_browser_request(
    request: &BrowserTranslateRequest,
    translation_service: &TranslationService,
    config: &AppConfig,
) -> Result<BrowserTranslateResponse, BrowserTranslateError> {
    let from = request
        .from
        .as_deref()
        .unwrap_or(&config.default_from);
    let to = request
        .to
        .as_deref()
        .unwrap_or(&config.default_to);

    match &request.payload {
        BrowserTranslatePayload::Selection(sel) => {
            if sel.text.trim().is_empty() {
                return Err(BrowserTranslateError {
                    error: TranslationError::InvalidInput("No text to translate".to_string()),
                    message: "No text to translate".to_string(),
                });
            }
            let response = translation_service
                .translate(&sel.text, from, to)
                .await
                .map_err(|e| BrowserTranslateError {
                    message: e.to_string(),
                    error: e,
                })?;
            Ok(BrowserTranslateResponse {
                mode: BrowserTranslateMode::Selection,
                response,
                overlay_shown: request.show_overlay,
                replaced_inline: request.replace_inline,
                segment_translations: None,
            })
        }
        BrowserTranslatePayload::FullPage(page) => {
            if page.segments.is_empty() {
                return Err(BrowserTranslateError {
                    error: TranslationError::InvalidInput("No segments to translate".to_string()),
                    message: "No segments to translate".to_string(),
                });
            }
            let lines: Vec<(usize, &str)> = page
                .segments
                .iter()
                .enumerate()
                .map(|(i, s)| (i, s.text.as_str()))
                .collect();
            let batch_results = translation_service
                .translate_batch(&lines, from, to, 3)
                .await;

            let segment_translations: Vec<SegmentTranslation> = batch_results
                .into_iter()
                .zip(page.segments.iter())
                .map(|(result, seg)| SegmentTranslation {
                    selector: seg.selector.clone(),
                    original: seg.text.clone(),
                    translated: result.translated,
                    index: seg.index,
                })
                .collect();

            let combined_text = segment_translations
                .iter()
                .map(|s| s.translated.as_str())
                .collect::<Vec<_>>()
                .join(" ");

            Ok(BrowserTranslateResponse {
                mode: BrowserTranslateMode::FullPage,
                response: TranslateResponse {
                    results: vec![TranslationResult {
                        engine: "batch".into(),
                        text: combined_text,
                    }],
                    detected_language: None,
                },
                overlay_shown: request.show_overlay,
                replaced_inline: request.replace_inline,
                segment_translations: Some(segment_translations),
            })
        }
        BrowserTranslatePayload::Hover(hover) => {
            if hover.text.trim().is_empty() {
                return Err(BrowserTranslateError {
                    error: TranslationError::InvalidInput("No text to translate".to_string()),
                    message: "No text to translate".to_string(),
                });
            }
            let response = translation_service
                .translate(&hover.text, from, to)
                .await
                .map_err(|e| BrowserTranslateError {
                    message: e.to_string(),
                    error: e,
                })?;
            Ok(BrowserTranslateResponse {
                mode: BrowserTranslateMode::Hover,
                response,
                overlay_shown: request.show_overlay,
                replaced_inline: request.replace_inline,
                segment_translations: None,
            })
        }
    }
}
