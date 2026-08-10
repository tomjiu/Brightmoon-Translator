//! Floating React `translate-card` window — independent of the legacy data-URL
//! overlay. Built once (hidden, preloaded at startup) as `translate-card`; each
//! show reuses the window, positions it near the selection/cursor, emits
//! structured `translate-card-data` to the FE, then shows it.
//!
//! Focus semantics (pot / `STranslate` / Youdao):
//! - Hover & dictionary cards → no-focus (`WS_EX_NOACTIVATE`, shown via
//!   `SW_SHOWNOACTIVATE`); `auto_watch` dismisses them on mouse-leave.
//! - User-initiated selection translate → takes focus; the FE closes the card
//!   on window blur (pot `OnDeactivated`).
//!
//! The FE self-sizes the card: it measures content with a `ResizeObserver` and
//! calls `set_size` (same pattern as `OcrRegionFrame`). The initial size passed
//! here is only a placement estimate.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::models::translation::TranslateResponse;
use crate::selection::present::DictCard;

pub const TRANSLATE_CARD_LABEL: &str = "translate-card";
pub const TRANSLATE_CARD_EVENT: &str = "translate-card-data";
const TRANSLATE_CARD_URL: &str = "index.html?window=translate-card&v=1";
/// FE emits this once its data listener is registered; backend uses it to avoid
/// emitting into a not-yet-mounted webview on the cold-start path.
pub const TRANSLATE_CARD_READY_EVENT: &str = "translate-card-ready";
/// FE emits this with the payload `nonce` once it has applied the payload AND
/// self-sized the window — the backend waits for it before showing, so a show
/// never paints stale content or a mid-resize window (flash fix).
pub const TRANSLATE_CARD_RENDERED_EVENT: &str = "translate-card-rendered";
/// FE emits this when the user closes the card manually (close button / Esc).
/// The backend suppresses hover re-presentation briefly so the card doesn't
/// instantly reappear under the still-hovering cursor.
pub const TRANSLATE_CARD_CLOSED_EVENT: &str = "translate-card-closed";
/// FE → backend: user clicked "translate remaining engines" on a quick card.
/// Payload: `{ source, from, to }`. Backend replies via `TRANSLATE_CARD_EXPAND_RESULT`.
pub const TRANSLATE_CARD_EXPAND_REQUEST: &str = "translate-card-expand-request";
/// Backend → FE: full (all-engine) result for the expand request. Payload:
/// `{ source, from, to, response }` — the FE applies it only if `source` still
/// matches the current card.
pub const TRANSLATE_CARD_EXPAND_RESULT: &str = "translate-card-expand-result";

/// Structured card payload emitted to the FE. `kind` tag discriminates the two
/// card types (serde tagged newtype variants flatten their inner fields).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum TranslateCardData {
    /// Multi-engine machine translation card (source + structured results).
    Mt(MtCardData),
    /// Youdao-style dictionary card.
    Dict(DictCardData),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MtCardData {
    pub source: String,
    /// Source language code (e.g. "en") — used by the FE for source TTS.
    pub from: String,
    /// Target language code (e.g. "zh") — used by the FE for target TTS.
    pub to: String,
    pub response: TranslateResponse,
    /// Total configured engines — the FE shows an "expand" affordance while
    /// `response.results.len() < total_engines` (quick card = first result).
    pub total_engines: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictCardData {
    pub card: DictCard,
}

/// Event wrapper — `nonce` lets the FE dedupe against a re-emit (cold start);
/// `focus` tells the FE whether this card was shown with keyboard focus
/// (user-initiated → blur auto-closes) or as a hover/dict card (never closes
/// on blur; `auto_watch` mouse-leave handles dismissal).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateCardEvent {
    pub nonce: u64,
    pub focus: bool,
    #[serde(flatten)]
    pub data: TranslateCardData,
}

/// Display options for a show request.
#[derive(Debug, Clone)]
pub struct TranslateCardOptions {
    /// `true` → the window takes keyboard focus (user-initiated 划词; the FE
    /// auto-closes on blur). `false` → hover/dictionary: never steals focus
    /// (`WS_EX_NOACTIVATE` + `SW_SHOWNOACTIVATE`).
    pub steal_focus: bool,
    /// Screen region (e.g. the hovered word) that keeps a no-focus card alive:
    /// mouse-leave dismissal is skipped while the cursor is over it. Parking on
    /// the hovered word must not dismiss the card sitting just below it.
    pub keep_alive: Option<crate::selection::SelectionBounds>,
}

/// True if the last shown translate card was hover/dict (no-focus) — only these
/// are eligible for mouse-leave auto-dismiss. User-initiated cards close on blur.
static LAST_NO_FOCUS: AtomicBool = AtomicBool::new(true);
/// FE listener ready (set by `translate-card-ready`, bounded wait in show).
static CARD_READY: AtomicBool = AtomicBool::new(false);
static CARD_NONCE: AtomicU64 = AtomicU64::new(0);
/// Highest nonce the FE has confirmed rendered + self-sized (`translate-card-rendered`).
static CARD_RENDERED_NONCE: AtomicU64 = AtomicU64::new(0);
/// When the last card was shown (for the no-flicker dismiss guard).
static LAST_SHOW: Mutex<Option<Instant>> = Mutex::new(None);
/// Instant of the last manual close (`translate-card-closed`) — hover is
/// suppressed briefly after it so the card doesn't instantly reappear.
static CARD_CLOSED_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// Cursor position at the last manual close — hover stays suppressed while the
/// cursor remains near it (the card the user just dismissed is "latched" shut
/// until they move away). This is what makes the close button feel like it
/// works: clicking X on a hover card must not have the card pop back a moment
/// later under the still-positioned cursor.
static CARD_CLOSED_AT_POS: Mutex<Option<(f64, f64)>> = Mutex::new(None);
/// Keep-alive region of the current no-focus card (cleared on hide).
static CARD_KEEP_ALIVE: Mutex<Option<(f64, f64, f64, f64)>> = Mutex::new(None);

/// Mark the FE listener as mounted (from the `translate-card-ready` handler).
pub fn mark_card_ready() {
    CARD_READY.store(true, Ordering::SeqCst);
}

/// Record that the FE finished applying payload `nonce` (and self-sized).
pub fn mark_card_rendered(nonce: u64) {
    CARD_RENDERED_NONCE.store(nonce, Ordering::SeqCst);
}

/// Record a manual close (from the `translate-card-closed` handler).
pub fn mark_card_closed() {
    *CARD_CLOSED_AT.lock().unwrap() = Some(Instant::now());
    // Latch: remember where the cursor was when the user closed the card so
    // hover stays suppressed around that spot until the cursor moves away.
    let (cx, cy) = crate::win::cursor_pos();
    *CARD_CLOSED_AT_POS.lock().unwrap() = Some((cx, cy));
    tracing::info!("[translate-card] closed by user (latch at {}, {})", cx, cy);
}

/// True while the cursor is within `radius` px of where the user last closed
/// the card (via the close button / Esc). Hover re-presentation must not fire
/// there — the user just dismissed the card and it must stay dismissed until
/// they move the cursor elsewhere. Releasing: when the cursor leaves the
/// radius, the latch clears itself so hover works normally again.
pub fn translate_card_close_latch_hit(cx: f64, cy: f64) -> bool {
    const RADIUS_PX: f64 = 80.0;
    let mut slot = CARD_CLOSED_AT_POS.lock().unwrap();
    match *slot {
        Some((px, py)) => {
            let dx = cx - px;
            let dy = cy - py;
            if dx * dx + dy * dy <= RADIUS_PX * RADIUS_PX {
                true
            } else {
                *slot = None;
                false
            }
        },
        None => false,
    }
}

/// True if the translate-card window is currently visible.
pub fn translate_card_is_visible(app: &AppHandle) -> bool {
    app.get_webview_window(TRANSLATE_CARD_LABEL)
        .is_some_and(|w| w.is_visible().unwrap_or(false))
}

/// True if the card was shown within `ms` — the no-flicker dismiss guard.
pub fn translate_card_shown_within_ms(ms: u64) -> bool {
    LAST_SHOW
        .lock()
        .unwrap()
        .map(|t| t.elapsed().as_millis() < u128::from(ms))
        .unwrap_or(false)
}

/// True if the card was manually closed within `ms` — hover re-presentation is
/// suppressed briefly so it doesn't instantly reappear under the cursor.
pub fn translate_card_closed_within_ms(ms: u64) -> bool {
    CARD_CLOSED_AT
        .lock()
        .unwrap()
        .map(|t| t.elapsed().as_millis() < u128::from(ms))
        .unwrap_or(false)
}

/// True if `(cx, cy)` is over the current card's keep-alive region (the hovered
/// word), inflated like the card-bounds hit test.
pub fn translate_card_keep_alive_hit(cx: f64, cy: f64) -> bool {
    CARD_KEEP_ALIVE
        .lock()
        .unwrap()
        .map(|(x, y, w, h)| {
            cx >= x - 12.0 && cx <= x + w + 12.0 && cy >= y - 12.0 && cy <= y + h + 12.0
        })
        .unwrap_or(false)
}

fn card_is_ready() -> bool {
    CARD_READY.load(Ordering::SeqCst)
}

/// True if the last shown card is a hover/dict (no-focus) card.
pub fn translate_card_no_focus_mode() -> bool {
    LAST_NO_FOCUS.load(Ordering::SeqCst)
}

/// Create the window hidden off-screen if it doesn't exist yet. Safe to call
/// multiple times — no-op when already built.
pub fn ensure_translate_card_window(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(TRANSLATE_CARD_LABEL).is_some() {
        return Ok(());
    }
    let window = WebviewWindowBuilder::new(
        app,
        TRANSLATE_CARD_LABEL,
        WebviewUrl::App(TRANSLATE_CARD_URL.into()),
    )
    .title("Translation")
    .inner_size(320.0, 160.0)
    // Off-screen so the hidden window never paints over the desktop if DWM
    // races the `visible(false)` setting during creation.
    .position(-32000.0, -32000.0)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(true)
    .focused(false)
    .fullscreen(false)
    .transparent(false)
    .background_color(tauri::window::Color(12, 12, 14, 255))
    .visible(false)
    .build()
    .map_err(|e| format!("Failed to create translate card: {e}"))?;

    // Default no-activate so hover/dict shows never steal focus; focus mode
    // clears the style at show time.
    #[cfg(target_os = "windows")]
    if let Ok(h) = window.hwnd() {
        crate::win::set_window_no_activate(h.0 as isize, true);
    }
    // P0: force-hide after build (same DWM race that affects `visible(false)`
    // on the OCR region frame).
    let _ = window.hide();
    tracing::info!("[translate-card] window created (hidden, off-screen)");
    Ok(())
}

/// Preload the translate-card webview at startup so the first show skips the
/// `WebView2` create cost and the FE listener is already mounted.
pub fn preload_translate_card(app: &tauri::AppHandle) -> Result<(), String> {
    ensure_translate_card_window(app)
}

/// Wait (bounded) for the FE webview to have mounted and registered its data
/// listener. No-op when already ready or when no window exists yet.
async fn wait_card_ready(app: &AppHandle) {
    if card_is_ready() {
        return;
    }
    for _ in 0..40 {
        // 4s budget — preload happens ~1s after startup; a manual trigger in
        // that window would otherwise emit into a blank webview.
        if card_is_ready() || app.get_webview_window(TRANSLATE_CARD_LABEL).is_none() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Position, emit structured data, and show the translate card.
/// `x, y` are the preferred top-left, `w, h` the initial size (physical px) —
/// the FE re-measures content and self-sizes via `set_size`.
pub async fn show_translate_card(
    app: &AppHandle,
    data: &TranslateCardData,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    opts: TranslateCardOptions,
) -> Result<(), String> {
    ensure_translate_card_window(app)?;
    wait_card_ready(app).await;

    let window = app
        .get_webview_window(TRANSLATE_CARD_LABEL)
        .ok_or_else(|| "translate-card window missing".to_string())?;

    let w = w.clamp(120.0, 620.0);
    let h = h.clamp(48.0, 720.0);
    let (cx, cy) = crate::win::cursor_pos();
    let (px, py) = crate::overlay::positioner::clamp_rect_to_cursor_monitor(x, y, w, h, cx, cy);
    let px = px.max(0.0) as i32;
    let py = py.max(0.0) as i32;

    // Reuse: hide → position → size → emit (while hidden, so the FE renders the
    // new content before we show — no stale flash at the new position) → wait
    // for the FE render-ack → show. Waiting for the ack also means the FE has
    // already self-sized via `set_size`, so there's no visible resize either.
    let _ = window.hide();
    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        px, py,
    )));
    let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(w as u32, h as u32)));
    let _ = window.set_always_on_top(true);

    let nonce = CARD_NONCE.fetch_add(1, Ordering::Relaxed) + 1;
    let event = TranslateCardEvent {
        nonce,
        focus: opts.steal_focus,
        data: data.clone(),
    };
    let _ = window.emit(TRANSLATE_CARD_EVENT, &event);

    // Bounded render-ack wait: the FE emits `translate-card-rendered` with the
    // nonce once it has applied the payload and self-sized. Skip (show anyway)
    // if the webview never acks so we never block a card permanently.
    for _ in 0..12 {
        if CARD_RENDERED_NONCE.load(Ordering::SeqCst) >= nonce {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    *LAST_SHOW.lock().unwrap() = Some(Instant::now());
    *CARD_KEEP_ALIVE.lock().unwrap() = opts
        .keep_alive
        .as_ref()
        .map(|b| (b.x, b.y, b.width, b.height));
    // A fresh card invalidates any close-latch (hover is suppressed by the
    // visible card itself while it's up).
    *CARD_CLOSED_AT_POS.lock().unwrap() = None;

    if opts.steal_focus {
        // User-initiated: clear WS_EX_NOACTIVATE, show, then take focus. The FE
        // closes itself on blur (pot OnDeactivated behavior).
        #[cfg(target_os = "windows")]
        if let Ok(hw) = window.hwnd() {
            crate::win::set_window_no_activate(hw.0 as isize, false);
        }
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        // Hover/dict: never steal focus; auto_watch dismisses on mouse-leave.
        // Apply WS_EX_NOACTIVATE *before* showing (a previous focus card may
        // have cleared it), then show via the Tauri `show()` path — NOT a raw
        // ShowWindow. Raw shows leave Tauri's cached visibility state stale
        // (window created hidden), so a later `hide()` (close button / Esc /
        // blur / mouse-leave) silently no-ops and the card can never be closed.
        #[cfg(target_os = "windows")]
        if let Ok(hw) = window.hwnd() {
            crate::win::set_window_no_activate(hw.0 as isize, true);
        }
        let _ = window.show();
    }
    LAST_NO_FOCUS.store(!opts.steal_focus, Ordering::SeqCst);

    tracing::info!(
        "[translate-card] shown kind={} {}x{} @ ({},{}) steal_focus={}",
        match data {
            TranslateCardData::Mt(_) => "mt",
            TranslateCardData::Dict(_) => "dict",
        },
        w as u32,
        h as u32,
        px,
        py,
        opts.steal_focus
    );
    Ok(())
}

/// Hide the translate card (pooled — never destroy, keeps FE listener alive).
pub fn hide_translate_card(app: &AppHandle) {
    *CARD_KEEP_ALIVE.lock().unwrap() = None;
    if let Some(window) = app.get_webview_window(TRANSLATE_CARD_LABEL) {
        let _ = window.hide();
    }
}

/// Screen bounds of the translate card if visible (for mouse-leave dismiss).
pub fn translate_card_screen_bounds(app: &AppHandle) -> Option<(f64, f64, f64, f64)> {
    let w = app.get_webview_window(TRANSLATE_CARD_LABEL)?;
    if !w.is_visible().ok()? {
        return None;
    }
    let pos = w.outer_position().ok()?;
    let size = w.outer_size().ok()?;
    Some((
        f64::from(pos.x),
        f64::from(pos.y),
        f64::from(size.width),
        f64::from(size.height),
    ))
}
