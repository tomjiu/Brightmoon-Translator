/**
 * H-Code / T-Code Text Hooking Commands
 *
 * Commands for managing DLL injection and reading captured text.
 */
use crate::error::AppError;
use crate::hook_inject::{CapturedText, HookManager, HookStatus};
use crate::AppState;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

/// State wrapper for the hook manager
pub struct HookState {
    pub manager: Mutex<HookManager>,
}

impl HookState {
    pub fn new() -> Self {
        Self {
            manager: Mutex::new(HookManager::new()),
        }
    }
}

/// Inject the hook DLL into the specified process.
/// If pid is 0, uses the foreground window's process.
#[tauri::command]
pub async fn hook_inject(state: State<'_, HookState>, pid: u32) -> Result<HookStatus, AppError> {
    let target_pid = if pid == 0 {
        // Get foreground window's process ID
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
            use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
            // SAFETY: GetForegroundWindow and GetWindowThreadProcessId are standard Win32 APIs.
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

    let mut manager = state.manager.lock()?;
    manager
        .inject(target_pid)
        .map_err(AppError::HookInjection)?;
    Ok(manager.status())
}

/// Eject the hook DLL and cleanup.
#[tauri::command]
pub async fn hook_eject(state: State<'_, HookState>) -> Result<HookStatus, AppError> {
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

/// Read new text messages from the hooked process.
#[tauri::command]
pub async fn hook_read_messages(
    state: State<'_, HookState>,
) -> Result<Vec<CapturedText>, AppError> {
    let mut manager = state.manager.lock()?;
    Ok(manager.read_messages())
}

/// Read H-Code shared-memory messages and run them through TranslationService.
/// Emits the same `hook-text-translated` event as the passive UIA/clipboard monitor
/// so the Hook UI and overlay share one pipeline (MODULE_MAP gap: inject → translate).
#[tauri::command]
pub async fn hook_process_messages(
    hook_state: State<'_, HookState>,
    app_state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Vec<CapturedText>, AppError> {
    let (messages, process_name, pid) = {
        let mut manager = hook_state.manager.lock()?;
        let msgs = manager.read_messages();
        let st = manager.status();
        (msgs, st.process_name, st.pid)
    };

    if messages.is_empty() {
        return Ok(messages);
    }

    let (from, to) = {
        let config = app_state.system.config.lock().await;
        (config.default_from.clone(), config.default_to.clone())
    };

    for msg in &messages {
        let text = msg.text.trim();
        if text.is_empty() || text.chars().count() < 2 {
            continue;
        }
        // Skip obvious noise / pure digits / single glyphs
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
