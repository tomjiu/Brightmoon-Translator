use std::sync::Arc;
use std::io::Cursor;
use tokio::sync::{mpsc, Mutex};
use windows::core::Interface;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationTextPattern,
    UIA_TextPatternId,
};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
};

/// Monitored text from a window
#[derive(Debug, Clone)]
pub struct MonitoredText {
    pub window_title: String,
    pub process_name: String,
    pub text: String,
    pub timestamp: i64,
    /// Source: "uia", "clipboard", "ocr"
    pub source: String,
}

/// Hook monitor with three capture sources:
/// 1. UI Automation (TextPattern) — best for supported apps
/// 2. Clipboard — passive watch for copy events
/// 3. OCR — screenshot + WinRT OCR fallback for games/unsupported apps
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

        // Source 1: UI Automation
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

        if let Some((text, hwnd_raw, window_title, process_name)) = result {
            if text != last_text || hwnd_raw != last_hwnd {
                last_text = text.clone();
                last_hwnd = hwnd_raw;
                let _ = tx.send(MonitoredText {
                    window_title, process_name, text,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    source: "uia".to_string(),
                });
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}

// ─── Source 2: Clipboard ────────────────────────────────────────────────────

async fn clipboard_monitor_task(
    running: Arc<Mutex<bool>>,
    tx: mpsc::UnboundedSender<MonitoredText>,
) {
    let mut last_clip = String::new();

    loop {
        {
            let r = running.lock().await;
            if !*r { break; }
        }

        let clip_text = tokio::task::spawn_blocking(read_clipboard_text)
            .await.ok().flatten();

        if let Some(text) = clip_text {
            let trimmed = text.trim().to_string();
            if !trimmed.is_empty() && trimmed != last_clip {
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
                });
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }
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

/// OCR monitoring task: periodically screenshots the foreground window
/// and runs WinRT OCR to extract text.
/// Runs every 5 seconds to avoid excessive CPU usage.
async fn ocr_monitor_task(
    running: Arc<Mutex<bool>>,
    tx: mpsc::UnboundedSender<MonitoredText>,
) {
    let mut last_text = String::new();

    // Wait a bit before starting OCR (give UIA time to work first)
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    loop {
        {
            let r = running.lock().await;
            if !*r { break; }
        }

        // Capture foreground window rect and screenshot + OCR in blocking task
        let result = tokio::task::spawn_blocking(|| {
            use screenshots::image::ImageFormat;

            unsafe {
                let hwnd = GetForegroundWindow();
                let mut rect = windows::Win32::Foundation::RECT::default();
                if GetWindowRect(hwnd, &mut rect).is_err() {
                    return None;
                }

                let width = (rect.right - rect.left) as u32;
                let height = (rect.bottom - rect.top) as u32;
                if width < 100 || height < 100 {
                    return None;
                }

                let window_title = get_window_title(hwnd);
                let process_name = get_process_name(hwnd);

                // Capture the window area
                let img = crate::commands::capture::capture_area_gdi(
                    rect.left, rect.top, width, height,
                ).ok()?;

                // Convert to PNG bytes
                let mut buf = Cursor::new(Vec::new());
                img.write_to(&mut buf, ImageFormat::Png).ok()?;

                // Run WinRT OCR
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
                });
            }
        }

        // OCR every 5 seconds (expensive operation)
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

// ─── UI Automation Helpers ──────────────────────────────────────────────────

fn capture_foreground_text() -> Option<(String, usize, String, String)> {
    unsafe {
        let hwnd = GetForegroundWindow();
        let hwnd_raw = hwnd.0 as usize;
        let text = get_window_text_pattern(hwnd)?;
        let window_title = get_window_title(hwnd);
        let process_name = get_process_name(hwnd);
        Some((text, hwnd_raw, window_title, process_name))
    }
}

unsafe fn get_window_text_pattern(hwnd: HWND) -> Option<String> {
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

    if text_str.is_empty() { None } else { Some(text_str) }
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
    let len = windows::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, &mut class_name);
    if len > 0 {
        String::from_utf16_lossy(&class_name[..len as usize])
    } else {
        format!("PID: {}", process_id)
    }
}
