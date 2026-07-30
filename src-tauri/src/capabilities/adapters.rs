use async_trait::async_trait;
use serde::{Deserialize, Serialize};

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
}

/// Detects the foreground application and its context.
/// Platform-specific implementations determine how to query the OS.
#[async_trait]
pub trait TargetAppDetector: Send + Sync {
    /// Get information about the current foreground application
    async fn detect(&self) -> Option<AppContext>;
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

