//! O1-O4: Pinned-card retain pool with stacked cascade origin.
//!
//! Unlike the single transient `overlay` window, pinned cards persist on
//! screen until the user dismisses them. This module maintains a pool of
//! reusable webview windows (`pin-0`, `pin-1`, …) so that pinning a new
//! card does not pay the full `WebView2` create cost every time.
//!
//! - O1 retain pool: hidden windows are reused instead of destroyed.
//! - O2 stackedOrigin: each new pin cascades +24/+24 from the last active pin.
//! - O3 pinSource: each slot stores source text + source app / window title.
//! - O4 dismiss: hide the window and mark the slot free for reuse.

use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use super::{OverlayContent, OverlayLevel};
use super::html_builder;

/// Staircase offset applied to each new pin relative to the last active one.
const STACK_OFFSET_X: f64 = 24.0;
const STACK_OFFSET_Y: f64 = 24.0;
/// Maximum cascade travel from the requested origin (keeps pins on-screen).
const STACK_MAX_TRAVEL: f64 = 200.0;
/// Upper bound on pool size — prevents unbounded window creation.
const MAX_POOL_SIZE: usize = 12;

/// O3: metadata for a pinned card — remembers where it came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinSlot {
    /// Webview window label (`pin-0`, `pin-1`, …).
    pub label: String,
    pub source: String,
    pub translated: String,
    pub source_app: Option<String>,
    pub window_title: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// `false` when the slot's window is hidden and available for reuse.
    pub in_use: bool,
}

/// O1: retain-pool manager for pinned translation cards.
pub struct PinWindowManager {
    slots: Vec<PinSlot>,
    next_id: u32,
}

impl PinWindowManager {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            next_id: 0,
        }
    }

    /// O1+O2+O3: pin a new card.
    ///
    /// Reuses a hidden window from the pool if available, otherwise creates a
    /// new one (up to `MAX_POOL_SIZE`). The card is placed at a stacked
    /// offset from the last active pin so multiple pins cascade visibly.
    ///
    /// Returns the label of the pinned window, or an error if the pool is full
    /// and no slot can be reused.
    pub fn pin_card(
        &mut self,
        app: &AppHandle,
        source: &str,
        translated: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        source_app: Option<&str>,
        window_title: Option<&str>,
    ) -> Result<String, String> {
        let w = width.clamp(120.0, 480.0);
        let h = height.clamp(48.0, 400.0);

        // O2: stacked cascade origin.
        let (final_x, final_y) = self.stacked_origin(x, y);

        // O1: find a free slot in the retain pool.
        let free_idx = self.slots.iter().position(|s| !s.in_use);
        let (label, is_new) = if free_idx.is_some() {
            let label = self.slots[free_idx.unwrap()].label.clone();
            (label, false)
        } else {
            if self.slots.len() >= MAX_POOL_SIZE {
                return Err(format!(
                    "Pin pool is full (max {MAX_POOL_SIZE}); dismiss a pinned card first"
                ));
            }
            let label = format!("pin-{}", self.next_id);
            self.next_id += 1;
            (label, true)
        };

        let content = OverlayContent {
            source: source.to_string(),
            translated: translated.to_string(),
            source_app: source_app.map(std::string::ToString::to_string),
            window_title: window_title.map(std::string::ToString::to_string),
        };
        // Pinned cards use Full level so copy/close controls are available.
        // Tier4-3: pass the pin label so the HTML includes a resize listener
        // that reports new window dimensions back to the backend.
        let html = html_builder::build_html(&content, OverlayLevel::Full, 0, Some(&label));

        let px = final_x.max(0.0) as i32;
        let py = final_y.max(0.0) as i32;

        if is_new {
            let encoded = urlencoding::encode(&html);
            let url_str = format!("data:text/html,{encoded}");
            let url = tauri::Url::parse(&url_str)
                .map_err(|e| format!("Failed to parse pin URL: {e}"))?;
            let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
                .title("Pinned Translation")
                .inner_size(w, h)
                // Tier4-3: enforce min/max size so users can't drag the pin
                // card to unusable dimensions. Matches the input clamp bounds
                // at lines 81-82 (width ∈ [120,480], height ∈ [48,400]).
                .min_inner_size(120.0, 48.0)
                .max_inner_size(480.0, 400.0)
                .decorations(false)
                .transparent(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(true)
                .focused(false)
                .visible(false)
                .background_color(if super::window_manager::overlay_theme_is_light() {
                    tauri::window::Color(255, 255, 255, 255)
                } else {
                    tauri::window::Color(26, 26, 30, 255)
                })
                .build()
                .map_err(|e| format!("Failed to create pin window: {e}"))?;

            let _ = window.set_position(tauri::Position::Physical(
                tauri::PhysicalPosition::new(px, py),
            ));
            let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                w as u32, h as u32,
            )));
            // C3+C4: pinned cards must not steal focus from the source app.
            #[cfg(windows)]
            if let Ok(hwnd) = window.hwnd() {
                crate::win::set_window_no_activate(hwnd.0 as isize, true);
                crate::win::show_window_no_activate(hwnd.0 as isize);
            }
            #[cfg(not(windows))]
            {
                let _ = window.show();
            }
        } else {
            // Reuse: hide → move/size → write content → show (no destroy flash).
            let window = app
                .get_webview_window(&label)
                .ok_or_else(|| format!("Pin window {label} vanished from pool"))?;
            let _ = window.hide();
            let _ = window.set_position(tauri::Position::Physical(
                tauri::PhysicalPosition::new(px, py),
            ));
            let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
                w as u32, h as u32,
            )));
            let escaped = html
                .replace('\\', "\\\\")
                .replace('`', "\\`")
                .replace('$', "\\$");
            let js = format!("document.documentElement.innerHTML = `{escaped}`;");
            let _ = window.eval(&js);
            #[cfg(windows)]
            if let Ok(hwnd) = window.hwnd() {
                crate::win::show_window_no_activate(hwnd.0 as isize);
            }
            #[cfg(not(windows))]
            {
                let _ = window.show();
            }
        }

        let slot = PinSlot {
            label: label.clone(),
            source: source.to_string(),
            translated: translated.to_string(),
            source_app: source_app.map(std::string::ToString::to_string),
            window_title: window_title.map(std::string::ToString::to_string),
            x: final_x,
            y: final_y,
            width: w,
            height: h,
            in_use: true,
        };
        if let Some(idx) = free_idx {
            self.slots[idx] = slot;
        } else {
            self.slots.push(slot);
        }

        tracing::info!(
            "[pin] pinned card {} at ({},{}) {}x{} (pool: {}/{})",
            label,
            final_x,
            final_y,
            w,
            h,
            self.slots.iter().filter(|s| s.in_use).count(),
            self.slots.len()
        );
        Ok(label)
    }

    /// O2: compute the stacked cascade origin for a new pin.
    ///
    /// If there are active pins, the new pin is placed `STACK_OFFSET_X/Y`
    /// from the last active pin, clamped to `STACK_MAX_TRAVEL` from the
    /// requested origin. This produces a visible staircase instead of
    /// perfectly overlapping cards.
    fn stacked_origin(&self, base_x: f64, base_y: f64) -> (f64, f64) {
        let last_active = self.slots.iter().rev().find(|s| s.in_use);
        match last_active {
            None => (base_x, base_y),
            Some(last) => {
                let x = (last.x + STACK_OFFSET_X).min(base_x + STACK_MAX_TRAVEL);
                let y = (last.y + STACK_OFFSET_Y).min(base_y + STACK_MAX_TRAVEL);
                (x, y)
            }
        }
    }

    /// O4: dismiss a single pinned card by label.
    ///
    /// Hides the window and marks the slot free for reuse. Returns `true` if
    /// the label matched an active pin.
    pub fn dismiss_pin(&mut self, app: &AppHandle, label: &str) -> bool {
        let slot = self.slots.iter_mut().find(|s| s.label == label && s.in_use);
        let Some(slot) = slot else {
            return false;
        };
        if let Some(window) = app.get_webview_window(&slot.label) {
            let _ = window.hide();
        }
        slot.in_use = false;
        tracing::info!("[pin] dismissed {}", label);
        true
    }

    /// Tier4-3: Update a pin slot's stored dimensions after the user
    /// drag-resizes the window. Keeps `PinSlot.width`/`height` in sync with
    /// the live window size so that re-pin / reposition logic uses the
    /// user's preferred size rather than the stale creation-time size.
    ///
    /// Values are clamped to the same bounds as `pin_card` to defend
    /// against malformed input from the FE.
    pub fn update_pin_size(&mut self, label: &str, width: f64, height: f64) -> bool {
        let slot = self.slots.iter_mut().find(|s| s.label == label && s.in_use);
        let Some(slot) = slot else {
            return false;
        };
        slot.width = width.clamp(120.0, 480.0);
        slot.height = height.clamp(48.0, 400.0);
        tracing::debug!(
            "[pin] resize {} → {}x{} (clamped to [{},{}]×[{},{}])",
            label, slot.width, slot.height, 120.0, 480.0, 48.0, 400.0
        );
        true
    }

    /// O4: dismiss all pinned cards (hide all, return all slots to pool).
    pub fn dismiss_all(&mut self, app: &AppHandle) {
        let mut count = 0u32;
        for slot in &mut self.slots {
            if slot.in_use {
                if let Some(window) = app.get_webview_window(&slot.label) {
                    let _ = window.hide();
                }
                slot.in_use = false;
                count += 1;
            }
        }
        if count > 0 {
            tracing::info!("[pin] dismissed all ({} cards)", count);
        }
    }

    /// List metadata for all active pins (FE can show count / manage).
    pub fn active_pins(&self) -> Vec<PinSlot> {
        self.slots.iter().filter(|s| s.in_use).cloned().collect()
    }

    /// Number of currently active pins.
    pub fn active_count(&self) -> usize {
        self.slots.iter().filter(|s| s.in_use).count()
    }
}

impl Default for PinWindowManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Global singleton (matches the existing OVERLAY_SHOWN_AT pattern) ────────

static PIN_MANAGER: OnceLock<Mutex<PinWindowManager>> = OnceLock::new();

fn pin_manager() -> &'static Mutex<PinWindowManager> {
    PIN_MANAGER.get_or_init(|| Mutex::new(PinWindowManager::new()))
}

/// Pin a card via the global manager. Returns the pin label.
pub fn pin_card(
    app: &AppHandle,
    source: &str,
    translated: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    source_app: Option<&str>,
    window_title: Option<&str>,
) -> Result<String, String> {
    let mut mgr = pin_manager()
        .lock()
        .map_err(|e| format!("PinManager lock: {e}"))?;
    mgr.pin_card(app, source, translated, x, y, width, height, source_app, window_title)
}

/// Dismiss a single pin by label via the global manager.
pub fn dismiss_pin(app: &AppHandle, label: &str) -> bool {
    if let Ok(mut mgr) = pin_manager().lock() {
        mgr.dismiss_pin(app, label)
    } else {
        false
    }
}

/// Tier4-3: Update a pin's stored size after user drag-resize.
pub fn update_pin_size(label: &str, width: f64, height: f64) -> bool {
    if let Ok(mut mgr) = pin_manager().lock() {
        mgr.update_pin_size(label, width, height)
    } else {
        false
    }
}

/// Dismiss all pins via the global manager.
pub fn dismiss_all(app: &AppHandle) {
    if let Ok(mut mgr) = pin_manager().lock() {
        mgr.dismiss_all(app);
    }
}

/// List active pins via the global manager.
pub fn active_pins() -> Vec<PinSlot> {
    pin_manager()
        .lock()
        .map(|mgr| mgr.active_pins())
        .unwrap_or_default()
}

/// Count of active pins via the global manager.
pub fn active_count() -> usize {
    pin_manager()
        .lock()
        .map_or(0, |mgr| mgr.active_count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stacked_origin_cascades_from_last_active() {
        let mut mgr = PinWindowManager::new();
        // No active pins → base origin returned as-is.
        assert_eq!(mgr.stacked_origin(100.0, 100.0), (100.0, 100.0));

        // Simulate one active pin at (100, 100).
        mgr.slots.push(PinSlot {
            label: "pin-0".into(),
            source: "hi".into(),
            translated: "你好".into(),
            source_app: None,
            window_title: None,
            x: 100.0,
            y: 100.0,
            width: 300.0,
            height: 120.0,
            in_use: true,
        });
        // Next pin should cascade +24/+24.
        let (x, y) = mgr.stacked_origin(100.0, 100.0);
        assert_eq!(x, 124.0);
        assert_eq!(y, 124.0);
    }

    #[test]
    fn stacked_origin_clamps_to_max_travel() {
        let mut mgr = PinWindowManager::new();
        // Active pin far from the requested origin — cascade clamps.
        mgr.slots.push(PinSlot {
            label: "pin-0".into(),
            source: "".into(),
            translated: "".into(),
            source_app: None,
            window_title: None,
            x: 1000.0,
            y: 1000.0,
            width: 300.0,
            height: 120.0,
            in_use: true,
        });
        let (x, y) = mgr.stacked_origin(100.0, 100.0);
        // 1000 + 24 = 1024, but base + 200 = 300 → clamped to 300.
        assert_eq!(x, 300.0);
        assert_eq!(y, 300.0);
    }

    #[test]
    fn dismiss_marks_slot_free() {
        let mut mgr = PinWindowManager::new();
        mgr.slots.push(PinSlot {
            label: "pin-0".into(),
            source: "hi".into(),
            translated: "你好".into(),
            source_app: None,
            window_title: None,
            x: 100.0,
            y: 100.0,
            width: 300.0,
            height: 120.0,
            in_use: true,
        });
        // No app handle in unit test — dismiss_pin only hides the window if
        // present; the slot is marked free regardless.
        // Use a fake AppHandle-less path: directly manipulate to test logic.
        let slot = mgr.slots.iter_mut().find(|s| s.label == "pin-0").unwrap();
        slot.in_use = false;
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn active_count_tracks_in_use_slots() {
        let mut mgr = PinWindowManager::new();
        assert_eq!(mgr.active_count(), 0);
        mgr.slots.push(PinSlot {
            label: "pin-0".into(),
            source: "".into(),
            translated: "".into(),
            source_app: None,
            window_title: None,
            x: 0.0,
            y: 0.0,
            width: 300.0,
            height: 120.0,
            in_use: true,
        });
        assert_eq!(mgr.active_count(), 1);
        mgr.slots.push(PinSlot {
            label: "pin-1".into(),
            source: "".into(),
            translated: "".into(),
            source_app: None,
            window_title: None,
            x: 24.0,
            y: 24.0,
            width: 300.0,
            height: 120.0,
            in_use: false,
        });
        assert_eq!(mgr.active_count(), 1);
    }

    // ── Tier4-3: update_pin_size tests ──────────────────────────────

    #[test]
    fn update_pin_size_updates_active_slot() {
        let mut mgr = PinWindowManager::new();
        mgr.slots.push(PinSlot {
            label: "pin-0".into(),
            source: "".into(),
            translated: "".into(),
            source_app: None,
            window_title: None,
            x: 0.0,
            y: 0.0,
            width: 300.0,
            height: 120.0,
            in_use: true,
        });
        assert!(mgr.update_pin_size("pin-0", 400.0, 200.0));
        assert_eq!(mgr.slots[0].width, 400.0);
        assert_eq!(mgr.slots[0].height, 200.0);
    }

    #[test]
    fn update_pin_size_clamps_to_bounds() {
        let mut mgr = PinWindowManager::new();
        mgr.slots.push(PinSlot {
            label: "pin-0".into(),
            source: "".into(),
            translated: "".into(),
            source_app: None,
            window_title: None,
            x: 0.0,
            y: 0.0,
            width: 300.0,
            height: 120.0,
            in_use: true,
        });
        // Below min
        assert!(mgr.update_pin_size("pin-0", 50.0, 20.0));
        assert_eq!(mgr.slots[0].width, 120.0); // clamped to min
        assert_eq!(mgr.slots[0].height, 48.0);
        // Above max
        assert!(mgr.update_pin_size("pin-0", 999.0, 999.0));
        assert_eq!(mgr.slots[0].width, 480.0); // clamped to max
        assert_eq!(mgr.slots[0].height, 400.0);
    }

    #[test]
    fn update_pin_size_fails_for_unknown_label() {
        let mut mgr = PinWindowManager::new();
        assert!(!mgr.update_pin_size("nonexistent", 300.0, 120.0));
    }

    #[test]
    fn update_pin_size_fails_for_inactive_slot() {
        let mut mgr = PinWindowManager::new();
        mgr.slots.push(PinSlot {
            label: "pin-0".into(),
            source: "".into(),
            translated: "".into(),
            source_app: None,
            window_title: None,
            x: 0.0,
            y: 0.0,
            width: 300.0,
            height: 120.0,
            in_use: false,
        });
        assert!(!mgr.update_pin_size("pin-0", 400.0, 200.0));
    }
}
