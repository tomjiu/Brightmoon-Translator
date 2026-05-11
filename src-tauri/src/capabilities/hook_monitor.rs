use std::sync::Arc;
use std::io::Cursor;
use tokio::sync::{mpsc, Mutex};
use windows::core::Interface;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationTextPattern,
    SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK,
    UIA_TextPatternId,
};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::WindowsAndMessaging::*;

/// Monitored text from a window
#[derive(Debug, Clone)]
pub struct MonitoredText {
    pub window_title: String,
    pub process_name: String,
    pub text: String,
    pub timestamp: i64,
    /// Source: "uia", "clipboard", "ocr", "hook"
    pub source: String,
    /// Bounding rectangle of the text element: (x, y, width, height) in screen pixels
    pub text_rect: Option<(i32, i32, i32, i32)>,
}

/// Hook monitor with four capture sources:
/// 1. UI Automation (TextPattern) — polling, best for supported apps
/// 2. Clipboard — passive watch for copy events
/// 3. OCR — screenshot + WinRT OCR fallback
/// 4. Win32 Event Hook — SetWinEventHook for EVENT_OBJECT_TEXTCHANGED
pub struct HookMonitor {
    running: Arc<Mutex<bool>>,
    sender: Option<mpsc::UnboundedSender<MonitoredText>>,
}

impl HookMonitor {
    pub fn new() -> Self {
        Self {
            running: Arc::new(Mutex::new(false)),
            sender: None,
        }
    }

    pub async fn start(
        &mut self,
        callback: impl Fn(MonitoredText) + Send + Sync + 'static,
    ) -> Result<(), String> {
        let mut running = self.running.lock().await;
        if *running {
            return Err("Monitor already running".to_string());
        }
        *running = true;
        drop(running);

        let (tx, mut rx) = mpsc::unbounded_channel::<MonitoredText>();
        self.sender = Some(tx.clone());

        let running_clone = self.running.clone();

        // Callback dispatch task
        tokio::spawn(async move {
            while let Some(text) = rx.recv().await {
                callback(text);
            }
        });

        // Source 1: UI Automation polling
        let tx_uia = tx.clone();
        let running_uia = running_clone.clone();
        tokio::spawn(async move {
            uia_monitor_task(running_uia, tx_uia).await;
        });

        // Source 2: Clipboard
        let tx_clip = tx.clone();
        let running_clip = running_clone.clone();
        tokio::spawn(async move {
            clipboard_monitor_task(running_clip, tx_clip).await;
        });

        // Source 3: OCR fallback
        let tx_ocr = tx.clone();
        let running_ocr = running_clone.clone();
        tokio::spawn(async move {
            ocr_monitor_task(running_ocr, tx_ocr).await;
        });

        // Source 4: Win32 event hook
        let tx_hook = tx.clone();
        let running_hook = running_clone.clone();
        tokio::spawn(async move {
            win_event_hook_task(running_hook, tx_hook).await;
        });

        Ok(())
    }

    pub async fn stop(&self) {
        let mut running = self.running.lock().await;
        *running = false;
    }

    pub async fn is_running(&self) -> bool {
        *self.running.lock().await
    }
}

// ─── Source 1: UI Automation ────────────────────────────────────────────────

async fn uia_monitor_task(
    running: Arc<Mutex<bool>>,
    tx: mpsc::UnboundedSender<MonitoredText>,
) {
    let mut last_text = String::new();
    let mut last_hwnd: usize = 0;

    loop {
        {
            let r = running.lock().await;
            if !*r { break; }
        }

        let result = tokio::task::spawn_blocking(capture_foreground_text)
            .await.ok().flatten();

        if let Some((text, hwnd_raw, window_title, process_name, text_rect)) = result {
            if text != last_text || hwnd_raw != last_hwnd {
                last_text = text.clone();
                last_hwnd = hwnd_raw;
                let _ = tx.send(MonitoredText {
                    window_title, process_name, text,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    source: "uia".to_string(),
                    text_rect,
                });
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}

// ─── Source 2: Clipboard (event-driven via AddClipboardFormatListener) ──────

async fn clipboard_monitor_task(
    running: Arc<Mutex<bool>>,
    tx: mpsc::UnboundedSender<MonitoredText>,
) {
    // Channel from the clipboard listener thread to async runtime
    let (clip_tx, mut clip_rx) = mpsc::unbounded_channel::<String>();

    // Spawn a dedicated thread with hidden window for clipboard notifications
    let listener_thread = std::thread::spawn(move || unsafe {
        use windows::Win32::System::DataExchange::{
            AddClipboardFormatListener, RemoveClipboardFormatListener,
        };

        CLIP_SENDER.with(|s| *s.borrow_mut() = Some(clip_tx));

        // Create a message-only window
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            windows::core::w!("STATIC"),
            windows::core::w!("MoonClipListener"),
            WINDOW_STYLE::default(),
            0, 0, 0, 0,
            None, None, None, None,
        );

        let hwnd = match hwnd {
            Ok(h) => h,
            Err(_) => return,
        };

        // Register for clipboard notifications
        let _ = AddClipboardFormatListener(hwnd);

        // Message loop
        let mut msg = MSG::default();
        loop {
            let result = GetMessageW(&mut msg, None, 0, 0);
            if result.as_bool() {
                if msg.message == WM_CLIPBOARDUPDATE {
                    // Read clipboard text and forward via channel
                    if let Some(text) = read_clipboard_text() {
                        let trimmed = text.trim().to_string();
                        if !trimmed.is_empty() && trimmed.len() >= 2 {
                            CLIP_SENDER.with(|s| {
                                if let Some(tx) = s.borrow().as_ref() {
                                    let _ = tx.send(trimmed);
                                }
                            });
                        }
                    }
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                break;
            }
        }

        // Cleanup
        let _ = RemoveClipboardFormatListener(hwnd);
        let _ = DestroyWindow(hwnd);
    });

    // Process clipboard events from the listener thread
    let mut last_clip = String::new();
    loop {
        {
            let r = running.lock().await;
            if !*r { break; }
        }

        while let Ok(trimmed) = clip_rx.try_recv() {
            if trimmed == last_clip { continue; }
            last_clip = trimmed.clone();

            let (window_title, process_name) = tokio::task::spawn_blocking(|| unsafe {
                let hwnd = GetForegroundWindow();
                (get_window_title(hwnd), get_process_name(hwnd))
            }).await.unwrap_or_default();

            let _ = tx.send(MonitoredText {
                window_title, process_name,
                text: trimmed,
                timestamp: chrono::Utc::now().timestamp_millis(),
                source: "clipboard".to_string(),
                text_rect: None,
            });
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Signal the listener thread to quit by posting WM_QUIT
    // The thread will exit its message loop
    drop(listener_thread);
}

thread_local! {
    static CLIP_SENDER: std::cell::RefCell<Option<mpsc::UnboundedSender<String>>> =
        std::cell::RefCell::new(None);
}

fn read_clipboard_text() -> Option<String> {
    unsafe {
        use windows::Win32::Foundation::HGLOBAL;
        use windows::Win32::System::DataExchange::{
            CloseClipboard, GetClipboardData, OpenClipboard,
        };
        use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

        const CF_UNICODETEXT: u32 = 13;

        if OpenClipboard(None).is_err() { return None; }

        let result = (|| -> Option<String> {
            let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
            let h_global = HGLOBAL(handle.0);
            let p_data = GlobalLock(h_global);
            if p_data.is_null() { return None; }
            let size = GlobalSize(h_global);
            if size <= 2 {
                let _ = GlobalUnlock(h_global);
                return None;
            }
            let slice = std::slice::from_raw_parts(p_data as *const u16, size / 2);
            let text = String::from_utf16_lossy(slice);
            let text = text.trim_end_matches('\0').to_string();
            let _ = GlobalUnlock(h_global);
            Some(text)
        })();

        let _ = CloseClipboard();
        result
    }
}

// ─── Source 3: OCR Fallback ─────────────────────────────────────────────────

async fn ocr_monitor_task(
    running: Arc<Mutex<bool>>,
    tx: mpsc::UnboundedSender<MonitoredText>,
) {
    let mut last_text = String::new();

    // Delay start to let UIA work first
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    loop {
        {
            let r = running.lock().await;
            if !*r { break; }
        }

        let result = tokio::task::spawn_blocking(|| {
            use screenshots::image::ImageFormat;

            unsafe {
                let hwnd = GetForegroundWindow();
                let mut rect = windows::Win32::Foundation::RECT::default();
                if GetWindowRect(hwnd, &mut rect).is_err() { return None; }

                let width = (rect.right - rect.left) as u32;
                let height = (rect.bottom - rect.top) as u32;
                if width < 100 || height < 100 { return None; }

                let window_title = get_window_title(hwnd);
                let process_name = get_process_name(hwnd);

                let img = crate::commands::capture::capture_area_gdi(
                    rect.left, rect.top, width, height,
                ).ok()?;

                let mut buf = Cursor::new(Vec::new());
                img.write_to(&mut buf, ImageFormat::Png).ok()?;

                let text = crate::ocr_engine::run_winrt_ocr(&buf.into_inner(), None)
                    .ok()??;

                Some((text, window_title, process_name))
            }
        }).await.ok().flatten();

        if let Some((text, window_title, process_name)) = result {
            let trimmed = text.trim().to_string();
            if !trimmed.is_empty() && trimmed != last_text {
                last_text = trimmed.clone();
                let _ = tx.send(MonitoredText {
                    window_title, process_name,
                    text: trimmed,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    source: "ocr".to_string(),
                    text_rect: None,
                });
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

// ─── Source 4: Win32 Event Hook ─────────────────────────────────────────────

/// Run SetWinEventHook in a dedicated thread with a message loop.
/// Listens for EVENT_OBJECT_TEXTCHANGED and EVENT_OBJECT_VALUECHANGED
/// to detect text changes in any foreground window.
async fn win_event_hook_task(
    running: Arc<Mutex<bool>>,
    tx: mpsc::UnboundedSender<MonitoredText>,
) {
    // Channel from the Win32 callback (OS thread) to async runtime
    let (hook_tx, mut hook_rx) = mpsc::unbounded_channel::<(isize, String, String)>();

    // Spawn the Win32 hook thread (must have a message loop)
    let hook_thread = std::thread::spawn(move || unsafe {
        // Store sender in thread-local for the callback
        HOOK_SENDER.with(|s| *s.borrow_mut() = Some(hook_tx));

        // Install event hooks
        let hook_text = SetWinEventHook(
            EVENT_OBJECT_TEXTSELECTIONCHANGED,
            EVENT_OBJECT_TEXTSELECTIONCHANGED,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );

        let hook_value = SetWinEventHook(
            EVENT_OBJECT_VALUECHANGE,
            EVENT_OBJECT_VALUECHANGE,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );

        // Message loop — required for the hook to receive events
        let mut msg = MSG::default();
        loop {
            // Check if we should stop (non-blocking peek)
            if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                // No messages, sleep briefly to avoid busy-wait
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }

        // Cleanup hooks
        if !hook_text.is_invalid() {
            let _ = UnhookWinEvent(hook_text);
        }
        if !hook_value.is_invalid() {
            let _ = UnhookWinEvent(hook_value);
        }
    });

    // Process events from the hook thread
    let mut last_text = String::new();
    loop {
        {
            let r = running.lock().await;
            if !*r { break; }
        }

        while let Ok((hwnd_raw, text, window_title)) = hook_rx.try_recv() {
            let trimmed = text.trim().to_string();
            if trimmed.is_empty() || trimmed == last_text { continue; }
            last_text = trimmed.clone();

            let process_name = tokio::task::spawn_blocking(move || unsafe {
                get_process_name(HWND(hwnd_raw as *mut _))
            }).await.unwrap_or_default();

            let _ = tx.send(MonitoredText {
                window_title, process_name,
                text: trimmed,
                timestamp: chrono::Utc::now().timestamp_millis(),
                source: "hook".to_string(),
                text_rect: None,
            });
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Signal the hook thread to quit
    // (It will exit its message loop when the thread is dropped)
    drop(hook_thread);
}

thread_local! {
    static HOOK_SENDER: std::cell::RefCell<Option<mpsc::UnboundedSender<(isize, String, String)>>> =
        std::cell::RefCell::new(None);
}

/// Win32 event callback — runs on the hook thread.
/// Receives text change events and forwards them via channel.
unsafe extern "system" fn win_event_proc(
    _h_hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    // Only process client area events (OBJID_CLIENT = -4)
    if id_object != OBJID_CLIENT.0 { return; }
    if hwnd.is_invalid() { return; }

    // Try to read text via WM_GETTEXT
    let text = get_wm_text(hwnd);
    if text.is_empty() { return; }

    let window_title = get_window_title(hwnd);
    let hwnd_raw = hwnd.0 as isize;

    HOOK_SENDER.with(|s| {
        if let Some(tx) = s.borrow().as_ref() {
            let _ = tx.send((hwnd_raw, text, window_title));
        }
    });
}

/// Read text from a window/control via WM_GETTEXT message.
unsafe fn get_wm_text(hwnd: HWND) -> String {
    let mut buffer = [0u16; 4096];
    let len = SendMessageW(hwnd, WM_GETTEXT, WPARAM(buffer.len()), LPARAM(buffer.as_mut_ptr() as _));
    if len.0 > 0 {
        String::from_utf16_lossy(&buffer[..len.0 as usize])
    } else {
        String::new()
    }
}

// ─── UI Automation Helpers ──────────────────────────────────────────────────

fn capture_foreground_text() -> Option<(String, usize, String, String, Option<(i32, i32, i32, i32)>)> {
    unsafe {
        let hwnd = GetForegroundWindow();
        let hwnd_raw = hwnd.0 as usize;
        let (text, text_rect) = get_window_text_pattern_with_rect(hwnd)?;
        let window_title = get_window_title(hwnd);
        let process_name = get_process_name(hwnd);
        Some((text, hwnd_raw, window_title, process_name, text_rect))
    }
}

unsafe fn get_window_text_pattern_with_rect(hwnd: HWND) -> Option<(String, Option<(i32, i32, i32, i32)>)> {
    let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    if hr.is_err() { return None; }

    let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL).ok()?;
    let element = automation.ElementFromHandle(hwnd).ok()?;
    let pattern = element
        .GetCurrentPattern(UIA_TextPatternId)
        .ok()
        .and_then(|p| p.cast::<IUIAutomationTextPattern>().ok())?;
    let range = pattern.DocumentRange().ok()?;
    let text = range.GetText(-1).ok()?;
    let text_str = text.to_string();

    if text_str.is_empty() { return None; }

    // Get bounding rectangle of the UIA element (screen coordinates)
    let text_rect = element.CurrentBoundingRectangle().ok().map(|r| {
        (r.left, r.top, r.right - r.left, r.bottom - r.top)
    });

    Some((text_str, text_rect))
}

unsafe fn get_window_title(hwnd: HWND) -> String {
    let mut buffer = [0u16; 512];
    let len = GetWindowTextW(hwnd, &mut buffer);
    if len > 0 {
        String::from_utf16_lossy(&buffer[..len as usize])
    } else {
        String::new()
    }
}

unsafe fn get_process_name(hwnd: HWND) -> String {
    let mut process_id = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    if process_id == 0 { return String::new(); }

    if let Ok(handle) = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id) {
        let mut buffer = [0u16; 512];
        let len = GetModuleFileNameExW(handle, None, &mut buffer);
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        if len > 0 {
            let full_path = String::from_utf16_lossy(&buffer[..len as usize]);
            if let Some(name) = full_path.rsplit(['\\', '/']).next() {
                return name.to_string();
            }
        }
    }

    let mut class_name = [0u16; 256];
    let len = GetClassNameW(hwnd, &mut class_name);
    if len > 0 {
        String::from_utf16_lossy(&class_name[..len as usize])
    } else {
        format!("PID: {}", process_id)
    }
}
