/**
 * H-Code / T-Code Text Hooking Commands
 *
 * Commands for managing DLL injection and reading captured text.
 * Host-side pump translates shared-memory messages without requiring the Hook UI.
 */
use crate::error::AppError;
use crate::hook_code::{parse_h_code, HookCode};
use crate::hook_inject::{CapturedText, HookInstallResult, HookManager, HookStats, HookStatus};
use crate::selection::hover_pick::is_ui_chrome_word;
use crate::AppState;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

#[cfg(target_os = "windows")]
extern "system" {
    fn GetACP() -> u32;
}

/// Recent hook captures for dedup (text + coarse time window).
struct HookDedup {
    recent: VecDeque<(String, Instant)>,
}

impl HookDedup {
    fn new() -> Self {
        Self {
            recent: VecDeque::with_capacity(32),
        }
    }

    /// Returns true if this text was seen recently (skip translate).
    fn is_dup(&mut self, text: &str) -> bool {
        let now = Instant::now();
        let key = text.trim().to_string();
        // Drop older than 8s
        while let Some((_, t)) = self.recent.front() {
            if now.duration_since(*t) > Duration::from_secs(8) {
                self.recent.pop_front();
            } else {
                break;
            }
        }
        if self.recent.iter().any(|(s, _)| s == &key) {
            return true;
        }
        self.recent.push_back((key, now));
        if self.recent.len() > 40 {
            self.recent.pop_front();
        }
        false
    }
}

fn hook_text_is_noise(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() || t.chars().count() < 2 {
        return true;
    }
    if t.len() > 4000 {
        return true;
    }
    // Pure digits / punctuation / single glyph spam
    if t.chars().all(|c| c.is_ascii_digit() || c.is_whitespace()) {
        return true;
    }
    if t.chars().count() == 1 {
        return true;
    }
    if is_ui_chrome_word(t) {
        return true;
    }
    // Common GDI chrome fragments
    let lower = t.to_ascii_lowercase();
    if lower == "ok"
        || lower == "cancel"
        || lower == "yes"
        || lower == "no"
        || lower == "file"
        || lower == "edit"
        || lower == "view"
        || lower == "help"
        || ((lower.starts_with("http://") || lower.starts_with("https://"))
            && t.chars().count() < 16)
    {
        return true;
    }
    false
}

/// State wrapper for the hook manager + host pump lifecycle.
pub struct HookState {
    pub manager: Mutex<HookManager>,
    /// When true, background pump should exit.
    pump_stop: Arc<AtomicBool>,
    pump_running: AtomicBool,
    dedup: Mutex<HookDedup>,
}

impl Default for HookState {
    fn default() -> Self {
        Self::new()
    }
}

impl HookState {
    pub fn new() -> Self {
        Self {
            manager: Mutex::new(HookManager::new()),
            pump_stop: Arc::new(AtomicBool::new(true)),
            pump_running: AtomicBool::new(false),
            dedup: Mutex::new(HookDedup::new()),
        }
    }

    fn start_pump(&self, app: AppHandle) {
        self.pump_stop.store(false, Ordering::SeqCst);
        if self
            .pump_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let stop = Arc::clone(&self.pump_stop);
        tauri::async_runtime::spawn(async move {
            tracing::info!("[HookInject] host pump started");
            while !stop.load(Ordering::SeqCst) {
                if let Some(hook_state) = app.try_state::<HookState>() {
                    if let Some(app_state) = app.try_state::<AppState>() {
                        if let Err(e) =
                            process_hook_messages_once(&hook_state, &app_state, &app).await
                        {
                            tracing::debug!("[HookInject] pump tick: {e}");
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(400)).await;
            }
            if let Some(hook_state) = app.try_state::<HookState>() {
                hook_state.pump_running.store(false, Ordering::SeqCst);
            }
            tracing::info!("[HookInject] host pump stopped");
        });
    }

    fn stop_pump(&self) {
        self.pump_stop.store(true, Ordering::SeqCst);
    }
}

/// One pump / command tick: drain shared memory → `TranslationService` → emit.
async fn process_hook_messages_once(
    hook_state: &HookState,
    app_state: &AppState,
    app: &AppHandle,
) -> Result<Vec<CapturedText>, AppError> {
    let (messages, process_name, pid) = {
        let mut manager = hook_state.manager.lock()?;
        if !manager.status().injected {
            return Ok(Vec::new());
        }
        let msgs = manager.read_messages();
        let st = manager.status();
        (msgs, st.process_name, st.pid)
    };

    if messages.is_empty() {
        return Ok(messages);
    }

    // Raw capture event so UI can list without competing for read_messages
    let _ = app.emit(
        "hook-text-captured",
        serde_json::json!({
            "pid": pid,
            "process_name": process_name,
            "messages": messages,
        }),
    );

    let (from, to) = {
        let config = app_state.system.config.lock().await;
        (config.default_from.clone(), config.default_to.clone())
    };

    for msg in &messages {
        let text = msg.text.trim();
        if hook_text_is_noise(text) {
            continue;
        }
        {
            let mut dedup = hook_state.dedup.lock()?;
            if dedup.is_dup(text) {
                continue;
            }
        }

        match app_state
            .translation
            .service
            .run_full(
                crate::models::translation::TranslateChannel::Hook,
                text,
                &from,
                &to,
            )
            .await
        {
            Ok(response) => {
                let results_json: Vec<_> = response
                    .results
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "engine": r.engine,
                            "text": r.text,
                            "latencyMs": r.latency_ms,
                        })
                    })
                    .collect();
                let _ = app.emit(
                    "hook-text-translated",
                    serde_json::json!({
                        "window_title": format!("H-Code PID {pid}"),
                        "process_name": process_name,
                        "original": text,
                        "translated": response.results.first().map(|r| r.text.clone()).unwrap_or_default(),
                        "engine": response.results.first().map_or_else(|| "hook".into(), |r| r.engine.clone()),
                        "timestamp": msg.timestamp as i64,
                        "source": "hook",
                        "text_rect": if msg.x != 0 || msg.y != 0 {
                            Some([msg.x, msg.y, 0, 0])
                        } else {
                            None
                        },
                        "results": results_json,
                    }),
                );
            },
            Err(e) => {
                tracing::warn!("[HookInject] translate failed: {e}");
            },
        }
    }

    Ok(messages)
}

/// Inject the hook DLL into the specified process.
/// If pid is 0, uses the foreground window's process.
#[tauri::command]
pub async fn hook_inject(
    app: AppHandle,
    state: State<'_, HookState>,
    pid: u32,
) -> Result<HookStatus, AppError> {
    let target_pid = if pid == 0 {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
            use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
            // SAFETY: GetForegroundWindow returns an HWND (or null, in which
            // case GetWindowThreadProcessId returns 0). process_id is a stack
            // &mut u32. Both are pure Win32 queries with no preconditions.
            unsafe {
                let hwnd = GetForegroundWindow();
                let mut process_id = 0u32;
                GetWindowThreadProcessId(hwnd, Some(&raw mut process_id));
                process_id
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            return Err(AppError::PlatformNotSupported);
        }
    } else {
        pid
    };

    if target_pid == 0 {
        return Err(AppError::Hook("No target process found".to_string()));
    }

    let status = {
        let mut manager = state.manager.lock()?;
        manager
            .inject(target_pid)
            .map_err(AppError::HookInjection)?;
        manager.status()
    };
    // Host pump: translate without Hook UI open
    state.start_pump(app);
    Ok(status)
}

/// Eject the hook DLL and cleanup.
#[tauri::command]
pub async fn hook_eject(state: State<'_, HookState>) -> Result<HookStatus, AppError> {
    state.stop_pump();
    // Brief wait so in-flight tick can release the mutex
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut manager = state.manager.lock()?;
    manager.eject().map_err(AppError::Hook)?;
    Ok(manager.status())
}

/// Get the current hook status.
#[tauri::command]
pub async fn hook_status(state: State<'_, HookState>) -> Result<HookStatus, AppError> {
    let manager = state.manager.lock()?;
    Ok(manager.status())
}

/// Preflight: is `moon_hook.dll` present on disk?
#[tauri::command]
pub async fn hook_dll_available(state: State<'_, HookState>) -> Result<bool, AppError> {
    let manager = state.manager.lock()?;
    Ok(manager.dll_available())
}

/// Preflight: resolved DLL path if any (for diagnostics).
#[tauri::command]
pub async fn hook_dll_path(state: State<'_, HookState>) -> Result<Option<String>, AppError> {
    let manager = state.manager.lock()?;
    Ok(manager.dll_path())
}

/// Query hook statistics from the injected DLL (IAT hits, late-loaded patches,
/// `send_text` counters, inline hooks installed).
///
/// Calls the remote `HookGetStats` export via `CreateRemoteThread` +
/// `ReadProcessMemory`, parses the returned JSON into [`HookStats`].
/// Returns `HookStats::default()` when not injected (no-op).
#[tauri::command]
pub async fn hook_get_stats(state: State<'_, HookState>) -> Result<HookStats, AppError> {
    let manager = state.manager.lock()?;
    manager
        .get_stats()
        .map_err(|e| AppError::Hook(format!("get_stats failed: {e}")))
}

/// Parse an H-Code string and install an inline hook in the remote process.
///
/// Accepts a Luna Hook H-Code (e.g. `/HW-4@12345:game.exe`) and an optional
/// ANSI code page override. When the code page is omitted, the system ANSI
/// code page (`GetACP()`) is used — 932 for Japanese, 936 for Simplified
/// Chinese, etc.
///
/// Returns the resolved hook address and exit code from the DLL.
#[tauri::command]
pub async fn hook_install_h_code(
    state: State<'_, HookState>,
    h_code: String,
    ansi_code_page: Option<u32>,
) -> Result<HookInstallResult, AppError> {
    let code: HookCode = parse_h_code(&h_code)
        .map_err(|e| AppError::Hook(format!("invalid H-Code '{h_code}': {e}")))?;

    // Default to the system ANSI code page if not specified.
    let default_cp = ansi_code_page.unwrap_or_else(|| {
        #[cfg(target_os = "windows")]
        {
            // SAFETY: GetACP is a pure kernel32 query with no preconditions.
            unsafe { GetACP() }
        }
        #[cfg(not(target_os = "windows"))]
        {
            932 // Shift-JIS default on non-Windows (dev/test only)
        }
    });

    let manager = state.manager.lock()?;
    manager
        .install_h_code(&code, default_cp)
        .map_err(|e| AppError::Hook(format!("install_h_code failed: {e}")))
}
