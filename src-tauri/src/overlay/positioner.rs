use super::OverlayPosition;
use crate::selection::SelectionBounds;

/// Calculate overlay position based on available context.
/// Prefers target bounds (selection area), falls back to cursor position.
pub fn calculate_position(
    target_bounds: Option<&SelectionBounds>,
    cursor_x: f64,
    cursor_y: f64,
) -> OverlayPosition {
    if let Some(bounds) = target_bounds {
        // Place overlay below the selection with a small gap
        OverlayPosition::below_bounds(bounds.x, bounds.y, bounds.width, bounds.height)
    } else {
        // Place near cursor
        OverlayPosition::at_cursor(cursor_x, cursor_y)
    }
}

/// Clamp overlay position to stay within screen bounds
pub fn clamp_to_screen(pos: OverlayPosition, screen_w: f64, screen_h: f64) -> OverlayPosition {
    OverlayPosition {
        x: pos.x.max(0.0).min(screen_w - pos.width - 20.0),
        y: pos.y.max(0.0).min(screen_h - pos.height - 20.0),
        width: pos.width,
        height: pos.height,
    }
}
