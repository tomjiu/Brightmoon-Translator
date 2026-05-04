pub mod clipboard;
pub mod manager;
pub mod uiautomation;

pub use manager::SelectionProviderManager;

use serde::{Deserialize, Serialize};

/// Bounding rectangle for a selection or element on screen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Result returned by any selection provider
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionResult {
    /// The selected text content
    pub text: String,
    /// Name of the application that owns the selection
    pub source_app: String,
    /// Title of the foreground window
    pub window_title: String,
    /// Screen bounds of the selection or source element, if available
    pub bounds: Option<SelectionBounds>,
    /// Confidence score 0.0-1.0 indicating how reliable the selection is
    pub confidence: f32,
    /// Which provider produced this result
    pub provider: &'static str,
}

/// Trait for selection text providers.
/// Each provider knows how to obtain selected text from a specific source.
#[async_trait::async_trait]
pub trait SelectionProvider: Send + Sync {
    /// Try to get the current selection. Returns None if this provider
    /// cannot obtain a selection (e.g., no focused text element for UIA).
    async fn get_selection(&self) -> Option<SelectionResult>;

    /// Human-readable name for this provider
    fn name(&self) -> &'static str;

    /// Priority for automatic selection (lower = tried first)
    fn priority(&self) -> u32;
}
