/**
 * Hook DLL Injection Manager
 *
 * Manages injection of the text hooking DLL into target processes.
 * Reads captured text from shared memory and dispatches to the translation pipeline.
 */

use serde::Serialize;
use windows::core::{PCWSTR, PSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Memory::{
    MapViewOfFile, MEMORY_MAPPED_VIEW_ADDRESS, OpenFileMappingW, UnmapViewOfFile,
    VirtualAllocEx, VirtualFreeEx, FILE_MAP_ALL_ACCESS, MEM_COMMIT, MEM_RELEASE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateRemoteThread, GetExitCodeThread, OpenProcess, WaitForSingleObject,
    PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ,
    PROCESS_VM_WRITE,
};

// Shared memory constants (must match DLL)
const SHARED_MEMORY_NAME: &str = "MoonTranslatorHookSharedMem";
const SHARED_MEMORY_SIZE: usize = 1024 * 1024; // 1MB
const SHARED_MEMORY_MAGIC: u32 = 0x4D4F4F4E; // "MOON"

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct SharedMemoryHeader {
    magic: u32,
    version: u32,
    write_offset: u32,
    buffer_size: u32,
    sequence: u32,
    process_name: [u8; 64],
    reserved: [u8; 40],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct TextMessage {
    length: u32,
    code_page: u32,
    x: i32,
    y: i32,
    timestamp: u64,
    // Followed by `length` bytes of UTF-8 text
}

/// Captured text from the hooked process
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturedText {
    pub text: String,
    pub code_page: u32,
    pub x: i32,
    pub y: i32,
    pub timestamp: u64,
}

/// Status of the hook injection
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookStatus {
    pub injected: bool,
    pub pid: u32,
    pub process_name: String,
    pub messages_read: u64,
}

/// Manages DLL injection and shared memory reading
pub struct HookManager {
    shared_mem: HANDLE,
    shared_view: MEMORY_MAPPED_VIEW_ADDRESS,
    shared_data: *mut SharedMemoryHeader,
    target_pid: u32,
    injected: bool,
    messages_read: u64,
    last_sequence: u32,
}

unsafe impl Send for HookManager {}
unsafe impl Sync for HookManager {}

impl HookManager {
    pub fn new() -> Self {
        Self {
            shared_mem: HANDLE::default(),
            shared_view: MEMORY_MAPPED_VIEW_ADDRESS::default(),
            shared_data: std::ptr::null_mut(),
            target_pid: 0,
            injected: false,
            messages_read: 0,
            last_sequence: 0,
        }
    }

    /// Inject the hook DLL into the specified process
    pub fn inject(&mut self, pid: u32) -> Result<(), String> {
        if self.injected {
            return Err("Already injected".to_string());
        }

        // Find the DLL path
        let dll_path = self.find_hook_dll()?;
        let dll_path_wide = to_wide(&dll_path);

        unsafe {
            // Open target process
            let process = OpenProcess(
                PROCESS_CREATE_THREAD
                    | PROCESS_QUERY_INFORMATION
                    | PROCESS_VM_OPERATION
                    | PROCESS_VM_READ
                    | PROCESS_VM_WRITE,
                false,
                pid,
            )
            .map_err(|e| format!("OpenProcess failed: {}", e))?;

            // Allocate memory in target process for DLL path
            let path_size = (dll_path_wide.len() * 2) as usize;
            let remote_mem = VirtualAllocEx(
                process,
                None,
                path_size,
                MEM_COMMIT,
                PAGE_READWRITE,
            );
            if remote_mem.is_null() {
                let _ = CloseHandle(process);
                return Err("VirtualAllocEx failed".to_string());
            }

            // Write DLL path to target process
            let mut written = 0usize;
            if WriteProcessMemory(
                process,
                remote_mem,
                dll_path_wide.as_ptr() as *const _,
                path_size,
                Some(&mut written),
            )
            .is_err()
            {
                let _ = VirtualFreeEx(process, remote_mem, 0, MEM_RELEASE);
                let _ = CloseHandle(process);
                return Err("WriteProcessMemory failed".to_string());
            }

            // Get LoadLibraryW address
            let kernel32 = GetModuleHandleW(PCWSTR(to_wide("kernel32.dll").as_ptr()))
                .map_err(|e| format!("GetModuleHandleW failed: {}", e))?;
            let load_library = GetProcAddress(kernel32, PSTR(b"LoadLibraryW\0".as_ptr() as *mut _))
                .ok_or("GetProcAddress LoadLibraryW failed")?;

            // Create remote thread to load DLL
            let thread = CreateRemoteThread(
                process,
                None,
                0,
                Some(std::mem::transmute(load_library)),
                Some(remote_mem),
                0,
                None,
            )
            .map_err(|e| format!("CreateRemoteThread failed: {}", e))?;

            // Wait for DLL to load
            let _ = WaitForSingleObject(thread, 5000);

            // Get the thread exit code (the DLL module handle)
            let mut exit_code = 0u32;
            let _ = GetExitCodeThread(thread, &mut exit_code);

            // Cleanup
            let _ = VirtualFreeEx(process, remote_mem, 0, MEM_RELEASE);
            let _ = CloseHandle(thread);
            let _ = CloseHandle(process);

            if exit_code == 0 {
                return Err("DLL injection failed (LoadLibraryW returned NULL)".to_string());
            }

            // Open shared memory
            self.open_shared_memory()?;
            self.target_pid = pid;
            self.injected = true;
            self.messages_read = 0;
            self.last_sequence = 0;

            tracing::info!("[HookManager] Injected into PID {} ({})", pid, dll_path);
            Ok(())
        }
    }

    /// Eject the DLL (cleanup shared memory)
    pub fn eject(&mut self) -> Result<(), String> {
        if !self.injected {
            return Ok(());
        }

        self.cleanup_shared_memory();
        self.injected = false;
        self.target_pid = 0;

        tracing::info!("[HookManager] Ejected");
        Ok(())
    }

    /// Read new text messages from shared memory
    pub fn read_messages(&mut self) -> Vec<CapturedText> {
        if !self.injected || self.shared_data.is_null() {
            return Vec::new();
        }

        let mut messages = Vec::new();

        unsafe {
            let header = &*self.shared_data;

            // Check if new data is available
            if header.magic != SHARED_MEMORY_MAGIC {
                return messages;
            }

            // Simple sequence-based check
            if header.sequence == self.last_sequence {
                return messages;
            }

            // Read messages from shared memory
            let buffer = self.shared_data as *const u8;
            let mut offset = std::mem::size_of::<SharedMemoryHeader>();

            while offset + std::mem::size_of::<TextMessage>() < header.write_offset as usize {
                let msg = &*(buffer.add(offset) as *const TextMessage);
                if msg.length == 0 || msg.length > 65536 {
                    break; // Invalid message
                }

                let text_offset = offset + std::mem::size_of::<TextMessage>();
                if text_offset + msg.length as usize > header.buffer_size as usize {
                    break; // Out of bounds
                }

                let text_bytes =
                    std::slice::from_raw_parts(buffer.add(text_offset), msg.length as usize);

                if let Ok(text) = std::str::from_utf8(text_bytes) {
                    messages.push(CapturedText {
                        text: text.to_string(),
                        code_page: msg.code_page,
                        x: msg.x,
                        y: msg.y,
                        timestamp: msg.timestamp,
                    });
                }

                offset = text_offset + msg.length as usize;
            }

            self.last_sequence = header.sequence;
            self.messages_read += messages.len() as u64;
        }

        messages
    }

    /// Get current status
    pub fn status(&self) -> HookStatus {
        let process_name = if self.injected && !self.shared_data.is_null() {
            unsafe {
                let header = &*self.shared_data;
                let name_bytes = &header.process_name;
                let len = name_bytes.iter().position(|&b| b == 0).unwrap_or(64);
                std::str::from_utf8(&name_bytes[..len])
                    .unwrap_or("unknown")
                    .to_string()
            }
        } else {
            String::new()
        };

        HookStatus {
            injected: self.injected,
            pid: self.target_pid,
            process_name,
            messages_read: self.messages_read,
        }
    }

    // --- Internal helpers ---

    fn find_hook_dll(&self) -> Result<String, String> {
        // Look for moon_hook.dll in several locations
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_default();

        let candidates = vec![
            exe_dir.join("moon_hook.dll"),
            exe_dir.join("..\\..\\src-tauri\\bin\\moon_hook.dll"),
            std::path::PathBuf::from("src-tauri\\bin\\moon_hook.dll"),
        ];

        for candidate in &candidates {
            if candidate.exists() {
                return candidate
                    .to_str()
                    .ok_or("Invalid path".to_string())
                    .map(|s| s.to_string());
            }
        }

        Err(format!(
            "moon_hook.dll not found. Searched: {:?}",
            candidates
        ))
    }

    fn open_shared_memory(&mut self) -> Result<(), String> {
        unsafe {
            let name_wide = to_wide(SHARED_MEMORY_NAME);
            let handle = OpenFileMappingW(
                FILE_MAP_ALL_ACCESS.0 as u32,
                false,
                PCWSTR(name_wide.as_ptr()),
            )
            .map_err(|e| format!("OpenFileMappingW failed: {}", e))?;

            let view = MapViewOfFile(
                handle,
                FILE_MAP_ALL_ACCESS,
                0,
                0,
                SHARED_MEMORY_SIZE,
            );

            if view.Value.is_null() {
                let _ = CloseHandle(handle);
                return Err("MapViewOfFile failed".to_string());
            }

            self.shared_mem = handle;
            self.shared_view = view;
            self.shared_data = view.Value as *mut SharedMemoryHeader;

            // Verify magic
            if (*self.shared_data).magic != SHARED_MEMORY_MAGIC {
                self.cleanup_shared_memory();
                return Err("Invalid shared memory magic".to_string());
            }

            Ok(())
        }
    }

    fn cleanup_shared_memory(&mut self) {
        unsafe {
            if !self.shared_data.is_null() {
                let _ = UnmapViewOfFile(self.shared_view);
                self.shared_data = std::ptr::null_mut();
                self.shared_view = MEMORY_MAPPED_VIEW_ADDRESS::default();
            }
            if !self.shared_mem.is_invalid() {
                let _ = CloseHandle(self.shared_mem);
                self.shared_mem = HANDLE::default();
            }
        }
    }
}

impl Drop for HookManager {
    fn drop(&mut self) {
        self.cleanup_shared_memory();
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
