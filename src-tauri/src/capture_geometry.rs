//! C2: Centralized capture geometry.
//!
//! Unifies monitor enumeration, logical↔physical conversion, and rect
//! clamping that was previously scattered across `window.rs` (MonitorBounds),
//! `capture.rs` (ScreenshotSnapshotInfo), and `positioner.rs`
//! (monitor_work_area).
//!
//! All coordinates in this module are **physical pixels** unless the name
//! contains `logical`. Scale factors are `f32` (matching Tauri's convention).

use serde::{Deserialize, Serialize};

/// A monitor's physical geometry + DPI scale.
///
/// `position` and `size` are in physical pixels relative to the virtual
/// desktop origin (which may be negative on multi-monitor rigs).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MonitorRegion {
    /// Physical x of the monitor's top-left, relative to virtual desktop.
    pub x: f64,
    /// Physical y of the monitor's top-left, relative to virtual desktop.
    pub y: f64,
    /// Physical width in pixels.
    pub width: f64,
    /// Physical height in pixels.
    pub height: f64,
    /// DPI scale factor (1.0 = 96 DPI, 1.5 = 144 DPI, 2.0 = 192 DPI).
    pub scale_factor: f32,
}

impl MonitorRegion {
    /// Physical center of this monitor.
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Returns `true` if the point (physical, virtual-desktop-relative) is
    /// inside this monitor.
    pub fn contains_point(&self, px: f64, py: f64) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    /// Returns `true` if the rect's center is inside this monitor.
    pub fn contains_rect_center(&self, x: f64, y: f64, w: f64, h: f64) -> bool {
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        self.contains_point(cx, cy)
    }

    /// Convert a logical (CSS) length to physical pixels on this monitor.
    pub fn logical_to_physical(&self, logical: f64) -> f64 {
        logical * self.scale_factor as f64
    }

    /// Convert a physical length to logical (CSS) pixels on this monitor.
    pub fn physical_to_logical(&self, physical: f64) -> f64 {
        physical / self.scale_factor as f64
    }
}

/// A rectangle in physical pixels, virtual-desktop-relative.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PhysicalRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl PhysicalRect {
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }
}

/// Find the monitor whose rect contains the center of the given physical rect.
///
/// Returns `None` if `monitors` is empty. Falls back to the first monitor if
/// no monitor contains the center (can happen with gaps between monitors).
pub fn monitor_for_rect_center(
    monitors: &[MonitorRegion],
    rect: PhysicalRect,
) -> Option<MonitorRegion> {
    if monitors.is_empty() {
        return None;
    }
    for m in monitors {
        if m.contains_rect_center(rect.x, rect.y, rect.width, rect.height) {
            return Some(*m);
        }
    }
    // Fallback: closest monitor by center-to-center distance
    let (cx, cy) = rect.center();
    let mut best = monitors[0];
    let mut best_dist = f64::MAX;
    for m in monitors {
        let (mx, my) = m.center();
        let d = (mx - cx).powi(2) + (my - cy).powi(2);
        if d < best_dist {
            best_dist = d;
            best = *m;
        }
    }
    Some(best)
}

/// Find the monitor containing a physical point.
pub fn monitor_for_point(
    monitors: &[MonitorRegion],
    px: f64,
    py: f64,
) -> Option<MonitorRegion> {
    for m in monitors {
        if m.contains_point(px, py) {
            return Some(*m);
        }
    }
    monitor_for_rect_center(monitors, PhysicalRect { x: px, y: py, width: 1.0, height: 1.0 })
}

/// Clamp a physical rect to fit within the bounding monitor.
///
/// If the rect extends past the monitor's edges, it's shifted and/or shrunk
/// so that it stays fully inside. Returns the clamped rect and the monitor.
pub fn clamp_rect_to_monitor(
    monitors: &[MonitorRegion],
    rect: PhysicalRect,
) -> (PhysicalRect, Option<MonitorRegion>) {
    let monitor = monitor_for_rect_center(monitors, rect);
    let Some(m) = monitor else {
        return (rect, None);
    };

    let mut x = rect.x;
    let mut y = rect.y;
    let mut w = rect.width;
    let mut h = rect.height;

    // Shrink to monitor size if larger
    if w > m.width {
        w = m.width;
    }
    if h > m.height {
        h = m.height;
    }

    // Shift to keep inside monitor
    if x < m.x {
        x = m.x;
    }
    if y < m.y {
        y = m.y;
    }
    if x + w > m.x + m.width {
        x = m.x + m.width - w;
    }
    if y + h > m.y + m.height {
        y = m.y + m.height - h;
    }

    (PhysicalRect { x, y, width: w, height: h }, Some(m))
}

/// Convert a logical (CSS-pixel) rect to a physical rect using the scale
/// of the monitor at the rect's center.
///
/// This is the canonical logical→physical conversion for window placement
/// and OCR region sizing.
pub fn logical_rect_to_physical(
    monitors: &[MonitorRegion],
    logical_x: f64,
    logical_y: f64,
    logical_w: f64,
    logical_h: f64,
    fallback_scale: f32,
) -> (PhysicalRect, f32) {
    let scale = monitor_for_rect_center(
        monitors,
        PhysicalRect {
            x: logical_x,
            y: logical_y,
            width: logical_w,
            height: logical_h,
        },
    )
    .map(|m| m.scale_factor)
    .unwrap_or(fallback_scale);

    let s = scale as f64;
    (
        PhysicalRect {
            x: logical_x * s,
            y: logical_y * s,
            width: logical_w * s,
            height: logical_h * s,
        },
        scale,
    )
}

/// Convert a physical rect to a logical (CSS-pixel) rect using the scale
/// of the monitor at the rect's center.
pub fn physical_rect_to_logical(
    monitors: &[MonitorRegion],
    physical: PhysicalRect,
    fallback_scale: f32,
) -> (f64, f64, f64, f64, f32) {
    let scale = monitor_for_rect_center(monitors, physical)
        .map(|m| m.scale_factor)
        .unwrap_or(fallback_scale);

    let s = scale as f64;
    (physical.x / s, physical.y / s, physical.width / s, physical.height / s, scale)
}

/// Enumerate all monitors via Tauri's `available_monitors()` and convert
/// them to `MonitorRegion`s. Returns an empty vec if enumeration fails.
pub fn enumerate_monitors(app: &tauri::AppHandle) -> Vec<MonitorRegion> {
    app.available_monitors()
        .map(|monitors| {
            monitors
                .iter()
                .map(|m| {
                    let pos = m.position();
                    let size = m.size();
                    MonitorRegion {
                        x: f64::from(pos.x),
                        y: f64::from(pos.y),
                        width: f64::from(size.width),
                        height: f64::from(size.height),
                        scale_factor: m.scale_factor() as f32,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Get the primary monitor's scale factor as a fallback.
pub fn primary_scale_factor(app: &tauri::AppHandle) -> f32 {
    app.primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.scale_factor() as f32)
        .unwrap_or(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(x: f64, y: f64, w: f64, h: f64, scale: f32) -> MonitorRegion {
        MonitorRegion {
            x,
            y,
            width: w,
            height: h,
            scale_factor: scale,
        }
    }

    #[test]
    fn test_contains_point() {
        let m = mon(0.0, 0.0, 1920.0, 1080.0, 1.0);
        assert!(m.contains_point(100.0, 100.0));
        assert!(m.contains_point(0.0, 0.0));
        assert!(!m.contains_point(1920.0, 0.0)); // right edge is exclusive
        assert!(!m.contains_point(-1.0, 0.0));
        assert!(!m.contains_point(0.0, 1080.0));
    }

    #[test]
    fn test_contains_rect_center() {
        let m = mon(0.0, 0.0, 1920.0, 1080.0, 1.0);
        // center at (960, 540) — inside
        assert!(m.contains_rect_center(0.0, 0.0, 1920.0, 1080.0));
        // center at (2880, 540) — outside
        assert!(!m.contains_rect_center(1920.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn test_monitor_for_rect_center_hit() {
        let monitors = vec![
            mon(-1280.0, 0.0, 1280.0, 1024.0, 1.25),
            mon(0.0, 0.0, 1920.0, 1080.0, 1.5),
        ];
        // rect on right monitor (center 960, 540)
        let rect = PhysicalRect { x: 100.0, y: 100.0, width: 200.0, height: 200.0 };
        let m = monitor_for_rect_center(&monitors, rect).unwrap();
        assert_eq!(m.scale_factor, 1.5);
    }

    #[test]
    fn test_monitor_for_rect_center_left_monitor() {
        let monitors = vec![
            mon(-1280.0, 0.0, 1280.0, 1024.0, 1.25),
            mon(0.0, 0.0, 1920.0, 1080.0, 1.5),
        ];
        // rect on left monitor (center -600, 500)
        let rect = PhysicalRect { x: -700.0, y: 400.0, width: 200.0, height: 200.0 };
        let m = monitor_for_rect_center(&monitors, rect).unwrap();
        assert_eq!(m.scale_factor, 1.25);
    }

    #[test]
    fn test_monitor_for_rect_center_fallback_closest() {
        let monitors = vec![
            mon(0.0, 0.0, 1920.0, 1080.0, 1.0),
            mon(3000.0, 0.0, 1920.0, 1080.0, 1.5), // gap between monitors
        ];
        // rect in the gap (center 2400, 540) — closest to first by distance
        let rect = PhysicalRect { x: 2300.0, y: 400.0, width: 200.0, height: 200.0 };
        let m = monitor_for_rect_center(&monitors, rect).unwrap();
        // Distance to monitor 1 center (960, 540): sqrt((2400-960)^2) = 1440
        // Distance to monitor 2 center (3960, 540): sqrt((3960-2400)^2) = 1560
        assert_eq!(m.scale_factor, 1.0);
    }

    #[test]
    fn test_monitor_for_rect_center_empty() {
        let monitors: Vec<MonitorRegion> = vec![];
        let rect = PhysicalRect { x: 0.0, y: 0.0, width: 100.0, height: 100.0 };
        assert!(monitor_for_rect_center(&monitors, rect).is_none());
    }

    #[test]
    fn test_clamp_rect_to_monitor_simple() {
        let monitors = vec![mon(0.0, 0.0, 1920.0, 1080.0, 1.0)];
        let rect = PhysicalRect { x: -50.0, y: -50.0, width: 200.0, height: 200.0 };
        let (clamped, m) = clamp_rect_to_monitor(&monitors, rect);
        assert_eq!(clamped.x, 0.0);
        assert_eq!(clamped.y, 0.0);
        assert_eq!(clamped.width, 200.0);
        assert_eq!(clamped.height, 200.0);
        assert!(m.is_some());
    }

    #[test]
    fn test_clamp_rect_to_monitor_oversized() {
        let monitors = vec![mon(0.0, 0.0, 1920.0, 1080.0, 1.0)];
        // rect larger than monitor
        let rect = PhysicalRect { x: -100.0, y: -100.0, width: 3000.0, height: 2000.0 };
        let (clamped, _) = clamp_rect_to_monitor(&monitors, rect);
        assert_eq!(clamped.x, 0.0);
        assert_eq!(clamped.y, 0.0);
        assert_eq!(clamped.width, 1920.0);
        assert_eq!(clamped.height, 1080.0);
    }

    #[test]
    fn test_clamp_rect_to_monitor_right_edge() {
        let monitors = vec![mon(0.0, 0.0, 1920.0, 1080.0, 1.0)];
        // rect extends past right edge
        let rect = PhysicalRect { x: 1800.0, y: 100.0, width: 200.0, height: 200.0 };
        let (clamped, _) = clamp_rect_to_monitor(&monitors, rect);
        // x + w = 1800 + 200 = 2000 > 1920, so x = 1920 - 200 = 1720
        assert_eq!(clamped.x, 1720.0);
        assert_eq!(clamped.width, 200.0);
    }

    #[test]
    fn test_clamp_rect_empty_monitors() {
        let monitors: Vec<MonitorRegion> = vec![];
        let rect = PhysicalRect { x: 100.0, y: 100.0, width: 200.0, height: 200.0 };
        let (clamped, m) = clamp_rect_to_monitor(&monitors, rect);
        assert_eq!(clamped.x, 100.0); // unchanged
        assert!(m.is_none());
    }

    #[test]
    fn test_logical_to_physical_conversion() {
        let monitors = vec![mon(0.0, 0.0, 2880.0, 1620.0, 1.5)]; // 1920x1080 logical
        let (phys, scale) = logical_rect_to_physical(&monitors, 100.0, 100.0, 400.0, 300.0, 1.0);
        assert_eq!(scale, 1.5);
        assert_eq!(phys.x, 150.0);
        assert_eq!(phys.y, 150.0);
        assert_eq!(phys.width, 600.0);
        assert_eq!(phys.height, 450.0);
    }

    #[test]
    fn test_physical_to_logical_conversion() {
        let monitors = vec![mon(0.0, 0.0, 2880.0, 1620.0, 1.5)];
        let phys = PhysicalRect { x: 150.0, y: 150.0, width: 600.0, height: 450.0 };
        let (lx, ly, lw, lh, scale) = physical_rect_to_logical(&monitors, phys, 1.0);
        assert_eq!(scale, 1.5);
        assert_eq!(lx, 100.0);
        assert_eq!(ly, 100.0);
        assert_eq!(lw, 400.0);
        assert_eq!(lh, 300.0);
    }

    #[test]
    fn test_logical_to_physical_fallback_scale() {
        // No monitors → use fallback scale
        let monitors: Vec<MonitorRegion> = vec![];
        let (phys, scale) = logical_rect_to_physical(&monitors, 100.0, 100.0, 200.0, 200.0, 2.0);
        assert_eq!(scale, 2.0);
        assert_eq!(phys.x, 200.0);
        assert_eq!(phys.width, 400.0);
    }

    #[test]
    fn test_roundtrip_logical_physical() {
        let monitors = vec![mon(0.0, 0.0, 2880.0, 1620.0, 1.5)];
        let (phys, _) = logical_rect_to_physical(&monitors, 123.0, 456.0, 789.0, 321.0, 1.0);
        let (lx, ly, lw, lh, _) = physical_rect_to_logical(&monitors, phys, 1.0);
        assert!((lx - 123.0).abs() < 0.001);
        assert!((ly - 456.0).abs() < 0.001);
        assert!((lw - 789.0).abs() < 0.001);
        assert!((lh - 321.0).abs() < 0.001);
    }
}
