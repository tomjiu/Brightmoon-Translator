pub mod auto_watch;
pub mod clipboard;
pub mod hover_pick;
pub mod manager;
#[cfg(windows)]
pub mod mouse_hook;
pub mod pop_button;
pub mod present;
pub mod process_class;
pub mod uiautomation;
#[cfg(windows)]
pub mod win_noactivate;

pub use auto_watch::SelectionAutoWatch;
pub use manager::SelectionProviderManager;
pub use process_class::{foreground_process, SelectionStrategy};

use serde::{Deserialize, Serialize};

/// OCR force pickup allowed: switch on + optional modifier held.
pub fn ocr_force_allowed(ux: &crate::config::SelectionUxConfig) -> bool {
    ux.ocr_force_pickup && modifier_key_satisfied(&ux.ocr_modifier_key)
}

/// Whether OCR-force modifier is satisfied (`""`/`none` = always ok).
/// Keys: shift | ctrl | alt (either left/right).
pub fn modifier_key_satisfied(key: &str) -> bool {
    let k = key.trim().to_ascii_lowercase();
    if k.is_empty() || k == "none" || k == "off" {
        return true;
    }
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_RCONTROL, VK_RMENU, VK_RSHIFT,
        };
        // SAFETY: GetAsyncKeyState is a pure Win32 query (i32 vk → i16) with
        // no preconditions. Captured by the closure for each call below.
        let down = |vk: i32| unsafe { GetAsyncKeyState(vk) as u16 & 0x8000 != 0 };
        match k.as_str() {
            "shift" | "lshift" | "rshift" => down(VK_LSHIFT.0 as i32) || down(VK_RSHIFT.0 as i32),
            "ctrl" | "control" | "lctrl" | "rctrl" => {
                down(VK_LCONTROL.0 as i32) || down(VK_RCONTROL.0 as i32)
            },
            "alt" | "menu" | "lalt" | "ralt" => down(VK_LMENU.0 as i32) || down(VK_RMENU.0 as i32),
            _ => true, // unknown → don't hard-block
        }
    }
    #[cfg(not(windows))]
    {
        let _ = k;
        true
    }
}

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
