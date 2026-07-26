/**
 * H-Code / T-Code Text Hooking Commands
 *
 * Commands for managing DLL injection and reading captured text.
 */
use crate::error::AppError;
use crate::hook_inject::{CapturedText, HookManager, HookStatus};
use std::sync::Mutex;
use tauri::State;

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
