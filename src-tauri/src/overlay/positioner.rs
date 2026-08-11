use super::OverlayPosition;
use crate::selection::SelectionBounds;

/// Calculate overlay position based on available context.
/// Prefers target bounds (selection area), falls back to cursor position.
/// Result is clamped into the monitor work area under the cursor (`QTranslate` multi-monitor).
pub fn calculate_position(
    target_bounds: Option<&SelectionBounds>,
    cursor_x: f64,
    cursor_y: f64,
) -> OverlayPosition {
    let mut pos = if let Some(bounds) = target_bounds {
        OverlayPosition::below_bounds(bounds.x, bounds.y, bounds.width, bounds.height)
    } else {
        OverlayPosition::at_cursor(cursor_x, cursor_y)
    };
    clamp_to_cursor_monitor(&mut pos, cursor_x, cursor_y);
    pos
}

/// Place a card of size (w, h) near `bounds` so it does not occlude the target
/// word: below when there is room on the monitor work area, otherwise above.
/// Returns the clamped top-left corner.
pub fn place_near_bounds(
    bounds: &SelectionBounds,
    w: f64,
    h: f64,
    cursor_x: f64,
    cursor_y: f64,
) -> (f64, f64) {
    let (wx, wy, ww, wh) = monitor_work_area(cursor_x, cursor_y);
    let margin = 4.0;
    let below_y = bounds.y + bounds.height + 8.0;
    let above_y = bounds.y - h - 8.0;
    // Center the card under the word horizontally when the word is narrow.
    let pref_x = bounds.x + (bounds.width - w) / 2.0;
    let x = pref_x.clamp(wx + margin, (wx + ww - w - margin).max(wx + margin));
    let fits_below = below_y + h <= wy + wh - margin;
    let fits_above = above_y >= wy + margin;
    let y = if fits_below {
        below_y
    } else if fits_above {
        above_y
    } else {
        // Neither fits: keep it below, clamped to the work area.
        below_y.clamp(wy + margin, (wy + wh - h - margin).max(wy + margin))
    };
    (x, y)
}

/// Keep a floating rect inside the work area of the monitor that contains (cx, cy).
pub fn clamp_rect_to_cursor_monitor(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    cx: f64,
    cy: f64,
) -> (f64, f64) {
    let (wx, wy, ww, wh) = monitor_work_area(cx, cy);
    let margin = 4.0;
    let max_x = (wx + ww - w - margin).max(wx + margin);
    let max_y = (wy + wh - h - margin).max(wy + margin);
    (x.clamp(wx + margin, max_x), y.clamp(wy + margin, max_y))
}

fn clamp_to_cursor_monitor(pos: &mut OverlayPosition, cx: f64, cy: f64) {
    let (nx, ny) = clamp_rect_to_cursor_monitor(pos.x, pos.y, pos.width, pos.height, cx, cy);
    pos.x = nx;
    pos.y = ny;
}

/// Monitor work area (physical px) containing point; falls back to primary / large virtual desk.
fn monitor_work_area(cx: f64, cy: f64) -> (f64, f64, f64, f64) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{POINT, RECT};
        use windows::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        };
        // SAFETY: pt is a stack-allocated POINT; mi is a stack-allocated
        // MONITORINFO with cbSize set. MonitorFromPoint/GetMonitorInfoW are
        // pure Win32 queries with no preconditions beyond valid pointers.
        unsafe {
            let pt = POINT {
                x: cx as i32,
                y: cy as i32,
            };
            let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                rcMonitor: RECT::default(),
                rcWork: RECT::default(),
                dwFlags: 0,
            };
            if GetMonitorInfoW(mon, &raw mut mi).as_bool() {
                let r = mi.rcWork;
                return (
                    f64::from(r.left),
                    f64::from(r.top),
                    f64::from((r.right - r.left).max(1)),
                    f64::from((r.bottom - r.top).max(1)),
                );
            }
        }
    }
    (0.0, 0.0, 1920.0, 1080.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_keeps_inside_synthetic_bounds() {
        // Without real monitors, fallback is 0,0,1920,1080
        let (x, y) = clamp_rect_to_cursor_monitor(5000.0, 5000.0, 200.0, 100.0, 100.0, 100.0);
        assert!(x < 1920.0);
        assert!(y < 1080.0);
        assert!(x >= 0.0);
        assert!(y >= 0.0);
    }

    #[test]
    fn place_near_bounds_prefers_below() {
        // Word mid-screen (100,100,60,20), 300x200 card → placed below.
        let b = crate::selection::SelectionBounds {
            x: 100.0,
            y: 100.0,
            width: 60.0,
            height: 20.0,
        };
        let (x, y) = place_near_bounds(&b, 300.0, 200.0, 130.0, 110.0);
        assert!(y >= 100.0 + 20.0 + 8.0, "should be below the word, got y={y}");
        assert!(y + 200.0 <= 1080.0);
        // Card is wider than the word → centered position clamps to the work area.
        assert!((4.0..=1616.0).contains(&x), "x inside work area, got x={x}");
    }

    #[test]
    fn place_near_bounds_flips_above_near_bottom() {
        // Word near the bottom of the synthetic 1080-tall work area.
        let b = crate::selection::SelectionBounds {
            x: 500.0,
            y: 1020.0,
            width: 60.0,
            height: 20.0,
        };
        let (_, y) = place_near_bounds(&b, 300.0, 200.0, 530.0, 1030.0);
        assert!(
            y + 200.0 <= 1080.0,
            "card must stay on screen, got y={y}"
        );
        assert!(y < 1020.0, "should flip above the word, got y={y}");
    }
}
