/**
 * H-Code / T-Code Text Hooking Commands
 *
 * Commands for managing DLL injection and reading captured text.
 * Host-side pump translates shared-memory messages without requiring the Hook UI.
 */
use crate::error::AppError;
use crate::hook_inject::{CapturedText, HookManager, HookStatus};
use crate::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

/// State wrapper for the hook manager + host pump lifecycle.
pub struct HookState {
    pub manager: Mutex<HookManager>,
    /// When true, background pump should exit.
    pump_stop: Arc<AtomicBool>,
    pump_running: AtomicBool,
}

impl HookState {
    pub fn new() -> Self {
        Self {
            manager: Mutex::new(HookManager::new()),
            pump_stop: Arc::new(AtomicBool::new(true)),
            pump_running: AtomicBool::new(false),
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

/// One pump / command tick: drain shared memory → TranslationService → emit.
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
        if text.is_empty() || text.chars().count() < 2 {
            continue;
        }
        if text.len() > 4000 {
            continue;
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
                        "engine": response.results.first().map(|r| r.engine.clone()).unwrap_or_else(|| "hook".into()),
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
            unsafe {
                let hwnd = GetForegroundWindow();
                let mut process_id = 0u32;
                GetWindowThreadProcessId(hwnd, Some(&mut process_id));
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

/// Preflight: is moon_hook.dll present on disk?
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

/// Read new text messages from the hooked process (no translate).
/// Prefer host pump + events when injected; this remains for diagnostics.
#[tauri::command]
pub async fn hook_read_messages(
    state: State<'_, HookState>,
) -> Result<Vec<CapturedText>, AppError> {
    let mut manager = state.manager.lock()?;
    Ok(manager.read_messages())
}

/// Read H-Code shared-memory messages and run them through TranslationService.
/// Host pump calls the same path automatically after inject.
#[tauri::command]
pub async fn hook_process_messages(
    hook_state: State<'_, HookState>,
    app_state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Vec<CapturedText>, AppError> {
    process_hook_messages_once(&hook_state, &app_state, &app).await
}
