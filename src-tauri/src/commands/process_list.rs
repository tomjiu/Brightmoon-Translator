use serde::{Deserialize, Serialize};
use tauri::command;

#[cfg(target_os = "windows")]
use windows::Win32::System::ProcessStatus::{
    EnumProcesses, GetModuleBaseNameW, K32GetModuleFileNameExW,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe_path: String,
}

/// Get list of all running processes with PID and name
#[command]
pub async fn get_process_list() -> Result<Vec<ProcessInfo>, String> {
    #[cfg(target_os = "windows")]
    {
        get_windows_process_list()
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Process list is only supported on Windows".to_string())
    }
}

#[cfg(target_os = "windows")]
fn get_windows_process_list() -> Result<Vec<ProcessInfo>, String> {
    use windows::Win32::Foundation::CloseHandle;

    let mut processes = Vec::new();
    let mut pids = vec![0u32; 4096];
    let mut bytes_returned = 0u32;

    // SAFETY: EnumProcesses writes into the pids buffer (sized 4096 entries)
    // and bytes_returned is a stack &mut. OpenProcess handles are null-checked
    // and closed via CloseHandle on every path.
    unsafe {
        // Get all process IDs
        let enum_result = EnumProcesses(
            pids.as_mut_ptr(),
            (pids.len() * std::mem::size_of::<u32>()) as u32,
            &raw mut bytes_returned,
        );

        if enum_result.is_err() {
            return Err("Failed to enumerate processes".to_string());
        }

        let process_count = (bytes_returned as usize) / std::mem::size_of::<u32>();
        pids.truncate(process_count);

        // Get process name and path for each PID
        for &pid in &pids {
            if pid == 0 {
                continue; // Skip system idle process
            }

            // Try to open process with query permissions
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid);

            if let Ok(handle) = handle {
                if handle.is_invalid() {
                    continue;
                }

                // Get process name
                let mut name_buffer = [0u16; 260];
                let name_len = GetModuleBaseNameW(handle, None, &mut name_buffer);

                let name = if name_len > 0 {
                    String::from_utf16_lossy(&name_buffer[..name_len as usize])
                } else {
                    format!("Process {pid}")
                };

                // Get exe path
                let mut path_buffer = [0u16; 260];
                let path_len = K32GetModuleFileNameExW(handle, None, &mut path_buffer);

                let exe_path = if path_len > 0 {
                    String::from_utf16_lossy(&path_buffer[..path_len as usize])
                } else {
                    String::new()
                };

                processes.push(ProcessInfo {
                    pid,
                    name,
                    exe_path,
                });

                let _ = CloseHandle(handle);
            }
        }
    }

    // Sort by name for better UX
    processes.sort_by_key(|a| a.name.to_lowercase());

    Ok(processes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_get_process_list() {
        let result = get_process_list().await;
        assert!(result.is_ok());

        let processes = result.unwrap();
        assert!(!processes.is_empty());

        // Should at least find the current process
        let current_pid = std::process::id();
        let found = processes.iter().any(|p| p.pid == current_pid);
        assert!(found, "Should find current process in list");
    }
}
