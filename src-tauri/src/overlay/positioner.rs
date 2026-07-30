use super::OverlayPosition;
use crate::selection::SelectionBounds;

/// Calculate overlay position based on available context.
/// Prefers target bounds (selection area), falls back to cursor position.
/// Result is clamped into the monitor work area under the cursor (QTranslate multi-monitor).
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
            if GetMonitorInfoW(mon, &mut mi).as_bool() {
                let r = mi.rcWork;
                return (
                    r.left as f64,
                    r.top as f64,
                    (r.right - r.left).max(1) as f64,
                    (r.bottom - r.top).max(1) as f64,
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
}
