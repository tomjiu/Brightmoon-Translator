//! M3 multi-region live OCR session manager — backend skeleton.
//!
//! Mirrors `overlay/pin_manager.rs` `PinWindowManager` singleton pattern
//! (`OnceLock<Mutex<..>>`). Scope of M3.1: the manager + 5 commands with a
//! `"default"` delegate shim so the frozen M0/M1/M2 single-frame baton is
//! **called, never rewritten**. UI wiring and per-region window creation land
//! in M3.2+.
//!
//! Open-decision defaults used here (adjust after user review of
//! `docs/M3_DECISION_REQUEST.md`):
//! - `MAX_REGIONS` = 8 (Q1 recommended)
//! - new region default mode = `translated` (Q2 recommended)
//! - continuous per-region, default off (I1)
//! - no cross-region translation cache (Q4 deferred to M4)

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use tauri::{Emitter, Manager};

/// Max live regions (Q1 recommended default; change after user decision).
pub const MAX_REGIONS: usize = 8;

/// Reserved legacy single-frame region id (bare label, zero-change shim).
pub const DEFAULT_REGION_ID: &str = "default";

/// Screen rect in physical pixels (matches `crop_screenshot_snapshot` and
/// `create_ocr_region_frame` argument semantics: x/y/width/height).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OcrRegionRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl OcrRegionRect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Region display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegionMode {
    Image,
    Source,
    Translated,
}

impl RegionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Source => "source",
            Self::Translated => "translated",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "image" => Some(Self::Image),
            "source" => Some(Self::Source),
            "translated" => Some(Self::Translated),
            _ => None,
        }
    }
}

impl Default for RegionMode {
    /// Q2 recommended default: translated (matches the single-frame experience).
    fn default() -> Self {
        Self::Translated
    }
}

/// Per-region live session state (design §2.2 interface contract).
#[derive(Debug, Clone)]
pub struct RegionSession {
    pub region_id: String,
    pub rect: OcrRegionRect,
    pub mode: RegionMode,
    /// Per-region continuous watch, default off (I1).
    pub continuous: bool,
    /// Per-region follow-HWND bind (I6).
    pub follow_hwnd: Option<isize>,
    /// Image fingerprint for the I7 skip gate.
    pub last_image_fp: Option<String>,
    /// Last OCR text for the I7 similarity gate.
    pub last_text: Option<String>,
    /// Whether this region is currently in sampling-excluded state.
    pub sampling: bool,
    /// M4: per-region OCR engine override ('' = use global default).
    pub engine: String,
    pub created_at: Instant,
    pub created_at_ms: u64,
}

impl RegionSession {
    pub fn new(region_id: String, rect: OcrRegionRect) -> Self {
        let created_at = Instant::now();
        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self {
            region_id,
            rect,
            mode: RegionMode::default(),
            continuous: false,
            follow_hwnd: None,
            last_image_fp: None,
            last_text: None,
            sampling: false,
            engine: String::new(),
            created_at,
            created_at_ms,
        }
    }
}

/// Serializable projection for `ocr_region_list` (design §8 contract, camelCase).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionSessionInfo {
    pub region_id: String,
    pub label: String,
    pub rect: OcrRegionRect,
    pub mode: String,
    pub continuous: bool,
    pub follow_hwnd: Option<isize>,
    pub sampling: bool,
    pub engine: String,
    pub created_at_ms: u64,
}

/// Window label for a region id. `"default"` → bare `"ocr-region-frame"`
/// (M0/M1/M2 zero-change shim); otherwise `"ocr-region-frame-{id}"`.
pub fn region_label(region_id: &str) -> String {
    if region_id == DEFAULT_REGION_ID {
        "ocr-region-frame".to_string()
    } else {
        format!("ocr-region-frame-{region_id}")
    }
}

/// Monotonic region-id counter state. Kept in the manager.
#[derive(Debug, Default)]
pub struct RegionSessionManager {
    sessions: HashMap<String, RegionSession>,
    next_id: u64,
}

impl RegionSessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate the next monotonic region id, skipping `"default"` and any
    /// id already in use.
    fn next_region_id(&mut self) -> String {
        loop {
            self.next_id += 1;
            let candidate = self.next_id.to_string();
            if candidate != DEFAULT_REGION_ID && !self.sessions.contains_key(&candidate) {
                return candidate;
            }
        }
    }

    /// Create a new live region session. Returns the new region id.
    /// Refuses once `MAX_REGIONS` live sessions exist (including default).
    pub fn create(&mut self, rect: OcrRegionRect) -> Result<String, String> {
        if self.sessions.len() >= MAX_REGIONS {
            return Err(format!(
                "Region limit reached ({MAX_REGIONS}). Close an existing region first."
            ));
        }
        let id = self.next_region_id();
        self.sessions
            .insert(id.clone(), RegionSession::new(id.clone(), rect));
        Ok(id)
    }

    /// Register (or refresh) the reserved default session (legacy shim).
    /// The default slot never counts against `MAX_REGIONS`.
    pub fn register_default(&mut self, rect: OcrRegionRect) -> Result<(), String> {
        self.sessions.insert(
            DEFAULT_REGION_ID.to_string(),
            RegionSession::new(DEFAULT_REGION_ID.to_string(), rect),
        );
        Ok(())
    }

    /// Remove a session by id. Returns `true` if it existed.
    pub fn remove(&mut self, region_id: &str) -> bool {
        self.sessions.remove(region_id).is_some()
    }

    pub fn get(&self, region_id: &str) -> Option<&RegionSession> {
        self.sessions.get(region_id)
    }

    pub fn get_mut(&mut self, region_id: &str) -> Option<&mut RegionSession> {
        self.sessions.get_mut(region_id)
    }

    pub fn contains(&self, region_id: &str) -> bool {
        self.sessions.contains_key(region_id)
    }

    pub fn active_count(&self) -> usize {
        self.sessions.len()
    }

    /// True if any live region exists (including default).
    pub fn has_live_regions(&self) -> bool {
        !self.sessions.is_empty()
    }

    /// Number of non-default live regions.
    pub fn non_default_count(&self) -> usize {
        self.sessions
            .keys()
            .filter(|k| *k != DEFAULT_REGION_ID)
            .count()
    }

    /// Serializable projection, default first, then numeric ids ascending.
    pub fn list_info(&self) -> Vec<RegionSessionInfo> {
        let mut infos: Vec<RegionSessionInfo> = self
            .sessions
            .values()
            .map(|s| RegionSessionInfo {
                region_id: s.region_id.clone(),
                label: region_label(&s.region_id),
                rect: s.rect,
                mode: s.mode.as_str().to_string(),
                continuous: s.continuous,
                follow_hwnd: s.follow_hwnd,
                sampling: s.sampling,
                engine: s.engine.clone(),
                created_at_ms: s.created_at_ms,
            })
            .collect();
        infos.sort_by_key(|i| {
            if i.region_id == DEFAULT_REGION_ID {
                (0u8, String::new())
            } else {
                (1u8, i.region_id.clone())
            }
        });
        infos
    }
}

// ── Global singleton (mirrors overlay/pin_manager.rs) ────────────────

static REGION_MANAGER: OnceLock<Mutex<RegionSessionManager>> = OnceLock::new();

fn region_manager() -> &'static Mutex<RegionSessionManager> {
    REGION_MANAGER.get_or_init(|| Mutex::new(RegionSessionManager::new()))
}

/// M3: Begin an OCR session for a region.
///
/// - `id == "default"` (or omitted): delegate to the existing single-frame
///   baton (`ocr_begin_session_hide_main`) + register the default session.
/// - other id: register a new live session. Main is hidden only when this is
///   the **first** live region. (Per-region window creation lands in M3.2.)
#[tauri::command]
pub async fn ocr_begin_session(
    app: tauri::AppHandle,
    id: String,
    rect: Option<OcrRegionRect>,
    snapshot: Option<String>,
) -> Result<(), String> {
    let rect = rect.unwrap_or_else(|| OcrRegionRect::new(0.0, 0.0, 0.0, 0.0));
    if id == DEFAULT_REGION_ID {
        crate::commands::window::ocr_begin_session_hide_main(app.clone()).await?;
        let mut mgr = region_manager()
            .lock()
            .map_err(|e| format!("RegionManager lock: {e}"))?;
        mgr.register_default(rect)?;
        return Ok(());
    }

    let was_empty;
    let new_id = {
        let mut mgr = region_manager()
            .lock()
            .map_err(|e| format!("RegionManager lock: {e}"))?;
        was_empty = mgr.active_count() == 0;
        let new_id = mgr.create(rect)?;
        new_id
    };
    if was_empty {
        // First live region hides main (matches single-frame session semantics).
        crate::commands::window::ocr_begin_session_hide_main(app).await?;
    }
    tracing::info!(
        region_id = %new_id,
        "M3: OCR region session started"
    );
    // `snapshot` is reserved for M3.3+ (per-region screenshot pipeline).
    let _ = snapshot;
    Ok(())
}

/// M3: End an OCR session for a region.
///
/// - `id == "default"`: close the default frame window, remove the default
///   session; main is restored only when no live region remains.
/// - other id: remove that session; close its labeled window; main is restored
///   only when this was the **last** live region (fixes single-frame B5, where
///   closing any one frame killed every other live session).
#[tauri::command]
pub async fn ocr_end_session(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let removed = {
        let mut mgr = region_manager()
            .lock()
            .map_err(|e| format!("RegionManager lock: {e}"))?;
        mgr.remove(&id)
    };
    if !removed {
        return Err(format!("OCR region session not found: {id}"));
    }

    if id == DEFAULT_REGION_ID {
        crate::commands::window::close_ocr_region_frame(app.clone()).await?;
    } else if let Some(window) = app.get_webview_window(&region_label(&id)) {
        let _ = window.close();
        // Short yield so the label releases without a long blank (matches
        // close_ocr_region_frame behavior).
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }

    let any_live = region_manager()
        .lock()
        .map(|mgr| mgr.has_live_regions())
        .unwrap_or(false);
    if !any_live {
        crate::commands::window::ocr_end_session_show_main(app).await?;
    }
    Ok(())
}

/// M3: Set a region's display mode.
///
/// Stores per-region mode. Event emission (`ocr-region-mode[-{id}]`) lands in
/// M3.2: Set a region's display mode and push the mode event to its frame.
/// - default → emit `ocr-region-mode` on the bare `ocr-region-frame` label.
/// - other id → emit `ocr-region-mode-{id}` on `ocr-region-frame-{id}`.
#[tauri::command]
pub async fn ocr_region_set_mode(
    app: tauri::AppHandle,
    id: String,
    mode: String,
) -> Result<(), String> {
    let parsed = RegionMode::parse(&mode)
        .ok_or_else(|| format!("Invalid region mode: {mode} (expected image|source|translated)"))?;
    {
        let mut mgr = region_manager()
            .lock()
            .map_err(|e| format!("RegionManager lock: {e}"))?;
        let Some(session) = mgr.get_mut(&id) else {
            return Err(format!("OCR region session not found: {id}"));
        };
        session.mode = parsed;
    }
    let event = if id == DEFAULT_REGION_ID {
        "ocr-region-mode".to_string()
    } else {
        format!("ocr-region-mode-{id}")
    };
    emit_to_region(
        &app,
        &id,
        &event,
        &serde_json::json!({ "regionId": id, "mode": parsed.as_str() }),
    );
    Ok(())
}

/// M3: Emit an event + payload to a specific region frame window by region id.
/// Resolves the window label via `region_label(id)`.
pub fn emit_to_region(
    app: &tauri::AppHandle,
    region_id: &str,
    event: &str,
    payload: &serde_json::Value,
) {
    let label = region_label(region_id);
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.emit(event, payload);
        tracing::debug!(region_id, event, "M3: emitted to region window");
    } else {
        tracing::debug!(region_id, event, "M3: region window not present, event dropped");
    }
}

/// M3.4: Set `WDA_EXCLUDEFROMCAPTURE` on **all** active region frame windows
/// (I1 multi-frame: any region sampling must not eat sibling frames into the
/// screenshot). Mirrors `set_ocr_region_frame_sampling`'s affinity path per
/// window; returns `true` if every region window used the affinity path (short
/// settle OK), `false` if any fell back to hide/show (caller waits longer).
/// When `exclude=false`, only re-shows windows that were visible before.
pub fn set_all_regions_exclude_from_capture(
    app: &tauri::AppHandle,
    exclude: bool,
) -> bool {
    let ids: Vec<String> = region_manager()
        .lock()
        .map(|mgr| mgr.list_info().into_iter().map(|i| i.region_id).collect())
        .unwrap_or_default();

    if ids.is_empty() {
        return true;
    }

    let mut all_affinity = true;
    for id in ids {
        let label = region_label(&id);
        let Some(window) = app.get_webview_window(&label) else {
            continue;
        };
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::WindowsAndMessaging::{
                SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
            };

            let was_visible = window.is_visible().unwrap_or(true);
            let hwnd = match window.hwnd() {
                Ok(h) => HWND(h.0 as *mut _),
                Err(_) => {
                    if exclude {
                        let _ = window.hide();
                    } else if was_visible {
                        let _ = window.show();
                    }
                    all_affinity = false;
                    continue;
                },
            };
            let affinity = if exclude {
                WDA_EXCLUDEFROMCAPTURE
            } else {
                WDA_NONE
            };
            // SAFETY: live Tauri window HWND.
            let ok = unsafe { SetWindowDisplayAffinity(hwnd, affinity) };
            if ok.is_err() {
                tracing::warn!(
                    "set_all_regions_exclude_from_capture({id}, exclude={exclude}) affinity failed: {:?}",
                    ok
                );
                if exclude {
                    let _ = window.hide();
                } else if was_visible {
                    let _ = window.show();
                }
                all_affinity = false;
            } else if !exclude && was_visible {
                let _ = window.show();
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            if exclude {
                let _ = window.hide();
            }
            all_affinity = false;
        }
    }
    all_affinity
}

/// M3: List all active region sessions (including default).
#[tauri::command]
pub fn ocr_region_list() -> Result<Vec<RegionSessionInfo>, String> {
    let mgr = region_manager()
        .lock()
        .map_err(|e| format!("RegionManager lock: {e}"))?;
    Ok(mgr.list_info())
}

/// M4: Set per-region OCR engine override ('' resets to global default).
/// Mirrors the frontend `RegionState.engine` so `ocr_region_list` exposes the
/// per-region engine choice. No-op for unknown regions (frame not yet created).
#[tauri::command]
pub fn ocr_region_set_engine(id: String, engine: String) -> Result<(), String> {
    let mut mgr = region_manager()
        .lock()
        .map_err(|e| format!("RegionManager lock: {e}"))?;
    if let Some(session) = mgr.get_mut(&id) {
        session.engine = engine;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> OcrRegionRect {
        OcrRegionRect::new(x, y, w, h)
    }

    #[test]
    fn region_label_default_is_bare() {
        assert_eq!(region_label(DEFAULT_REGION_ID), "ocr-region-frame");
        assert_eq!(region_label("3"), "ocr-region-frame-3");
        assert_eq!(region_label("default").len(), "ocr-region-frame".len());
    }

    #[test]
    fn create_assigns_monotonic_ids_and_skips_default() {
        let mut mgr = RegionSessionManager::new();
        let id1 = mgr.create(rect(0., 0., 100., 100.)).unwrap();
        assert_eq!(id1, "1");
        let id2 = mgr.create(rect(10., 10., 200., 200.)).unwrap();
        assert_eq!(id2, "2");
        assert_eq!(mgr.active_count(), 2);
        assert_eq!(mgr.non_default_count(), 2);
    }

    #[test]
    fn default_register_and_remove() {
        let mut mgr = RegionSessionManager::new();
        mgr.register_default(rect(0., 0., 320., 200.)).unwrap();
        assert!(mgr.contains(DEFAULT_REGION_ID));
        assert_eq!(mgr.active_count(), 1);
        assert_eq!(mgr.non_default_count(), 0);
        assert!(mgr.has_live_regions());
        assert!(mgr.remove(DEFAULT_REGION_ID));
        assert!(!mgr.has_live_regions());
        assert!(!mgr.remove(DEFAULT_REGION_ID));
    }

    #[test]
    fn default_slot_does_not_count_against_limit() {
        let mut mgr = RegionSessionManager::new();
        // Fill to MAX_REGIONS with regular regions.
        for _ in 0..MAX_REGIONS {
            mgr.create(rect(0., 0., 100., 100.)).unwrap();
        }
        assert_eq!(mgr.active_count(), MAX_REGIONS);
        // default still registers on top.
        mgr.register_default(rect(0., 0., 100., 100.)).unwrap();
        assert_eq!(mgr.active_count(), MAX_REGIONS + 1);
    }

    #[test]
    fn max_regions_rejects_overflow() {
        let mut mgr = RegionSessionManager::new();
        for _ in 0..MAX_REGIONS {
            mgr.create(rect(0., 0., 100., 100.)).unwrap();
        }
        let err = mgr.create(rect(0., 0., 100., 100.)).unwrap_err();
        assert!(err.contains("Region limit reached"), "got: {err}");
        assert_eq!(mgr.active_count(), MAX_REGIONS);
    }

    #[test]
    fn remove_returns_existence() {
        let mut mgr = RegionSessionManager::new();
        let id = mgr.create(rect(0., 0., 100., 100.)).unwrap();
        assert!(mgr.remove(&id));
        assert!(!mgr.contains(&id));
        assert!(!mgr.remove(&id));
    }

    /// M3.5: closing one region must NOT affect the others (design B5 fix —
    /// the old single-frame path killed every live session when any frame closed).
    #[test]
    fn closing_one_keeps_others_alive() {
        let mut mgr = RegionSessionManager::new();
        let id1 = mgr.create(rect(0., 0., 100., 100.)).unwrap();
        let id2 = mgr.create(rect(10., 10., 200., 200.)).unwrap();
        let id3 = mgr.create(rect(20., 20., 300., 300.)).unwrap();
        assert_eq!(mgr.active_count(), 3);

        assert!(mgr.remove(&id2));
        assert_eq!(mgr.active_count(), 2);
        assert!(!mgr.contains(&id2));
        assert!(mgr.contains(&id1));
        assert!(mgr.contains(&id3));
        // Their independent state (continuous etc.) is untouched.
        mgr.get_mut(&id3).unwrap().continuous = true;
        assert!(mgr.get(&id3).unwrap().continuous);
    }

    /// M3.5: closing ALL regions returns to no-live-regions (main may show).
    #[test]
    fn closing_all_empties_manager() {
        let mut mgr = RegionSessionManager::new();
        mgr.register_default(rect(0., 0., 320., 200.)).unwrap();
        let id = mgr.create(rect(5., 5., 100., 80.)).unwrap();
        assert!(mgr.has_live_regions());

        assert!(mgr.remove(&id));
        assert!(mgr.has_live_regions()); // default still alive
        assert!(mgr.remove(DEFAULT_REGION_ID));
        assert!(!mgr.has_live_regions());
        assert_eq!(mgr.active_count(), 0);
    }

    /// M3.5: default + regular coexist; removing default keeps regular alive.
    #[test]
    fn default_and_regular_coexist() {
        let mut mgr = RegionSessionManager::new();
        mgr.register_default(rect(0., 0., 320., 200.)).unwrap();
        let id = mgr.create(rect(5., 5., 100., 80.)).unwrap();
        assert_eq!(mgr.active_count(), 2);
        assert_eq!(mgr.non_default_count(), 1);
        assert!(mgr.remove(DEFAULT_REGION_ID));
        assert!(mgr.contains(&id));
        assert_eq!(mgr.active_count(), 1);
    }

    /// M3.5: re-creating after reaching the limit fails until one is closed.
    #[test]
    fn limit_rejects_until_slot_freed() {
        let mut mgr = RegionSessionManager::new();
        for _ in 0..MAX_REGIONS {
            mgr.create(rect(0., 0., 100., 100.)).unwrap();
        }
        assert!(mgr.create(rect(0., 0., 100., 100.)).is_err());
        // Free one slot, then create succeeds.
        let first = mgr
            .list_info()
            .into_iter()
            .map(|i| i.region_id)
            .min()
            .unwrap();
        assert!(mgr.remove(&first));
        let new_id = mgr.create(rect(0., 0., 100., 100.)).unwrap();
        assert!(!new_id.is_empty());
        assert_eq!(mgr.active_count(), MAX_REGIONS);
    }

    #[test]
    fn list_info_order_and_shape() {
        let mut mgr = RegionSessionManager::new();
        mgr.register_default(rect(0., 0., 300., 200.)).unwrap();
        let id = mgr.create(rect(5., 5., 100., 80.)).unwrap();
        assert_eq!(id, "1");
        mgr.get_mut(&id).unwrap().continuous = true;
        let infos = mgr.list_info();
        assert_eq!(infos.len(), 2);
        // default first, then numeric.
        assert_eq!(infos[0].region_id, DEFAULT_REGION_ID);
        assert_eq!(infos[0].label, "ocr-region-frame");
        assert_eq!(infos[1].region_id, "1");
        assert_eq!(infos[1].label, "ocr-region-frame-1");
        assert!(infos[1].continuous);
        assert_eq!(infos[1].mode, "translated");
        assert!(infos[1].created_at_ms > 0);
        // default's mode default is translated.
        assert_eq!(infos[0].mode, "translated");
    }

    #[test]
    fn set_mode_parses_and_updates() {
        let mut mgr = RegionSessionManager::new();
        let id = mgr.create(rect(0., 0., 100., 100.)).unwrap();
        mgr.get_mut(&id).unwrap().mode = RegionMode::parse("image").unwrap();
        assert_eq!(mgr.get(&id).unwrap().mode, RegionMode::Image);
        assert_eq!(RegionMode::parse("translated"), Some(RegionMode::Translated));
        assert_eq!(RegionMode::parse("bogus"), None);
        assert_eq!(RegionMode::Image.as_str(), "image");
    }

    #[test]
    fn manager_singleton_is_shared() {
        let a = region_manager();
        let b = region_manager();
        assert!(std::ptr::eq(a, b));
    }
}
