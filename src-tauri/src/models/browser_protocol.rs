//! Shared browser translation protocol models.
//!
//! These types define the wire format for communication between the browser
//! extension and the desktop translation core. Both sides must reference
//! these exact types — no duplicate definitions allowed.
//!
//! The protocol covers three translation modes:
//! - Selection: translate selected text in the page
//! - FullPage: translate all text nodes on the page
//! - Hover: translate text under the cursor
//!
//! Field semantics are aligned with:
//! - `BrowserTranslateOptions` (capabilities/browser_translation.rs)
//! - `TranslateResponse` (models/translation.rs)
//! - `TranslationError` (models/error.rs)
//! - `DomSelection` / `DomBounds` (capabilities/adapters.rs)

use serde::{Deserialize, Serialize};

use super::error::TranslationError;
use super::translation::TranslateResponse;

// ─── Request payloads (extension → desktop) ───────────────────────────

/// Bounds rectangle from DOM (mirrors `DomBounds` from adapters.rs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Selected text payload from browser extension.
/// Maps to `BrowserTranslationSource::Selection(DomSelection)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserSelectionPayload {
    /// The selected text
    pub text: String,
    /// CSS selector of the containing element
    pub selector: Option<String>,
    /// Screen/DOM bounds of the selection
    pub bounds: Option<ProtocolBounds>,
    /// Page URL
    pub url: String,
    /// Page title
    pub title: String,
}

/// Full page content payload from browser extension.
/// Maps to `BrowserTranslationSource::FullPage { url, title, segment_count }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserPagePayload {
    /// Page URL
    pub url: String,
    /// Page title
    pub title: String,
    /// Text segments extracted from the page (each DOM text node)
    pub segments: Vec<PageSegment>,
}

/// A single text segment from a page (for full-page translation).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSegment {
    /// CSS selector path to the element
    pub selector: String,
    /// Original text content
    pub text: String,
    /// Index of this segment in the page (for ordering)
    pub index: usize,
}

/// Hover element payload from browser extension.
/// Maps to `BrowserTranslationSource::Hover(DomSelection)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserHoverPayload {
    /// Text content of the hovered element
    pub text: String,
    /// CSS selector of the hovered element
    pub selector: Option<String>,
    /// Bounds of the hovered element
    pub bounds: Option<ProtocolBounds>,
    /// Page URL
    pub url: String,
    /// Page title
    pub title: String,
}

// ─── Request wrapper (extension → desktop) ────────────────────────────

/// Translation mode — aligned with `BrowserTranslationMode`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserTranslateMode {
    Selection,
    FullPage,
    Hover,
}

/// Unified translation request from browser extension to desktop.
/// Wraps the payload with shared options.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTranslateRequest {
    /// Translation mode
    pub mode: BrowserTranslateMode,
    /// The payload (variant determined by `mode`)
    pub payload: BrowserTranslatePayload,
    /// Source language override (None = auto-detect)
    pub from: Option<String>,
    /// Target language override (None = use default)
    pub to: Option<String>,
    /// Whether to show in-page overlay
    pub show_overlay: bool,
    /// Whether to replace text inline
    pub replace_inline: bool,
}

/// Payload variant for the translation request.
/// The active variant must match the `mode` field in `BrowserTranslateRequest`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum BrowserTranslatePayload {
    Selection(BrowserSelectionPayload),
    FullPage(BrowserPagePayload),
    Hover(BrowserHoverPayload),
}

// ─── Response types (desktop → extension) ─────────────────────────────

/// Successful translation response to browser extension.
/// Wraps `TranslateResponse` with browser-specific metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTranslateResponse {
    /// The mode that was used
    pub mode: BrowserTranslateMode,
    /// Translation results (aligned with `TranslateResponse`)
    pub response: TranslateResponse,
    /// Whether an overlay was shown
    pub overlay_shown: bool,
    /// Whether text was replaced inline
    pub replaced_inline: bool,
    /// For full-page mode: per-segment translations
    pub segment_translations: Option<Vec<SegmentTranslation>>,
}

/// Translation result for a single page segment (full-page mode).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentTranslation {
    /// CSS selector of the element to replace
    pub selector: String,
    /// Original text
    pub original: String,
    /// Translated text
    pub translated: String,
    /// Index matching the original segment
    pub index: usize,
}

// ─── Error response (desktop → extension) ─────────────────────────────

/// Error response to browser extension.
/// Wraps `TranslationError` with a user-facing message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTranslateError {
    /// Structured error type (aligned with `TranslationError`)
    pub error: TranslationError,
    /// User-facing error message
    pub message: String,
}

// ─── Action payloads (desktop → extension) ────────────────────────────

/// Instruction to show an overlay in the browser page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserOverlayPayload {
    /// The translated text to display
    pub translated: String,
    /// The original source text
    pub source: String,
    /// Where to position the overlay (near the selection)
    pub bounds: Option<ProtocolBounds>,
    /// Overlay level (1=minimal, 2=standard, 3=full)
    pub level: u8,
    /// Auto-dismiss timeout in milliseconds (0 = no auto-dismiss)
    pub dismiss_ms: u64,
}

/// Instruction to replace text inline in the browser page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserReplacePayload {
    /// CSS selector of the element to replace
    pub selector: String,
    /// The translated text to insert
    pub translated: String,
    /// The original text (for verification)
    pub original: String,
}
