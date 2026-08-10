//! Tier 4 P2: `ResizeWindowService` — aspect-ratio-constrained resize for pin
//! windows.
//!
//! Reference: snow-shot `resize_window_service.rs` (30fps mouse sampling,
//! aspect-ratio constraint, 4-direction fixed-point).
//!
//! ## What this module provides
//! The pure geometry of an aspect-ratio-preserving resize:
//! - Given the requested new size, enforce the target `width / height` ratio.
//! - Keep the corner **opposite** the dragged handle stationary (4-direction
//!   fixed-point), so the window grows/shrinks toward the cursor rather than
//!   recentering.
//! - Clamp to the pin-card min/max bounds so the card stays usable.
//!
//! ## Integration model
//! The FE implements the 30fps mouse sampling (a `pointermove` handler
//! throttled via `requestAnimationFrame`). On each sampled delta it calls
//! [`compute_aspect_resize`] with the requested size + the fixed anchor
//! corner, receives the constrained `(x, y, w, h)`, and applies it via
//! `window.setPosition` + `window.setSize`. Keeping the sampling on the FE
//! avoids a native mouse hook and lets the webview drive smooth frames.
//!
//! Text translation cards have `aspect_ratio = None` and are freely resizable
//! (text reflows). The constraint is opt-in, primarily for screenshot pin
//! windows (M2) where distorting the aspect ratio would stretch the image.

use serde::{Deserialize, Serialize};

/// Pin-card size bounds — must match `pin_manager::pin_card` clamp bounds.
const MIN_W: f64 = 120.0;
const MIN_H: f64 = 48.0;
const MAX_W: f64 = 480.0;
const MAX_H: f64 = 400.0;

/// Which corner of the window stays fixed during the resize.
///
/// The dragged handle is the **opposite** corner. For example, dragging the
/// bottom-right resize handle keeps `TopLeft` stationary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResizeAnchor {
    /// Dragging bottom-right → top-left stays put.
    TopLeft,
    /// Dragging bottom-left → top-right stays put.
    TopRight,
    /// Dragging top-right → bottom-left stays put.
    BottomLeft,
    /// Dragging top-left → bottom-right stays put.
    BottomRight,
}

impl ResizeAnchor {
    /// Parse an anchor from a case-insensitive string identifier.
    /// Accepts "topLeft"/"top-left"/"top_left", etc. Returns `None` on
    /// unrecognized input so callers can fall back to a default rather than
    /// crashing the resize flow.
    pub fn parse(s: &str) -> Option<Self> {
        let norm: String = s.to_ascii_lowercase().chars().filter(char::is_ascii_alphanumeric).collect();
        match norm.as_str() {
            "topleft" | "tl" => Some(Self::TopLeft),
            "topright" | "tr" => Some(Self::TopRight),
            "bottomleft" | "bl" => Some(Self::BottomLeft),
            "bottomright" | "br" => Some(Self::BottomRight),
            _ => None,
        }
    }
}

/// Constrained resize result — the rect the FE should apply.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AspectResizeResult {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Enforce a target aspect ratio on a requested size, clamped to bounds.
///
/// The ratio is `width / height`. The dimension that the caller requested is
/// honoured first (width-driven), then the other is derived; if the derived
/// value crosses a bound, the bound wins and the primary dimension is
/// re-derived so the ratio is preserved exactly.
fn constrain_size(req_w: f64, req_h: f64, ratio: f64) -> (f64, f64) {
    if ratio <= 0.0 || !ratio.is_finite() {
        // Degenerate ratio → no constraint, just clamp.
        return (req_w.clamp(MIN_W, MAX_W), req_h.clamp(MIN_H, MAX_H));
    }

    // Width-driven: derive height from requested width.
    let mut w = req_w.clamp(MIN_W, MAX_W);
    let mut h = w / ratio;

    // If derived height crosses bounds, pin it and re-derive width so the
    // ratio stays exact (a clamped height would otherwise distort the ratio).
    if h > MAX_H {
        h = MAX_H;
        w = h * ratio;
    } else if h < MIN_H {
        h = MIN_H;
        w = h * ratio;
    }

    // Final clamp on width (the re-derivation above could push it past max).
    if w > MAX_W {
        w = MAX_W;
        h = w / ratio;
    } else if w < MIN_W {
        w = MIN_W;
        h = w / ratio;
    }

    (w, h)
}

/// Compute the aspect-ratio-constrained resize rect for a pin window.
///
/// # Arguments
/// * `origin_x`, `origin_y` — current window top-left (physical px).
/// * `cur_w`, `cur_h` — current window size (physical px).
/// * `req_w`, `req_h` — requested new size (from the drag delta).
/// * `anchor` — the corner that stays fixed (opposite the dragged handle).
/// * `ratio` — target `width / height`. Non-positive / `NaN` disables the
///   constraint (returns the clamped requested size at the fixed corner).
///
/// # Returns
/// The `(x, y, width, height)` rect the FE should apply via
/// `window.setPosition` + `window.setSize`.
pub fn compute_aspect_resize(
    origin_x: f64,
    origin_y: f64,
    cur_w: f64,
    cur_h: f64,
    req_w: f64,
    req_h: f64,
    anchor: ResizeAnchor,
    ratio: f64,
) -> AspectResizeResult {
    let (new_w, new_h) = constrain_size(req_w, req_h, ratio);

    // 4-direction fixed-point: keep the anchor corner stationary by shifting
    // the origin opposite to the growth. The fixed corner's coordinates are
    // computed from the CURRENT rect, then the new top-left is derived so
    // that corner is unchanged.
    let (new_x, new_y) = match anchor {
        ResizeAnchor::TopLeft => (origin_x, origin_y),
        ResizeAnchor::TopRight => {
            // Top-right corner: (origin_x + cur_w, origin_y). Keep it fixed.
            (origin_x + cur_w - new_w, origin_y)
        }
        ResizeAnchor::BottomLeft => {
            // Bottom-left corner: (origin_x, origin_y + cur_h). Keep it fixed.
            (origin_x, origin_y + cur_h - new_h)
        }
        ResizeAnchor::BottomRight => {
            // Bottom-right corner: (origin_x + cur_w, origin_y + cur_h).
            (origin_x + cur_w - new_w, origin_y + cur_h - new_h)
        }
    };

    AspectResizeResult {
        x: new_x.max(0.0),
        y: new_y.max(0.0),
        width: new_w,
        height: new_h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ratio preserved exactly across a range of requested widths.
    #[test]
    fn ratio_preserved_width_driven() {
        let ratio = 16.0 / 9.0; // ~1.778
        for &req_w in &[200.0, 300.0, 400.0] {
            let r = compute_aspect_resize(0.0, 0.0, 300.0, 168.0, req_w, 999.0, ResizeAnchor::TopLeft, ratio);
            let actual = r.width / r.height;
            assert!(
                (actual - ratio).abs() < 1e-9,
                "req_w={req_w}: ratio drifted {actual} vs {ratio}"
            );
        }
    }

    /// Height clamp re-derives width so the ratio stays exact.
    #[test]
    fn height_clamp_preserves_ratio() {
        let ratio = 0.5; // w = h/2, so h grows faster → height clamp triggers first
        // Request a width within MAX_W whose derived height would exceed MAX_H (400).
        // width=480 → h=960 > 400 → clamp h=400, re-derive w=200.
        let r = compute_aspect_resize(0.0, 0.0, 400.0, 200.0, 480.0, 960.0, ResizeAnchor::TopLeft, ratio);
        assert_eq!(r.height, MAX_H);
        assert_eq!(r.width, MAX_H * ratio);
    }

    /// Width clamp re-derives height so the ratio stays exact.
    #[test]
    fn width_clamp_preserves_ratio() {
        let ratio = 0.5; // tall card (w < h)
        // Request a tiny width; derived height huge → height clamps → width re-derives.
        let r = compute_aspect_resize(0.0, 0.0, 200.0, 400.0, 10.0, 10.0, ResizeAnchor::TopLeft, ratio);
        // ratio 0.5 means w = h/2. MIN_W=120 → h=240 (within bounds).
        assert!(r.width >= MIN_W);
        let actual = r.width / r.height;
        assert!((actual - ratio).abs() < 1e-9);
    }

    /// TopLeft anchor: origin unchanged, size grows from top-left.
    #[test]
    fn top_left_anchor_keeps_origin() {
        let r = compute_aspect_resize(100.0, 200.0, 300.0, 150.0, 400.0, 200.0, ResizeAnchor::TopLeft, 2.0);
        assert_eq!(r.x, 100.0);
        assert_eq!(r.y, 200.0);
        assert_eq!(r.width, 400.0);
        assert_eq!(r.height, 200.0); // 400/2
    }

    /// TopRight anchor: top-right corner fixed → x shifts left as width grows.
    #[test]
    fn top_right_anchor_fixes_top_right_corner() {
        // origin (100,200), cur 300×150 → top-right corner at (400, 200).
        // New width 400 → x must be 400 - 400 = 0; y stays 200.
        let r = compute_aspect_resize(100.0, 200.0, 300.0, 150.0, 400.0, 200.0, ResizeAnchor::TopRight, 2.0);
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 200.0);
        assert_eq!(r.width, 400.0);
        assert_eq!(r.height, 200.0);
    }

    /// BottomLeft anchor: bottom-left corner fixed → y shifts up as height grows.
    #[test]
    fn bottom_left_anchor_fixes_bottom_left_corner() {
        // origin (100,200), cur 300×150 → bottom-left at (100, 350).
        // New height 200 → y = 350 - 200 = 150; x stays 100.
        let r = compute_aspect_resize(100.0, 200.0, 300.0, 150.0, 400.0, 200.0, ResizeAnchor::BottomLeft, 2.0);
        assert_eq!(r.x, 100.0);
        assert_eq!(r.y, 150.0);
        assert_eq!(r.width, 400.0);
        assert_eq!(r.height, 200.0);
    }

    /// BottomRight anchor: bottom-right corner fixed → both x,y shift.
    #[test]
    fn bottom_right_anchor_fixes_bottom_right_corner() {
        // origin (100,200), cur 300×150 → bottom-right at (400, 350).
        // New 400×200 → x = 400-400=0, y = 350-200=150.
        let r = compute_aspect_resize(100.0, 200.0, 300.0, 150.0, 400.0, 200.0, ResizeAnchor::BottomRight, 2.0);
        assert_eq!(r.x, 0.0);
        assert_eq!(r.y, 150.0);
        assert_eq!(r.width, 400.0);
        assert_eq!(r.height, 200.0);
    }

    /// Degenerate ratio (≤0 / NaN) disables the constraint → clamped request.
    #[test]
    fn degenerate_ratio_disables_constraint() {
        let r = compute_aspect_resize(0.0, 0.0, 300.0, 150.0, 999.0, 1.0, ResizeAnchor::TopLeft, 0.0);
        assert_eq!(r.width, MAX_W); // clamped, not ratio-derived
        assert_eq!(r.height, 1.0_f64.clamp(MIN_H, MAX_H));

        let r_nan = compute_aspect_resize(0.0, 0.0, 300.0, 150.0, 200.0, 100.0, ResizeAnchor::TopLeft, f64::NAN);
        assert_eq!(r_nan.width, 200.0);
        assert_eq!(r_nan.height, 100.0);
    }

    /// Anchor parser accepts camelCase / kebab / snake + short forms.
    #[test]
    fn anchor_parse_accepts_variants() {
        assert_eq!(ResizeAnchor::parse("topLeft"), Some(ResizeAnchor::TopLeft));
        assert_eq!(ResizeAnchor::parse("top-left"), Some(ResizeAnchor::TopLeft));
        assert_eq!(ResizeAnchor::parse("top_left"), Some(ResizeAnchor::TopLeft));
        assert_eq!(ResizeAnchor::parse("tl"), Some(ResizeAnchor::TopLeft));
        assert_eq!(ResizeAnchor::parse("BR"), Some(ResizeAnchor::BottomRight));
        assert_eq!(ResizeAnchor::parse("bottom-right"), Some(ResizeAnchor::BottomRight));
        assert_eq!(ResizeAnchor::parse("nonsense"), None);
    }

    /// Origin never goes negative (clamped to 0).
    #[test]
    fn origin_clamped_non_negative() {
        // TopRight anchor with large growth would push x negative.
        let r = compute_aspect_resize(10.0, 10.0, 100.0, 100.0, 500.0, 250.0, ResizeAnchor::TopRight, 2.0);
        assert!(r.x >= 0.0);
        assert!(r.y >= 0.0);
    }
}
