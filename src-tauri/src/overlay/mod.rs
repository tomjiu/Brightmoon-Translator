pub mod html_builder;
pub mod interaction;
pub mod positioner;
pub mod window_manager;

use serde::{Deserialize, Serialize};

/// Overlay display level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverlayLevel {
    /// L1: Minimal - translated text only, auto-dismiss
    Minimal = 1,
    /// L2: Standard - source + translated, copy button
    Standard = 2,
    /// L3: Full - source + translated, all controls
    Full = 3,
}

impl From<u8> for OverlayLevel {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::Minimal,
            2 => Self::Standard,
            _ => Self::Full,
        }
    }
}

/// Overlay lifecycle state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverlayState {
    /// Auto-dismiss after timeout (L1 default)
    Transient,
    /// Stays until user closes (L2 default)
    Interactive,
    /// Always-on-top, user must explicitly close (L3 can switch to this)
    Pinned,
}

/// Configuration for overlay display
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    pub level: OverlayLevel,
    pub state: OverlayState,
    pub dismiss_ms: u64,
    pub follow_cursor: bool,
    pub follow_target_bounds: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            level: OverlayLevel::Standard,
            state: OverlayState::Interactive,
            dismiss_ms: 3000,
            follow_cursor: true,
            follow_target_bounds: false,
        }
    }
}

/// Data for rendering an overlay
#[derive(Debug, Clone)]
pub struct OverlayContent {
    pub source: String,
    pub translated: String,
    pub source_app: Option<String>,
    pub window_title: Option<String>,
}

/// Position hint for overlay placement
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OverlayPosition {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl OverlayPosition {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    /// Create position at cursor + offset
    pub fn at_cursor(cursor_x: f64, cursor_y: f64) -> Self {
        Self {
            x: cursor_x + 10.0,
            y: cursor_y + 10.0,
            width: 350.0,
            height: 200.0,
        }
    }

    /// Create position below a target bounds
    pub fn below_bounds(bounds_x: f64, bounds_y: f64, bounds_w: f64, bounds_h: f64) -> Self {
        Self {
            x: bounds_x,
            y: bounds_y + bounds_h + 8.0,
            width: bounds_w.max(300.0).min(500.0),
            height: 200.0,
        }
    }
}
