use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::models::translation::TranslateResponse;

/// Information about the foreground application
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppContext {
    /// Application name (e.g., "chrome.exe", "code.exe")
    pub app_name: String,
    /// Window title
    pub window_title: String,
    /// Process ID
    pub pid: u32,
    /// Whether the app is an Electron/WebView2/CEF app
    pub is_embedded: bool,
    /// Embedded app type if applicable
    pub embedded_type: Option<EmbeddedAppType>,
}

/// Type of embedded application container
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddedAppType {
    Electron,
    WebView2,
    Cef,
}

/// Detects the foreground application and its context.
/// Platform-specific implementations determine how to query the OS.
#[async_trait]
pub trait TargetAppDetector: Send + Sync {
    /// Get information about the current foreground application
    async fn detect(&self) -> Option<AppContext>;

    /// Check if the foreground app is a known embedded app
    async fn is_embedded_app(&self) -> bool {
        self.detect()
            .await
            .map(|ctx| ctx.is_embedded)
            .unwrap_or(false)
    }
}

/// Adapter for translating text within embedded applications
/// (Electron, WebView2, CEF). These apps may expose their own
/// DOM/accessibility trees that can be queried for selection.
#[async_trait]
pub trait EmbeddedAppAdapter: Send + Sync {
    /// The type of embedded app this adapter handles
    fn app_type(&self) -> EmbeddedAppType;

    /// Try to get selected text from the embedded app
    /// Returns None if the app doesn't support this
    async fn get_selection(&self, app: &AppContext) -> Option<String>;

    /// Try to replace text in the embedded app
    /// Returns true if replacement was successful
    async fn replace_text(&self, app: &AppContext, text: &str) -> bool;

    /// Check if this adapter can handle the given app
    fn can_handle(&self, app: &AppContext) -> bool {
        app.embedded_type.as_ref() == Some(&self.app_type())
    }
}

/// Result from a DOM-based selection (browser extension)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomSelection {
    /// Selected text
    pub text: String,
    /// CSS selector of the containing element
    pub selector: Option<String>,
    /// Bounding rectangle of the selection
    pub bounds: Option<DomBounds>,
    /// Page URL
    pub url: String,
    /// Page title
    pub title: String,
}

/// Bounding rectangle from DOM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Provider for DOM-based text selection (browser extension).
/// This interface is what a browser extension would implement
/// to communicate selection data to the translation core.
#[async_trait]
pub trait DomSelectionProvider: Send + Sync {
    /// Get the current selection from the DOM
    async fn get_selection(&self) -> Option<DomSelection>;

    /// Inject translated text as an overlay near the selection
    async fn show_overlay(&self, selection: &DomSelection, response: &TranslateResponse) -> bool;

    /// Replace the selected text in the DOM with translated text
    async fn replace_selection(&self, selection: &DomSelection, translated: &str) -> bool;
}

/// Input method adapter for replacing text in any application.
/// Used by the "replace translate" feature.
#[async_trait]
pub trait InputAdapter: Send + Sync {
    /// Replace the current selection with the given text
    async fn replace_selection(&self, text: &str) -> bool;

    /// Simulate typing the given text
    async fn type_text(&self, text: &str) -> bool;

    /// Get the platform name
    fn platform(&self) -> &str;
}
