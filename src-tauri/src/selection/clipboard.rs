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
const CLIPWAIT_ELECTRON_MS: u64 = 1200; // Easydict Electron
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
        let result = tokio::task::spawn_blocking(get_clipboard_selection)
            .await
            .ok()
            .flatten();
        let (text, window_title) = result?;
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
    }

    fn name(&self) -> &'static str {
        "clipboard"
    }

    fn priority(&self) -> u32 {
        100
    }
}

fn get_clipboard_selection() -> Option<(String, String)> {
    #[cfg(target_os = "windows")]
    {
        get_clipboard_selection_win()
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

#[cfg(target_os = "windows")]
fn get_clipboard_selection_win() -> Option<(String, String)> {
    use std::mem::size_of;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY, VK_C, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU,
        VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
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
        return None;
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

    unsafe fn release_modifiers() {
        let mods = [
            VK_CONTROL,
            VK_LCONTROL,
            VK_RCONTROL,
            VK_SHIFT,
            VK_LSHIFT,
            VK_RSHIFT,
            VK_MENU,
            VK_LMENU,
            VK_RMENU,
            VK_LWIN,
            VK_RWIN,
        ];
        let inputs: Vec<INPUT> = mods
            .iter()
            .map(|vk| make_input(*vk, KEYEVENTF_KEYUP))
            .collect();
        let _ = SendInput(&inputs, size_of::<INPUT>() as i32);
    }

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
        let s = if oem {
            // best-effort OEM: treat as system default if encoding crate not available
            let _cp = GetOEMCP();
            String::from_utf8_lossy(raw).to_string()
        } else {
            String::from_utf8_lossy(raw).to_string()
        };
        GlobalUnlock(h);
        Some(s)
    }

    unsafe fn probe_clipboard() -> (bool, Option<String>, i32) {
        if !open_clipboard_retry() {
            return (false, None, -1);
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
        (usable, text, formats)
    }

    struct SyntheticGuard;
    impl Drop for SyntheticGuard {
        fn drop(&mut self) {
            crate::clipboard_dedupe::end_synthetic_clipboard();
        }
    }

    unsafe {
        crate::clipboard_dedupe::begin_synthetic_clipboard();
        let _synthetic = SyntheticGuard;

        // Do NOT AttachThreadInput / SetForegroundWindow — both can reshuffle
        // Windows Terminal tabs / multi-window focus when user clicks 「译」 later
        // or when Ctrl+C is sent from a background worker thread.
        let hwnd_raw = GetForegroundWindow();
        if hwnd_raw.is_null() {
            return None;
        }
        let window_title = get_window_title(hwnd_raw);

        // Snapshot original clipboard (Easydict: text + empty vs non-text)
        let seq_before = GetClipboardSequenceNumber();
        let (had_text, original_text, format_count) = {
            let (usable, text, formats) = probe_clipboard();
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
                let (usable, text, formats) = probe_clipboard();
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
            return Some((t.clone(), window_title));
        }
        None
    }
}

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
}
