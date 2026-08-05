//! Clipboard selection via synthetic Ctrl+C.
//! Ported patterns from:
//! - Easydict TextSelectionService (ClipWait, non-text suppress, restore matrix)
//! - STranslate ClipboardHelper (modifier flush, multi-format text, OpenClipboard retry)

use super::process_class::{foreground_process, normalize_process_name};
use super::{SelectionProvider, SelectionResult};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Easydict ClipWait classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipWaitResult {
    Success,
    NonTextPayload,
    Timeout,
}

/// S5-6: clipboard open failure (10×100ms retry exhausted).
/// Used to trigger UIA fallback instead of silently returning None.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClipboardOpenError {
    /// `OpenClipboard` failed after `OPEN_CLIP_RETRIES` attempts.
    OpenFailed,
}

/// Easydict ResolveClipboardRestore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardRestoreAction {
    None,
    RestoreText,
    ClearToEmpty,
}

struct ProcessClipStats {
    consecutive_non_text: u32,
    suppressed_until: Option<Instant>,
}

static PROCESS_STATS: Mutex<Option<HashMap<String, ProcessClipStats>>> = Mutex::new(None);

const NON_TEXT_FAILURE_THRESHOLD: u32 = 2;
const SUPPRESSION_WINDOW: Duration = Duration::from_secs(5 * 60);
const OPEN_CLIP_RETRIES: u32 = 10;
const OPEN_CLIP_SLEEP_MS: u64 = 100; // STranslate 10×100ms
const CLIPWAIT_NORMAL_MS: u64 = 450; // Easydict default
const CLIPWAIT_ELECTRON_MS: u64 = 600; // Easydict Electron (P1: 1200ms was too slow for Chrome/Edge/VSCode)
const CLIPWAIT_POLL_MS: u64 = 10;

fn with_stats<R>(f: impl FnOnce(&mut HashMap<String, ProcessClipStats>) -> R) -> R {
    let mut guard = PROCESS_STATS.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
    f(guard.as_mut().unwrap())
}

/// Easydict: skip clipboard (and full path while suppressed) after non-text spam.
pub fn is_process_clipboard_suppressed(process_name: &str) -> bool {
    let key = normalize_process_name(process_name);
    with_stats(|map| {
        map.get(&key)
            .and_then(|s| s.suppressed_until)
            .map(|until| Instant::now() < until)
            .unwrap_or(false)
    })
}

fn record_outcome(process_name: &str, outcome: ClipWaitResult) {
    let key = normalize_process_name(process_name);
    with_stats(|map| {
        let stats = map.entry(key.clone()).or_insert(ProcessClipStats {
            consecutive_non_text: 0,
            suppressed_until: None,
        });
        match outcome {
            ClipWaitResult::Success => {
                stats.consecutive_non_text = 0;
                stats.suppressed_until = None;
            },
            ClipWaitResult::NonTextPayload => {
                stats.consecutive_non_text = stats.consecutive_non_text.saturating_add(1);
                if stats.consecutive_non_text >= NON_TEXT_FAILURE_THRESHOLD {
                    stats.suppressed_until = Some(Instant::now() + SUPPRESSION_WINDOW);
                    tracing::warn!(
                        "[clipboard] suppress '{}' for {}s after {} non-text failures",
                        key,
                        SUPPRESSION_WINDOW.as_secs(),
                        stats.consecutive_non_text
                    );
                }
            },
            ClipWaitResult::Timeout => {
                // timeouts do not engage suppress
            },
        }
    });
}

/// Easydict `RecordOutcome(Success)` for a non-clipboard selection success (e.g. UIA).
/// A successful UIA pick rehabilitates a process that previously accumulated non-text
/// clipboard failures: resets `consecutive_non_text` and clears any active suppress,
/// so the next single non-text clipboard payload doesn't immediately trip the 5min
/// window on top of a stale count. Mirrors `TextSelectionService.RecordOutcome(Success)`.
pub fn record_selection_success(process_name: &str) {
    record_outcome(process_name, ClipWaitResult::Success);
}

fn resolve_clipboard_restore(
    original_text: Option<&str>,
    original_was_empty: bool,
    clipboard_changed: bool,
    copied_text: Option<&str>,
) -> (ClipboardRestoreAction, Option<String>) {
    if !clipboard_changed {
        return (ClipboardRestoreAction::None, None);
    }
    if let Some(orig) = original_text {
        if Some(orig) == copied_text {
            // same text — keep richer formats on clipboard
            return (ClipboardRestoreAction::None, None);
        }
        return (ClipboardRestoreAction::RestoreText, Some(orig.to_string()));
    }
    if original_was_empty {
        return (ClipboardRestoreAction::ClearToEmpty, None);
    }
    // original was non-text (image/RTF) — leave as-is (Easydict issue #168)
    (ClipboardRestoreAction::None, None)
}

pub struct ClipboardSelectionProvider;

#[async_trait::async_trait]
impl SelectionProvider for ClipboardSelectionProvider {
    async fn get_selection(&self) -> Option<SelectionResult> {
        // S5-6: when open_clipboard_retry fails (10×100ms exhausted, usually
        // because another process holds the clipboard lock or RDP session is
        // unstable), fall back to UIA instead of silently returning None.
        // This is safe because UIA doesn't touch the clipboard at all — it
        // reads the selection via TextPattern/ValuePattern COM calls.
        let spawn_result = tokio::task::spawn_blocking(get_clipboard_selection).await;

        match spawn_result {
            Ok(Ok(Some((text, window_title)))) => {
                if text.trim().is_empty() {
                    return None;
                }
                tracing::info!(
                    "[clipboard] Got selection: {} chars from '{}'",
                    text.trim().len(),
                    window_title
                );
                Some(SelectionResult {
                    text: text.trim().to_string(),
                    source_app: detect_app_from_title(&window_title),
                    window_title,
                    bounds: None,
                    confidence: 0.7,
                    provider: "clipboard",
                })
            },
            Ok(Ok(None)) => None, // normal failure (timeout, unchanged, etc.)
            Ok(Err(ClipboardOpenError::OpenFailed)) => {
                tracing::warn!(
                    "[clipboard] OpenClipboard failed after {} retries — fallback to UIA",
                    OPEN_CLIP_RETRIES
                );
                super::uiautomation::UiAutomationSelectionProvider.get_selection().await
            },
            Err(e) => {
                tracing::warn!("[clipboard] spawn_blocking join error: {}", e);
                None
            },
        }
    }

    fn name(&self) -> &'static str {
        "clipboard"
    }

    fn priority(&self) -> u32 {
        100
    }
}

fn get_clipboard_selection() -> Result<Option<(String, String)>, ClipboardOpenError> {
    #[cfg(target_os = "windows")]
    {
        get_clipboard_selection_win()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(None)
    }
}

#[cfg(target_os = "windows")]
fn get_clipboard_selection_win() -> Result<Option<(String, String)>, ClipboardOpenError> {
    // M4-03: serialize with other clipboard readers/writers (replace paste,
    // hook monitor) for the Ctrl+C capture window.
    let _clip_lock = crate::clipboard_dedupe::clipboard_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    use std::mem::size_of;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY, VK_C, VK_CONTROL, VK_LCONTROL, VK_RCONTROL,
    };

    extern "system" {
        fn OpenClipboard(hWndNewOwner: *mut std::ffi::c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(uFormat: u32, hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn GetClipboardData(uFormat: u32) -> *mut std::ffi::c_void;
        fn GetClipboardSequenceNumber() -> u32;
        fn IsClipboardFormatAvailable(format: u32) -> i32;
        fn CountClipboardFormats() -> i32;
        fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
        fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;
        fn GlobalSize(hMem: *mut std::ffi::c_void) -> usize;
        fn GetForegroundWindow() -> *mut std::ffi::c_void;
        fn GetOEMCP() -> u32;
        fn GetACP() -> u32;
    }

    const CF_TEXT: u32 = 1;
    const CF_OEMTEXT: u32 = 7;
    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 0x0002;

    let fg = foreground_process();
    let process_name = fg
        .as_ref()
        .map(|p| p.process_name.clone())
        .unwrap_or_else(|| "unknown".into());
    let is_electron_or_browser = fg
        .as_ref()
        .map(|p| p.is_electron || p.is_browser)
        .unwrap_or(false);

    if is_process_clipboard_suppressed(&process_name) {
        tracing::debug!(
            "[clipboard] process '{}' suppressed (non-text history) — skip",
            process_name
        );
        return Ok(None);
    }

    fn make_input(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
        let extra = crate::selection::mouse_hook::MOON_SYNTHETIC_KEY;
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: extra,
                },
            },
        }
    }

    // SAFETY: SendInput with a stack-allocated INPUT slice of known length.
    // No preconditions beyond a valid pointer + correct struct size.
    unsafe fn release_modifiers() {
        // P1 fix: only release Ctrl — releasing Shift/Alt broke reverse
        // selection and Alt+click context menus. Ctrl alone is enough to
        // ensure the synthetic Ctrl+C below is not turned into Ctrl+Shift+C.
        let mods = [VK_CONTROL, VK_LCONTROL, VK_RCONTROL];
        let inputs: Vec<INPUT> = mods
            .iter()
            .map(|vk| make_input(*vk, KEYEVENTF_KEYUP))
            .collect();
        let _ = SendInput(&inputs, size_of::<INPUT>() as i32);
    }

    // SAFETY: OpenClipboard(NULL) — no HWND ownership requirement, retry loop
    // just polls the Win32 clipboard lock. CloseClipboard is called by callers.
    // STranslate: 10 × 100ms
    unsafe fn open_clipboard_retry() -> bool {
        for _ in 0..OPEN_CLIP_RETRIES {
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                return true;
            }
            std::thread::sleep(Duration::from_millis(OPEN_CLIP_SLEEP_MS));
        }
        false
    }

    // SAFETY: GetClipboardData returns a valid HGLOBAL (checked for null) and
    // GlobalLock/GlobalSize/GlobalUnlock are paired. The slice is bounded by
    // GlobalSize and only read until the NUL terminator.
    //
    // S5-9: CF_TEXT is ANSI (system ACP) and CF_OEMTEXT is OEM (DOS ACP).
    // Previously both went through `String::from_utf8_lossy`, which mangled
    // any non-ASCII byte sequence (e.g. Shift-JIS 0x82 0x71 → replacement
    // chars). We now decode via `encoding_rs` using the codepage returned by
    // GetACP()/GetOEMCP(). For CF_TEXT we still try UTF-8 first, because
    // many modern apps put UTF-8 bytes into CF_TEXT despite the spec.
    unsafe fn read_ansi_or_oem(format: u32, oem: bool) -> Option<String> {
        let h = GetClipboardData(format);
        if h.is_null() {
            return None;
        }
        let p = GlobalLock(h);
        if p.is_null() {
            return None;
        }
        let size = GlobalSize(h);
        let bytes = if size > 0 {
            std::slice::from_raw_parts(p as *const u8, size)
        } else {
            GlobalUnlock(h);
            return None;
        };
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        let raw = &bytes[..end];
        let cp = if oem { GetOEMCP() } else { GetACP() };
        let s = decode_with_codepage(raw, cp);
        GlobalUnlock(h);
        Some(s)
    }

    // SAFETY: Clipboard is opened via open_clipboard_retry and always closed
    // with CloseClipboard before returning. HGLOBAL handles are null-checked
    // and locked/unlocked in pairs.
    //
    // S5-6: returns Err(ClipboardOpenError::OpenFailed) when
    // open_clipboard_retry exhausts, so the caller can fall back to UIA
    // instead of silently treating it as "empty clipboard".
    unsafe fn probe_clipboard() -> Result<(bool, Option<String>, i32), ClipboardOpenError> {
        if !open_clipboard_retry() {
            return Err(ClipboardOpenError::OpenFailed);
        }
        let formats = CountClipboardFormats();
        let has_text = IsClipboardFormatAvailable(CF_UNICODETEXT) != 0
            || IsClipboardFormatAvailable(CF_TEXT) != 0
            || IsClipboardFormatAvailable(CF_OEMTEXT) != 0;
        // read without nested open — already open
        let text = if has_text {
            // inline read while open
            let t = if IsClipboardFormatAvailable(CF_UNICODETEXT) != 0 {
                let h = GetClipboardData(CF_UNICODETEXT);
                if !h.is_null() {
                    let p = GlobalLock(h);
                    if !p.is_null() {
                        let size = GlobalSize(h);
                        let out = if size > 2 {
                            let slice = std::slice::from_raw_parts(p as *const u16, size / 2);
                            Some(
                                String::from_utf16_lossy(slice)
                                    .trim_end_matches('\0')
                                    .to_string(),
                            )
                        } else {
                            None
                        };
                        GlobalUnlock(h);
                        out
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else if IsClipboardFormatAvailable(CF_TEXT) != 0 {
                read_ansi_or_oem(CF_TEXT, false)
            } else {
                read_ansi_or_oem(CF_OEMTEXT, true)
            };
            t
        } else {
            None
        };
        CloseClipboard();
        let usable = text.as_ref().map(|t| !t.trim().is_empty()).unwrap_or(false);
        Ok((usable, text, formats))
    }

    struct SyntheticGuard;
    impl Drop for SyntheticGuard {
        fn drop(&mut self) {
            crate::clipboard_dedupe::end_synthetic_clipboard();
        }
    }

    // SAFETY: Foreground HWND comes from GetForegroundWindow (validated for
    // null). All clipboard operations are guarded by open_clipboard_retry/
    // CloseClipboard pairs; HGLOBAL handles are null-checked and paired with
    // GlobalLock/GlobalUnlock. SendInput takes a stack INPUT slice.
    unsafe {
        crate::clipboard_dedupe::begin_synthetic_clipboard();
        let _synthetic = SyntheticGuard;

        // Do NOT AttachThreadInput / SetForegroundWindow — both can reshuffle
        // Windows Terminal tabs / multi-window focus when user clicks 「译」 later
        // or when Ctrl+C is sent from a background worker thread.
        let hwnd_raw = GetForegroundWindow();
        if hwnd_raw.is_null() {
            return Ok(None);
        }
        let window_title = get_window_title(hwnd_raw);

        // Snapshot original clipboard (Easydict: text + empty vs non-text)
        let seq_before = GetClipboardSequenceNumber();
        let (had_text, original_text, format_count) = {
            // S5-6: propagate open_clipboard failure so the caller can
            // fall back to UIA instead of treating it as "empty clipboard".
            let (usable, text, formats) = probe_clipboard()?;
            let original_was_empty = formats == 0;
            let original_had_text = usable;
            (
                original_had_text,
                text.map(|t| t.trim().to_string()),
                if original_was_empty {
                    0
                } else {
                    formats.max(0)
                },
            )
        };
        let original_was_empty = format_count == 0;
        let _ = had_text;

        release_modifiers();
        std::thread::sleep(Duration::from_millis(20));

        let inputs = [
            make_input(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
            make_input(VK_C, KEYBD_EVENT_FLAGS(0)),
            make_input(VK_C, KEYEVENTF_KEYUP),
            make_input(VK_CONTROL, KEYEVENTF_KEYUP),
        ];
        let sent = SendInput(&inputs, size_of::<INPUT>() as i32);
        if sent == 0 {
            tracing::warn!("[clipboard] SendInput returned 0");
        }

        // Easydict ClipWait with non-text classification
        let timeout_ms = if is_electron_or_browser {
            CLIPWAIT_ELECTRON_MS
        } else {
            CLIPWAIT_NORMAL_MS
        };
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let mut wait = ClipWaitResult::Timeout;
        let mut consecutive_non_text = 0u32;
        let mut selected: Option<String> = None;

        while Instant::now() < deadline {
            if GetClipboardSequenceNumber() != seq_before {
                // S5-6: if open_clipboard fails mid-loop (another process
                // holds the lock), bail out and let UIA handle it.
                let (usable, text, formats) = match probe_clipboard() {
                    Ok(v) => v,
                    Err(e) => return Err(e),
                };
                if usable {
                    selected = text.map(|t| t.trim().to_string());
                    wait = ClipWaitResult::Success;
                    break;
                } else if formats > 0 {
                    consecutive_non_text += 1;
                    if consecutive_non_text >= 2 {
                        wait = ClipWaitResult::NonTextPayload;
                        tracing::debug!(
                            "[clipboard] non-text payload from '{}' (formats={})",
                            process_name,
                            formats
                        );
                        break;
                    }
                } else {
                    consecutive_non_text = 0;
                }
            }
            std::thread::sleep(Duration::from_millis(CLIPWAIT_POLL_MS));
        }

        if wait == ClipWaitResult::Timeout {
            tracing::debug!(
                "[clipboard] ClipWait timeout {}ms process='{}'",
                timeout_ms,
                process_name
            );
        }

        // Reject unchanged text (stale clipboard)
        if let Some(ref t) = selected {
            if let Some(ref orig) = original_text {
                if orig == t {
                    tracing::debug!("[clipboard] text unchanged after Ctrl+C — no selection");
                    selected = None;
                    if wait == ClipWaitResult::Success {
                        wait = ClipWaitResult::Timeout;
                    }
                }
            }
        }

        let clipboard_changed = GetClipboardSequenceNumber() != seq_before;
        let (restore_action, restore_text) = resolve_clipboard_restore(
            original_text.as_deref(),
            original_was_empty,
            clipboard_changed,
            selected.as_deref(),
        );

        match restore_action {
            ClipboardRestoreAction::None => {},
            ClipboardRestoreAction::ClearToEmpty => {
                for _ in 0..3 {
                    if open_clipboard_retry() {
                        EmptyClipboard();
                        CloseClipboard();
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            },
            ClipboardRestoreAction::RestoreText => {
                if let Some(ref text) = restore_text {
                    for _ in 0..3 {
                        if open_clipboard_retry() {
                            EmptyClipboard();
                            let wide: Vec<u16> =
                                text.encode_utf16().chain(std::iter::once(0)).collect();
                            let bytes = wide.len() * 2;
                            let h_mem = GlobalAlloc(GMEM_MOVEABLE, bytes);
                            if !h_mem.is_null() {
                                let p = GlobalLock(h_mem);
                                if !p.is_null() {
                                    std::ptr::copy_nonoverlapping(
                                        wide.as_ptr() as *const u8,
                                        p as *mut u8,
                                        bytes,
                                    );
                                    GlobalUnlock(h_mem);
                                    SetClipboardData(CF_UNICODETEXT, h_mem);
                                }
                            }
                            CloseClipboard();
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            },
        }

        record_outcome(&process_name, wait);

        if let Some(ref t) = selected {
            crate::clipboard_dedupe::mark_clipboard_text(t);
            return Ok(Some((t.clone(), window_title)));
        }
        Ok(None)
    }
}

/// SAFETY: GetWindowTextW writes into a stack-allocated [u16; 512] buffer.
/// Caller passes a valid HWND (or null, which yields an empty string).
#[cfg(target_os = "windows")]
unsafe fn get_window_title(hwnd: *mut std::ffi::c_void) -> String {
    extern "system" {
        fn GetWindowTextW(hWnd: *mut std::ffi::c_void, lpString: *mut u16, nMaxCount: i32) -> i32;
    }
    let mut buf = [0u16; 512];
    let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    if len > 0 {
        String::from_utf16_lossy(&buf[..len as usize])
    } else {
        String::new()
    }
}

fn detect_app_from_title(title: &str) -> String {
    if let Some(pos) = title.rfind(" - ") {
        let app = &title[pos + 3..];
        if !app.is_empty() {
            return app.to_string();
        }
    }
    title.to_string()
}

/// S5-9: decode a byte slice using a Windows codepage number.
///
/// `codepage` is the value returned by `GetACP()` (for CF_TEXT) or
/// `GetOEMCP()` (for CF_OEMTEXT). We map the handful of codepages that
/// actually show up on user systems to the corresponding `encoding_rs`
/// encoder. Unknown codepages fall back to UTF-8 (with lossy replacement),
/// matching the old behavior — so this never makes things worse.
///
/// This is a free function (not inside `get_clipboard_selection_win`) so it
/// can be unit-tested without touching the Win32 clipboard.
fn decode_with_codepage(raw: &[u8], codepage: u32) -> String {
    // Try UTF-8 first: many modern apps (browsers, Electron, VS Code, …)
    // put UTF-8 bytes into CF_TEXT regardless of the system ACP. UTF-8 is a
    // strict superset of ASCII, so pure-ASCII text round-trips for free.
    if let Ok(s) = std::str::from_utf8(raw) {
        return s.trim_end_matches('\0').to_string();
    }

    let encoding = match codepage {
        // CJK — the cases that actually produced user-visible mojibake
        932 => Some(encoding_rs::SHIFT_JIS),   // Japanese Shift-JIS
        936 => Some(encoding_rs::GBK),         // Simplified Chinese GBK
        949 => Some(encoding_rs::EUC_KR),      // Korean (Windows 949 = UHC, a superset of EUC-KR; encoding_rs unified them)
        950 => Some(encoding_rs::BIG5),        // Traditional Chinese Big5
        // Cyrillic DOS codepage — the one that actually surfaces on
        // Russian-locale legacy console apps.
        866 => Some(encoding_rs::IBM866),
        // Windows ANSI codepages (rarely surface as CF_OEMTEXT but can
        // appear as CF_TEXT on systems with a non-Latin ACP)
        1250 => Some(encoding_rs::WINDOWS_1250),
        1251 => Some(encoding_rs::WINDOWS_1251),
        1252 => Some(encoding_rs::WINDOWS_1252),
        1253 => Some(encoding_rs::WINDOWS_1253),
        1254 => Some(encoding_rs::WINDOWS_1254),
        1255 => Some(encoding_rs::WINDOWS_1255),
        1256 => Some(encoding_rs::WINDOWS_1256),
        1257 => Some(encoding_rs::WINDOWS_1257),
        1258 => Some(encoding_rs::WINDOWS_1258),
        // Western DOS codepages (fallback for English-locale DOS apps)
        437 | 850 | 852 | 860 | 862 | 863 | 865 | 857 => Some(encoding_rs::WINDOWS_1252),
        _ => None,
    };

    match encoding {
        Some(enc) => {
            let (cow, _, _) = enc.decode(raw);
            cow.trim_end_matches('\0').to_string()
        },
        None => {
            // Unknown codepage — preserve old behavior so we never regress.
            String::from_utf8_lossy(raw).trim_end_matches('\0').to_string()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_matrix_easydict() {
        assert_eq!(
            resolve_clipboard_restore(Some("a"), false, false, Some("b")).0,
            ClipboardRestoreAction::None
        );
        assert_eq!(
            resolve_clipboard_restore(Some("a"), false, true, Some("a")).0,
            ClipboardRestoreAction::None
        );
        assert_eq!(
            resolve_clipboard_restore(Some("a"), false, true, Some("b")).0,
            ClipboardRestoreAction::RestoreText
        );
        assert_eq!(
            resolve_clipboard_restore(None, true, true, Some("x")).0,
            ClipboardRestoreAction::ClearToEmpty
        );
        assert_eq!(
            resolve_clipboard_restore(None, false, true, Some("x")).0,
            ClipboardRestoreAction::None
        );
    }

    /// S5-6: ClipboardOpenError distinguishes open_clipboard_retry failure
    /// (which should trigger UIA fallback) from a normal None (timeout /
    /// unchanged / non-text payload, which should NOT).
    #[test]
    fn clipboard_open_error_is_distinct_from_none() {
        // Ok(None)  = clipboard opened fine, but no usable selection
        // Err(OpenFailed) = could not even open the clipboard → UIA fallback
        assert_ne!(
            Ok::<Option<(String, String)>, ClipboardOpenError>(None),
            Err(ClipboardOpenError::OpenFailed)
        );
        // equality + copy semantics (used by `match` in get_selection)
        let e1 = ClipboardOpenError::OpenFailed;
        let e2 = e1;
        assert_eq!(e1, e2);
    }

    /// S5-6: verify the non-windows stub returns Ok(None) (not an error),
    /// so non-windows hosts don't spuriously trigger UIA fallback.
    #[test]
    fn non_windows_stub_returns_ok_none() {
        #[cfg(not(target_os = "windows"))]
        {
            assert_eq!(get_clipboard_selection(), Ok(None));
        }
        #[cfg(target_os = "windows")]
        {
            // On windows the function touches GetForegroundWindow — we
            // only assert the signature compiles and the type matches.
            let _: Result<Option<(String, String)>, ClipboardOpenError> =
                get_clipboard_selection();
        }
    }

    // ── S5-9: OEM / ANSI codepage decoding ────────────────────────────────

    /// Shift-JIS (cp932) is the whole reason S5-9 was filed: Japanese
    /// clipboard text from legacy apps came through as mojibake because
    /// `from_utf8_lossy` replaced every non-ASCII byte with U+FFFD.
    #[test]
    fn decode_shift_jis_japanese() {
        // "日本語" in Shift-JIS: 93 fa 96 7b 8c ea
        let bytes = [0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea];
        assert_eq!(decode_with_codepage(&bytes, 932), "日本語");
    }

    /// GBK (cp936) — Simplified Chinese, same class of bug as Shift-JIS.
    #[test]
    fn decode_gbk_chinese() {
        // "中文" in GBK: d6 d0 ce c4
        let bytes = [0xd6, 0xd0, 0xce, 0xc4];
        assert_eq!(decode_with_codepage(&bytes, 936), "中文");
    }

    /// Big5 (cp950) — Traditional Chinese.
    #[test]
    fn decode_big5_chinese() {
        // "中文" in Big5: a4 a4 a4 e5
        let bytes = [0xa4, 0xa4, 0xa4, 0xe5];
        assert_eq!(decode_with_codepage(&bytes, 950), "中文");
    }

    /// UTF-8 must win even when the codepage says otherwise: modern apps
    /// (browsers, Electron, VS Code) put UTF-8 into CF_TEXT on every locale.
    #[test]
    fn decode_prefers_utf8_over_codepage() {
        // "日本語" as UTF-8 bytes, but pretend the codepage is 1252.
        let bytes = "日本語".as_bytes();
        assert_eq!(decode_with_codepage(bytes, 1252), "日本語");
    }

    /// Pure ASCII round-trips regardless of codepage.
    #[test]
    fn decode_ascii_agnostic() {
        let bytes = b"Hello, world!";
        for &cp in &[932usize, 936, 949, 950, 1252, 437, 9999] {
            assert_eq!(
                decode_with_codepage(bytes, cp as u32),
                "Hello, world!",
                "cp={cp}"
            );
        }
    }

    /// Unknown codepage falls back to lossy UTF-8 (old behavior preserved).
    #[test]
    fn decode_unknown_codepage_falls_back() {
        // Invalid UTF-8 + unknown cp → replacement chars, not a panic.
        let bytes = [0xff, 0xfe, 0xfd];
        let s = decode_with_codepage(&bytes, 9999);
        assert!(!s.is_empty());
        assert!(s.contains('\u{FFFD}'));
    }

    /// Cyrillic DOS (cp866) — Russian text from legacy console apps.
    /// WHATWG IBM866 layout:
    ///   0x80-0x8F = А-П (uppercase), 0x90-0x9F = Р-Я (uppercase)
    ///   0xA0-0xAF = а-п (lowercase), 0xE0-0xEF = р-я (lowercase)
    #[test]
    fn decode_cp866_russian() {
        // "Привет" (mixed case) in CP866:
        //   П=0x8F  р=0xE0  и=0xA8  в=0xA2  е=0xA5  т=0xE2
        let bytes = [0x8f, 0xe0, 0xa8, 0xa2, 0xa5, 0xe2];
        assert_eq!(decode_with_codepage(&bytes, 866), "Привет");
    }

    /// NUL terminator should be stripped (clipboard payloads are NUL-padded).
    #[test]
    fn decode_strips_trailing_nul() {
        let bytes = b"abc\0\0";
        assert_eq!(decode_with_codepage(bytes, 1252), "abc");
    }
}
