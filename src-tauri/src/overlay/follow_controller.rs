use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell as TokioOnceCell, RwLock};
use tokio::time::{interval, Duration};

use super::OverlayState;

/// Follow mode for the overlay position refresh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FollowMode {
    /// Track the cursor position and move overlay accordingly.
    Cursor,
    /// Track a target region (e.g., selection bounds) and reposition when it changes.
    TargetBounds,
    /// No following — overlay stays where it was placed.
    None,
}

/// Active follow state managed by the controller.
struct FollowState {
    mode: FollowMode,
    running: bool,
}

/// Controls overlay position refreshing without destroying/recreating the window.
///
/// Lifecycle integration:
/// - `Transient`: no following (auto-dismiss handles it)
/// - `Interactive`: follows based on config (cursor or target bounds)
/// - `Pinned`: follows based on config, pin state preserved across refreshes
///
/// The `AppHandle` is set via `init()` during Tauri `setup()` since it is
/// not available when `AppState` is first constructed.
pub struct FollowController {
    app_handle: TokioOnceCell<tauri::AppHandle>,
    state: Arc<RwLock<FollowState>>,
    target_bounds: Arc<Mutex<Option<TargetBounds>>>,
    task_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

/// Target bounds that the overlay should track.
#[derive(Debug, Clone, Copy)]
pub struct TargetBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Cursor position snapshot.
#[derive(Debug, Clone, Copy)]
struct CursorPos {
    x: f64,
    y: f64,
}

/// Poll interval for position updates (milliseconds).
const FOLLOW_POLL_MS: u64 = 100;

/// Minimum pixel movement threshold to avoid jittery updates.
const MOVE_THRESHOLD: f64 = 3.0;

impl FollowController {
    /// Create an uninitialized controller. Call `init()` with the AppHandle
    /// once it becomes available (typically in Tauri `setup()`).
    pub fn new() -> Self {
        Self {
            app_handle: TokioOnceCell::new(),
            state: Arc::new(RwLock::new(FollowState {
                mode: FollowMode::None,
                running: false,
            })),
            target_bounds: Arc::new(Mutex::new(None)),
            task_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize with the Tauri AppHandle. Must be called once during `setup()`.
    /// Returns `Err` if already initialized.
    pub fn init(&self, app_handle: tauri::AppHandle) {
        let _ = self.app_handle.set(app_handle);
    }

    /// Start following with the given mode.
    /// If already running, stops the previous follow task first.
    ///
    /// - `Cursor`: spawns a background task that polls cursor position.
    /// - `TargetBounds`: positions once at the stored bounds, then stops.
    ///   Does NOT do continuous polling — call `update_target_bounds()` +
    ///   `refresh_once()` if the target moves.
    /// - `None`: no-op, does not set `running = true`.
    pub async fn start(&self, mode: FollowMode, overlay_state: OverlayState) {
        // Transient overlays don't follow — they auto-dismiss
        if overlay_state == OverlayState::Transient {
            return;
        }

        self.stop().await;

        // None mode: don't start anything, don't set running
        if mode == FollowMode::None {
            let mut state = self.state.write().await;
            state.mode = mode;
            state.running = false;
            return;
        }

        let app = match self.app_handle.get() {
            Some(h) => h.clone(),
            None => return, // Not initialized yet
        };

        // TargetBounds mode: position once at stored bounds, then stop.
        // No continuous polling since we don't have a mechanism to detect
        // target window movement. Call update_target_bounds() + refresh_once()
        // to reposition when the target moves.
        if mode == FollowMode::TargetBounds {
            {
                let mut state = self.state.write().await;
                state.mode = mode;
                state.running = false; // Not a continuous task
            }
            let bounds = self.target_bounds.lock().await;
            if let Some(b) = *bounds {
                super::window_manager::move_overlay_window(&app, b.x, b.y + b.height + 8.0);
            }
            return;
        }

        // Cursor mode: continuous polling
        {
            let mut state = self.state.write().await;
            state.mode = mode;
            state.running = true;
        }

        let state = Arc::clone(&self.state);

        let handle = tokio::spawn(async move {
            let mut tick = interval(Duration::from_millis(FOLLOW_POLL_MS));
            let mut last_pos: Option<(f64, f64)> = None;

            loop {
                tick.tick().await;

                // Check if still running
                {
                    let s = state.read().await;
                    if !s.running {
                        break;
                    }
                }

                let cursor = get_cursor_position();
                let nx = cursor.x + 10.0;
                let ny = cursor.y + 10.0;

                let should_move = match last_pos {
                    Some((lx, ly)) => {
                        (nx - lx).abs() > MOVE_THRESHOLD
                            || (ny - ly).abs() > MOVE_THRESHOLD
                    }
                    None => true,
                };

                if should_move {
                    // Preserve pin/click-through state across position updates
                    // The window is NOT recreated, just moved
                    super::window_manager::move_overlay_window(&app, nx, ny);
                    last_pos = Some((nx, ny));
                }
            }
        });

        *self.task_handle.lock().await = Some(handle);
    }

    /// Stop following. Does not close the overlay.
    pub async fn stop(&self) {
        {
            let mut state = self.state.write().await;
            state.running = false;
        }

        if let Some(handle) = self.task_handle.lock().await.take() {
            handle.abort();
        }
    }

    /// Change follow mode while running.
    pub async fn set_mode(&self, mode: FollowMode) {
        let mut state = self.state.write().await;
        state.mode = mode;
    }

    /// Update the target bounds to track.
    /// Call this when the selection/window being tracked moves.
    pub async fn update_target_bounds(&self, bounds: Option<TargetBounds>) {
        *self.target_bounds.lock().await = bounds;
    }

    /// Get current follow mode.
    pub async fn mode(&self) -> FollowMode {
        self.state.read().await.mode
    }

    /// Check if currently following.
    pub async fn is_running(&self) -> bool {
        self.state.read().await.running
    }

    /// Refresh overlay position once without starting continuous following.
    /// Useful for one-off repositioning (e.g., after target window moves).
    pub async fn refresh_once(&self) {
        let current_mode = self.state.read().await.mode;

        let new_pos = match current_mode {
            FollowMode::Cursor => {
                let cursor = get_cursor_position();
                Some((cursor.x + 10.0, cursor.y + 10.0))
            }
            FollowMode::TargetBounds => {
                let bounds = self.target_bounds.lock().await;
                bounds.map(|b| (b.x, b.y + b.height + 8.0))
            }
            FollowMode::None => None,
        };

        if let Some((x, y)) = new_pos {
            if let Some(app) = self.app_handle.get() {
                // Preserve pin/click-through state — just move, don't recreate
                super::window_manager::move_overlay_window(app, x, y);
            }
        }
    }
}

/// Get the current cursor position. Falls back to (100, 100) if unavailable.
fn get_cursor_position() -> CursorPos {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct POINT {
            x: i32,
            y: i32,
        }
        extern "system" {
            fn GetCursorPos(lpPoint: *mut POINT) -> i32;
        }
        unsafe {
            let mut point = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut point) != 0 {
                return CursorPos {
                    x: point.x as f64,
                    y: point.y as f64,
                };
            }
        }
    }
    CursorPos {
        x: 100.0,
        y: 100.0,
    }
}
