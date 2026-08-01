/**
 * Hook DLL Injection Manager
 *
 * Manages injection of the text hooking DLL into target processes.
 * Reads captured text from shared memory and dispatches to the translation pipeline.
 */
use serde::Serialize;
use windows::core::{PCWSTR, PSTR};
use windows::Win32::Foundation::{CloseHandle, FreeLibrary, HANDLE, HMODULE};
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
// S5-5: GetModuleInformation + GetModuleFileNameExW + filesystem stat used
// to verify local and remote hook DLL are the same build before computing
// RVA — if the DLL was rebuilt after injection, the RVA would point to the
// wrong offset in the remote process and CreateRemoteThread would jump into
// arbitrary code. Three signals are checked: SizeOfImage, file path, file size.
use windows::Win32::System::ProcessStatus::{GetModuleInformation, GetModuleFileNameExW, MODULEINFO};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::System::Memory::{
    MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, VirtualAllocEx, VirtualFreeEx,
    FILE_MAP_ALL_ACCESS, MEMORY_MAPPED_VIEW_ADDRESS, MEM_COMMIT, MEM_RELEASE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateRemoteThread, GetCurrentProcess, GetExitCodeThread, OpenProcess, WaitForSingleObject,
    PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ,
    PROCESS_VM_WRITE,
};

// Shared memory constants (must match DLL; name is PID-scoped)
const SHARED_MEMORY_SIZE: usize = 1024 * 1024; // 1MB
const SHARED_MEMORY_MAGIC: u32 = 0x4D4F4F4E; // "MOON"

/// 4a-marker: reserved code_page sentinel used by the DLL's Ldr callback to
/// report late-loaded module basenames through shared memory without
/// polluting the captured-text stream. Must match
/// `LATE_LOADED_MARKER_CODE_PAGE` in hook_text.cpp.
const LATE_LOADED_MARKER_CODE_PAGE: u32 = 0xFFFF_FFFF;

fn shared_memory_name(pid: u32) -> String {
    format!("MoonTranslatorHookSharedMem_PID{}", pid)
}

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

/// 4b: HookStats — host mirror of the DLL's HookStats struct.
///
/// Populated by `HookManager::get_stats()`, which calls the remote
/// `HookGetStats` export via CreateRemoteThread + ReadProcessMemory.
/// The DLL returns a static JSON string; we parse it into this struct
/// so the host/UI can consume typed fields.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookStats {
    /// Modules walked by PatchAllModulesIAT (initial sweep + late-loaded).
    pub modules_scanned: u64,
    /// IAT slots successfully patched.
    pub iat_hits: u64,
    /// Modules patched via LdrRegisterDllNotification callback.
    pub late_loaded_patched: u64,
    /// SendTextToHost invocations (before filter).
    pub send_text_calls: u64,
    /// Calls rejected by IsPrintableText (noise/short).
    pub send_text_filtered: u64,
    /// Calls blocked during eject (g_ejecting was true).
    pub send_text_eject_blocked: u64,
    /// H-Code inline hooks installed.
    pub inline_hooks: u64,
    /// Whether IAT hooks are currently installed.
    pub hooks_installed: bool,
    /// Whether LdrRegisterDllNotification cookie is held.
    pub ldr_cookie: bool,
}

/// 4d-2: HookInstallParams — must match `HookInstallParams` in
/// hook_text.cpp (HookInstallAtAddressStruct export). Used to pass the
/// 5 hook parameters through CreateRemoteThread's single lpParameter.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HookInstallParams {
    /// Absolute virtual address in the target process to hook.
    pub target_addr: usize,
    /// Byte offset (positive=after addr, negative=before addr).
    pub data_offset: i32,
    /// 0=read bytes directly, 1+=pointer dereference chain.
    pub deref_levels: u32,
    /// 0=UTF-16, 932=Shift-JIS, 936=GBK, ...
    pub code_page: u32,
    /// 0=null-terminated, 1=length-prefixed.
    pub text_type: u32,
}

// Compile-time layout assertion: the DLL uses `#pragma pack(8)`, which on
// x64 gives the same layout as Rust's default `#[repr(C)]` for this struct
// (all fields naturally aligned to ≤8 bytes). If either side changes,
// this assertion will fail at compile time on the host.
const _: () = assert!(std::mem::size_of::<HookInstallParams>() == 24);

/// Result of an H-Code inline hook installation attempt.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookInstallResult {
    /// 0 = success, 1 = invalid params, 2 = VirtualProtect fail,
    /// 3 = trampoline alloc fail, 4 = unsupported arch.
    pub exit_code: u32,
    /// Absolute address that was hooked (for UI display).
    pub resolved_addr: u64,
    /// True if exit_code == 0.
    pub success: bool,
    /// Human-readable message.
    pub message: String,
}

/// Manages DLL injection and shared memory reading
pub struct HookManager {
    shared_mem: HANDLE,
    shared_view: MEMORY_MAPPED_VIEW_ADDRESS,
    shared_data: *mut SharedMemoryHeader,
    target_pid: u32,
    /// Remote HMODULE from LoadLibraryW exit code (for HookUninstall)
    remote_module: usize,
    injected: bool,
    messages_read: u64,
    last_sequence: u32,
}

// SAFETY: HookManager contains raw pointers (shared_data) and Win32 handles.
// - shared_data points to a memory-mapped file that is valid for the process lifetime
// - The pointer is only accessed through methods that check for null
// - Win32 handles are valid until cleanup_shared_memory() is called
unsafe impl Send for HookManager {}
// SAFETY: All access to shared_data is through methods with proper synchronization.
// - read_messages() takes &mut self, ensuring exclusive access for writes
// - status() takes &self and only reads immutable header fields
unsafe impl Sync for HookManager {}

impl HookManager {
    pub fn new() -> Self {
        Self {
            shared_mem: HANDLE::default(),
            shared_view: MEMORY_MAPPED_VIEW_ADDRESS::default(),
            shared_data: std::ptr::null_mut(),
            target_pid: 0,
            remote_module: 0,
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

        // SAFETY: DLL injection via CreateRemoteThread + LoadLibraryW.
        // Standard Windows DLL injection technique. All handles are properly
        // cleaned up on both success and failure paths.
        // SAFETY: Reading from shared memory mapped file.
        // - self.shared_data is non-null (checked above)
        // - Magic number is validated before reading
        // - Message length is validated (0 < length <= 65536)
        // - Buffer bounds are checked before reading text
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
            let remote_mem = VirtualAllocEx(process, None, path_size, MEM_COMMIT, PAGE_READWRITE);
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

            self.remote_module = exit_code as usize;
            self.target_pid = pid;

            // Open PID-scoped shared memory (DLL creates on attach)
            // Brief retry: DLL may still be initializing mapping
            let mut last_err = String::new();
            for _ in 0..20 {
                match self.open_shared_memory(pid) {
                    Ok(()) => {
                        last_err.clear();
                        break;
                    }
                    Err(e) => {
                        last_err = e;
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
            }
            if !last_err.is_empty() {
                self.remote_module = 0;
                self.target_pid = 0;
                return Err(last_err);
            }

            self.injected = true;
            self.messages_read = 0;
            self.last_sequence = 0;

            tracing::info!("[HookManager] Injected into PID {} ({})", pid, dll_path);
            Ok(())
        }
    }

    /// Eject the DLL: remote HookUninstall (IAT restore + FreeLibrary) then unmap local view
    pub fn eject(&mut self) -> Result<(), String> {
        if !self.injected {
            return Ok(());
        }

        let pid = self.target_pid;
        let remote_module = self.remote_module;

        if pid != 0 && remote_module != 0 {
            if let Err(e) = self.remote_uninstall(pid, remote_module) {
                tracing::warn!("[HookManager] remote HookUninstall failed: {}", e);
            }
        }

        self.cleanup_shared_memory();
        self.injected = false;
        self.target_pid = 0;
        self.remote_module = 0;
        self.last_sequence = 0;

        tracing::info!("[HookManager] Ejected");
        Ok(())
    }

    /// CreateRemoteThread(HookUninstall) in target process.
    fn remote_uninstall(&self, pid: u32, remote_module: usize) -> Result<(), String> {
        unsafe {
            let process = OpenProcess(
                PROCESS_CREATE_THREAD
                    | PROCESS_QUERY_INFORMATION
                    | PROCESS_VM_OPERATION
                    | PROCESS_VM_READ
                    | PROCESS_VM_WRITE,
                false,
                pid,
            )
            .map_err(|e| format!("OpenProcess for eject failed: {}", e))?;

            // Resolve HookUninstall in *local* moon_hook.dll, then rebase to remote HMODULE.
            // Same image layout → RVA is stable across load addresses.
            let local_dll = self.find_hook_dll()?;
            let local_mod =
                LoadLibraryW(PCWSTR(to_wide(&local_dll).as_ptr()))
                    .map_err(|e| format!("LoadLibraryW local hook dll failed: {}", e))?;

            let local_proc = GetProcAddress(local_mod, PSTR(b"HookUninstall\0".as_ptr() as *mut _))
                .ok_or_else(|| {
                    let _ = FreeLibrary(local_mod);
                    "GetProcAddress HookUninstall failed".to_string()
                })?;

            // S5-5: verify local and remote hook DLL are the same build
            // before computing RVA. Three signals are checked:
            //   1. SizeOfImage (cheap, catches most rebuilds)
            //   2. File path (catches same-size different-build edge cases)
            //   3. File size on disk (catches same-path different-content)
            // If any mismatch → refuse RVA rebase (avoids jumping into
            // arbitrary code in the remote process).
            let mut local_info = MODULEINFO::default();
            let local_process = GetCurrentProcess();
            if let Err(e) = GetModuleInformation(
                local_process,
                local_mod,
                &mut local_info,
                std::mem::size_of::<MODULEINFO>() as u32,
            ) {
                let _ = FreeLibrary(local_mod);
                let _ = CloseHandle(process);
                return Err(format!("GetModuleInformation local failed: {}", e));
            }

            let mut remote_info = MODULEINFO::default();
            if let Err(e) = GetModuleInformation(
                process,
                HMODULE(remote_module as *mut core::ffi::c_void),
                &mut remote_info,
                std::mem::size_of::<MODULEINFO>() as u32,
            ) {
                let _ = FreeLibrary(local_mod);
                let _ = CloseHandle(process);
                return Err(format!("GetModuleInformation remote failed: {}", e));
            }

            // Get full file path of local and remote module via GetModuleFileNameExW.
            let local_path = module_file_path(local_process, local_mod);
            let remote_path = module_file_path(
                process,
                HMODULE(remote_module as *mut core::ffi::c_void),
            );
            // Stat the DLL file on disk for a content-level size check.
            let local_file_size = local_path
                .as_ref()
                .and_then(|p| std::fs::metadata(p).ok())
                .map(|m| m.len());
            let remote_file_size = remote_path
                .as_ref()
                .and_then(|p| std::fs::metadata(p).ok())
                .map(|m| m.len());

            if let Err(e) = compare_dll_metadata(
                local_path.as_deref(),
                local_file_size,
                local_info.SizeOfImage,
                remote_path.as_deref(),
                remote_file_size,
                remote_info.SizeOfImage,
            ) {
                let _ = FreeLibrary(local_mod);
                let _ = CloseHandle(process);
                return Err(e);
            }

            let local_base = local_mod.0 as usize;
            let rva = (local_proc as usize).wrapping_sub(local_base);
            let remote_fn = remote_module.wrapping_add(rva);

            let _ = FreeLibrary(local_mod);

            let thread = CreateRemoteThread(
                process,
                None,
                0,
                Some(std::mem::transmute(remote_fn)),
                None,
                0,
                None,
            )
            .map_err(|e| {
                let _ = CloseHandle(process);
                format!("CreateRemoteThread HookUninstall failed: {}", e)
            })?;

            let _ = WaitForSingleObject(thread, 5000);

            // 4c: read the HookUninstall exit code so the host can detect a
            // dirty rollback. The DLL returns:
            //   0 = IAT rollback verified clean (all slots restored)
            //   1 = dangling IAT slots remain (host should consider killing the
            //       target to avoid a crash on the next hooked call)
            //   2 = cleanup ran but verify was skipped (already uninstalled)
            // We surface dirty rollbacks as a warning; the caller (eject) keeps
            // the Ok contract because the DLL is freed regardless.
            let mut exit_code: u32 = 0;
            let _ = GetExitCodeThread(thread, &mut exit_code);
            if exit_code == 1 {
                tracing::warn!(
                    "[HookManager] HookUninstall reported dangling IAT slots (exit_code=1) \
                     in PID {} — target process may crash on next hooked call; consider \
                     force-killing it",
                    pid
                );
            } else if exit_code == 0 {
                tracing::info!("[HookManager] HookUninstall clean (exit_code=0) for PID {}", pid);
            }

            let _ = CloseHandle(thread);
            let _ = CloseHandle(process);
            Ok(())
        }
    }

    /// Read new text messages from shared memory
    pub fn read_messages(&mut self) -> Vec<CapturedText> {
        if !self.injected || self.shared_data.is_null() {
            return Vec::new();
        }

        let mut messages = Vec::new();

        // SAFETY: Reading from shared memory mapped file.
        // - self.shared_data is non-null (checked above)
        // - Magic number is validated before reading
        // - Message length is validated (0 < length <= 65536)
        // - Buffer bounds are checked before reading text
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
                    // 4a-marker: the DLL sends late-loaded DLL basenames through
                    // shared memory with a reserved code_page sentinel so the
                    // host can route them to a debug channel instead of treating
                    // them as captured text (which would pollute the translation
                    // pipeline with filenames like "d3d9.dll").
                    if msg.code_page == LATE_LOADED_MARKER_CODE_PAGE {
                        tracing::debug!(
                            "[hook] late-loaded module patched by Ldr callback: {}",
                            text
                        );
                    } else {
                        messages.push(CapturedText {
                            text: text.to_string(),
                            code_page: msg.code_page,
                            x: msg.x,
                            y: msg.y,
                            timestamp: msg.timestamp,
                        });
                    }
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
            // SAFETY: Reading process_name from shared memory header.
            // - self.shared_data is non-null (checked above)
            // - Header was validated by magic number in open_shared_memory()
            // - process_name is a fixed-size array [u8; 64], always valid to read
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

    /// 4b: Query the remote `HookGetStats` export and parse the returned
    /// JSON into a typed [`HookStats`].
    ///
    /// Flow:
    /// 1. Resolve `HookGetStats` in *local* moon_hook.dll, compute RVA.
    /// 2. Rebase to remote module handle (validated by `compare_dll_metadata`
    ///    so we don't jump into the wrong code if the DLL was rebuilt).
    /// 3. CreateRemoteThread to call the remote function.
    /// 4. The thread exit code is the `const char*` returned by HookGetStats
    ///    (a pointer to a static buffer inside the remote process).
    /// 5. ReadProcessMemory to read the JSON string from the remote buffer.
    /// 6. Parse the JSON into `HookStats`.
    ///
    /// Returns `Ok(HookStats::default())` when not injected (no-op).
    pub fn get_stats(&self) -> Result<HookStats, String> {
        if !self.injected || self.target_pid == 0 || self.remote_module == 0 {
            return Ok(HookStats::default());
        }

        // SAFETY: Standard CreateRemoteThread pattern. All handles are
        // released on every path. The remote function returns a pointer
        // (exit code) to a static buffer in the remote process; we copy
        // the bytes out via ReadProcessMemory and never dereference the
        // pointer locally.
        unsafe {
            let process = OpenProcess(
                PROCESS_CREATE_THREAD
                    | PROCESS_QUERY_INFORMATION
                    | PROCESS_VM_OPERATION
                    | PROCESS_VM_READ
                    | PROCESS_VM_WRITE,
                false,
                self.target_pid,
            )
            .map_err(|e| format!("get_stats: OpenProcess failed: {}", e))?;

            // Resolve HookGetStats locally, compute RVA, rebase to remote.
            let stats_result = self.remote_call_returning_pointer(
                process,
                self.target_pid,
                self.remote_module,
                b"HookGetStats\0",
            );

            let _ = CloseHandle(process);

            let json_ptr = stats_result?;
            if json_ptr == 0 {
                return Ok(HookStats::default());
            }

            // Read the JSON string from the remote process. The DLL uses
            // a static 512-byte buffer, so reading 512 bytes is safe.
            let mut buf = vec![0u8; 512];
            let mut bytes_read = 0usize;
            // Re-open for ReadProcessMemory (the process handle above was
            // consumed by remote_call_returning_pointer's cleanup path; we
            // pass a fresh handle here).
            let proc2 = OpenProcess(
                PROCESS_VM_READ | PROCESS_QUERY_INFORMATION,
                false,
                self.target_pid,
            )
            .map_err(|e| format!("get_stats: OpenProcess(read) failed: {}", e))?;

            let read_ok = ReadProcessMemory(
                proc2,
                json_ptr as *const _,
                buf.as_mut_ptr() as *mut _,
                buf.len(),
                Some(&mut bytes_read),
            )
            .is_ok();
            let _ = CloseHandle(proc2);

            if !read_ok || bytes_read == 0 {
                return Err("get_stats: ReadProcessMemory failed".to_string());
            }

            // Truncate at first NUL.
            let nul = buf.iter().position(|&b| b == 0).unwrap_or(bytes_read);
            let json_str = std::str::from_utf8(&buf[..nul])
                .map_err(|e| format!("get_stats: invalid UTF-8 in stats JSON: {}", e))?;

            parse_stats_json(json_str)
        }
    }

    /// 4d: Install an H-Code inline hook in the remote process.
    ///
    /// Flow:
    /// 1. Resolve the target module's base address in the remote process
    ///    (case-insensitive basename match against `code.module`).
    /// 2. Compute the absolute hook address:
    ///    - If `is_rva(code.addr)`, hook = module_base + code.addr
    ///    - Otherwise, hook = code.addr (treated as absolute VA)
    /// 3. Allocate a `HookInstallParams` struct in the remote process and
    ///    write the field values via WriteProcessMemory.
    /// 4. Resolve `HookInstallAtAddressStruct` locally, compute RVA, rebase
    ///    to remote module handle.
    /// 5. CreateRemoteThread with lpParameter = remote struct pointer.
    /// 6. Wait for thread exit (5s timeout), read exit code.
    /// 7. Free the remote struct memory.
    ///
    /// Returns the resolved address + exit code on success.
    pub fn install_h_code(
        &self,
        code: &crate::hook_code::HookCode,
        default_ansi_cp: u32,
    ) -> Result<HookInstallResult, String> {
        if !self.injected || self.target_pid == 0 || self.remote_module == 0 {
            return Err("install_h_code: not injected".to_string());
        }

        // SAFETY: Same CreateRemoteThread + WriteProcessMemory pattern as
        // remote_uninstall. The remote struct is freed on every path. The
        // remote thread runs HookInstallAtAddressStruct which performs its
        // own SEH-wrapped VirtualProtect + trampoline alloc.
        unsafe {
            let process = OpenProcess(
                PROCESS_CREATE_THREAD
                    | PROCESS_QUERY_INFORMATION
                    | PROCESS_VM_OPERATION
                    | PROCESS_VM_READ
                    | PROCESS_VM_WRITE,
                false,
                self.target_pid,
            )
            .map_err(|e| format!("install_h_code: OpenProcess failed: {}", e))?;

            // Step 1+2: resolve absolute hook address.
            let resolved_addr = if crate::hook_code::is_rva(code.addr) {
                // RVA into module — find module base in remote process.
                let base = find_remote_module_base(process, self.target_pid, &code.module)?
                    .ok_or_else(|| format!(
                        "install_h_code: module '{}' not found in target PID {}",
                        code.module, self.target_pid
                    ))?;
                base.wrapping_add(code.addr as usize)
            } else {
                code.addr as usize
            };

            // Step 3: allocate + write HookInstallParams in remote memory.
            let params = HookInstallParams {
                target_addr: resolved_addr,
                data_offset: code.data_offset,
                deref_levels: code.deref_levels,
                code_page: code.text_type.code_page(default_ansi_cp),
                text_type: 0, // null-terminated (only type we currently support)
            };

            let params_ptr = VirtualAllocEx(
                process,
                None,
                std::mem::size_of::<HookInstallParams>(),
                MEM_COMMIT,
                PAGE_READWRITE,
            );
            if params_ptr.is_null() {
                let _ = CloseHandle(process);
                return Err("install_h_code: VirtualAllocEx failed".to_string());
            }

            let mut written = 0usize;
            let write_ok = WriteProcessMemory(
                process,
                params_ptr,
                &params as *const _ as *const _,
                std::mem::size_of::<HookInstallParams>(),
                Some(&mut written),
            )
            .is_ok();

            if !write_ok {
                let _ = VirtualFreeEx(process, params_ptr, 0, MEM_RELEASE);
                let _ = CloseHandle(process);
                return Err("install_h_code: WriteProcessMemory failed".to_string());
            }

            // Step 4: resolve HookInstallAtAddressStruct in local DLL + rebase.
            let local_dll = self.find_hook_dll()?;
            let local_mod = LoadLibraryW(PCWSTR(to_wide(&local_dll).as_ptr()))
                .map_err(|e| format!("install_h_code: LoadLibraryW failed: {}", e))?;

            let local_proc = GetProcAddress(
                local_mod,
                PSTR(b"HookInstallAtAddressStruct\0".as_ptr() as *mut _),
            )
            .ok_or_else(|| {
                let _ = FreeLibrary(local_mod);
                "install_h_code: GetProcAddress HookInstallAtAddressStruct failed".to_string()
            })?;

            // Reuse S5-5 metadata check before computing RVA.
            let mut local_info = MODULEINFO::default();
            let local_process = GetCurrentProcess();
            if let Err(e) = GetModuleInformation(
                local_process,
                local_mod,
                &mut local_info,
                std::mem::size_of::<MODULEINFO>() as u32,
            ) {
                let _ = FreeLibrary(local_mod);
                let _ = VirtualFreeEx(process, params_ptr, 0, MEM_RELEASE);
                let _ = CloseHandle(process);
                return Err(format!("install_h_code: GetModuleInformation local failed: {}", e));
            }

            let mut remote_info = MODULEINFO::default();
            if let Err(e) = GetModuleInformation(
                process,
                HMODULE(self.remote_module as *mut _),
                &mut remote_info,
                std::mem::size_of::<MODULEINFO>() as u32,
            ) {
                let _ = FreeLibrary(local_mod);
                let _ = VirtualFreeEx(process, params_ptr, 0, MEM_RELEASE);
                let _ = CloseHandle(process);
                return Err(format!("install_h_code: GetModuleInformation remote failed: {}", e));
            }

            let local_path = module_file_path(local_process, local_mod);
            let remote_path = module_file_path(process, HMODULE(self.remote_module as *mut _));
            let local_file_size = local_path
                .as_ref()
                .and_then(|p| std::fs::metadata(p).ok())
                .map(|m| m.len());
            let remote_file_size = remote_path
                .as_ref()
                .and_then(|p| std::fs::metadata(p).ok())
                .map(|m| m.len());

            if let Err(e) = compare_dll_metadata(
                local_path.as_deref(),
                local_file_size,
                local_info.SizeOfImage,
                remote_path.as_deref(),
                remote_file_size,
                remote_info.SizeOfImage,
            ) {
                let _ = FreeLibrary(local_mod);
                let _ = VirtualFreeEx(process, params_ptr, 0, MEM_RELEASE);
                let _ = CloseHandle(process);
                return Err(e);
            }

            let local_base = local_mod.0 as usize;
            let rva = (local_proc as usize).wrapping_sub(local_base);
            let remote_fn = self.remote_module.wrapping_add(rva);
            let _ = FreeLibrary(local_mod);

            // Step 5: CreateRemoteThread with lpParameter = remote params ptr.
            let thread = CreateRemoteThread(
                process,
                None,
                0,
                Some(std::mem::transmute(remote_fn)),
                Some(params_ptr),
                0,
                None,
            )
            .map_err(|e| {
                let _ = VirtualFreeEx(process, params_ptr, 0, MEM_RELEASE);
                let _ = CloseHandle(process);
                format!("install_h_code: CreateRemoteThread failed: {}", e)
            })?;

            // Step 6: wait + read exit code.
            let _ = WaitForSingleObject(thread, 5000);
            let mut exit_code = 0u32;
            let _ = GetExitCodeThread(thread, &mut exit_code);
            let _ = CloseHandle(thread);

            // Step 7: free remote params memory.
            let _ = VirtualFreeEx(process, params_ptr, 0, MEM_RELEASE);
            let _ = CloseHandle(process);

            let success = exit_code == 0;
            let message = match exit_code {
                0 => "inline hook installed".to_string(),
                1 => "invalid parameters".to_string(),
                2 => "VirtualProtect failed (target not writable)".to_string(),
                3 => "trampoline allocation failed".to_string(),
                4 => "unsupported architecture".to_string(),
                other => format!("unknown error code {}", other),
            };

            Ok(HookInstallResult {
                exit_code,
                resolved_addr: resolved_addr as u64,
                success,
                message,
            })
        }
    }

    /// 4b: Helper for `get_stats()` — call a remote DLL export that returns
    /// a `const char*` (pointer to static buffer). Returns the pointer as
    /// a `usize` (the thread exit code). Caller is responsible for
    /// ReadProcessMemory on the returned pointer.
    ///
    /// SAFETY: Caller must ensure `process` has PROCESS_CREATE_THREAD +
    /// PROCESS_VM_READ permissions, and `remote_module` is a valid HMODULE
    /// in `process`. The function name must be a NUL-terminated ASCII string.
    unsafe fn remote_call_returning_pointer(
        &self,
        process: HANDLE,
        _pid: u32,
        remote_module: usize,
        export_name: &[u8],
    ) -> Result<usize, String> {
        // Resolve export locally, compute RVA, rebase to remote.
        let local_dll = self.find_hook_dll()?;
        let local_mod = LoadLibraryW(PCWSTR(to_wide(&local_dll).as_ptr()))
            .map_err(|e| format!("LoadLibraryW local hook dll failed: {}", e))?;

        let local_proc = GetProcAddress(local_mod, PSTR(export_name.as_ptr() as *mut _))
            .ok_or_else(|| {
                let _ = FreeLibrary(local_mod);
                format!(
                    "GetProcAddress {} failed",
                    std::str::from_utf8(export_name).unwrap_or("?").trim_end_matches('\0')
                )
            })?;

        // S5-5 metadata check before computing RVA.
        let mut local_info = MODULEINFO::default();
        let local_process = GetCurrentProcess();
        if let Err(e) = GetModuleInformation(
            local_process,
            local_mod,
            &mut local_info,
            std::mem::size_of::<MODULEINFO>() as u32,
        ) {
            let _ = FreeLibrary(local_mod);
            return Err(format!("GetModuleInformation local failed: {}", e));
        }

        let mut remote_info = MODULEINFO::default();
        if let Err(e) = GetModuleInformation(
            process,
            HMODULE(remote_module as *mut _),
            &mut remote_info,
            std::mem::size_of::<MODULEINFO>() as u32,
        ) {
            let _ = FreeLibrary(local_mod);
            return Err(format!("GetModuleInformation remote failed: {}", e));
        }

        let local_path = module_file_path(local_process, local_mod);
        let remote_path = module_file_path(process, HMODULE(remote_module as *mut _));
        let local_file_size = local_path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len());
        let remote_file_size = remote_path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len());

        if let Err(e) = compare_dll_metadata(
            local_path.as_deref(),
            local_file_size,
            local_info.SizeOfImage,
            remote_path.as_deref(),
            remote_file_size,
            remote_info.SizeOfImage,
        ) {
            let _ = FreeLibrary(local_mod);
            return Err(e);
        }

        let local_base = local_mod.0 as usize;
        let rva = (local_proc as usize).wrapping_sub(local_base);
        let remote_fn = remote_module.wrapping_add(rva);
        let _ = FreeLibrary(local_mod);

        // CreateRemoteThread: the export takes no params, so lpParameter = None.
        // Its return value (a `const char*`) becomes the thread exit code.
        let thread = CreateRemoteThread(
            process,
            None,
            0,
            Some(std::mem::transmute(remote_fn)),
            None,
            0,
            None,
        )
        .map_err(|e| format!("CreateRemoteThread failed: {}", e))?;

        let _ = WaitForSingleObject(thread, 5000);
        let mut exit_code = 0u32;
        let _ = GetExitCodeThread(thread, &mut exit_code);
        let _ = CloseHandle(thread);

        Ok(exit_code as usize)
    }

    // --- Internal helpers ---

    /// Whether moon_hook.dll is discoverable (for UI preflight).
    pub fn dll_available(&self) -> bool {
        self.find_hook_dll().is_ok()
    }

    /// Resolved absolute path if present.
    pub fn dll_path(&self) -> Option<String> {
        self.find_hook_dll().ok()
    }

    fn find_hook_dll(&self) -> Result<String, String> {
        // Look for moon_hook.dll next to the exe (release bundle), then dev build outputs.
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_default();

        let mut candidates = vec![
            // Release / portable: DLL next to moontranslator.exe
            exe_dir.join("moon_hook.dll"),
            exe_dir.join("resources").join("moon_hook.dll"),
            exe_dir.join("bin").join("moon_hook.dll"),
            // Relative to exe when running from target/debug
            exe_dir.join("..\\..\\hook-dll\\build\\Release\\moon_hook.dll"),
            exe_dir.join("..\\..\\bin\\moon_hook.dll"),
            exe_dir.join("..\\..\\src-tauri\\bin\\moon_hook.dll"),
            exe_dir.join("..\\..\\src-tauri\\hook-dll\\build\\Release\\moon_hook.dll"),
            // CWD-relative (cargo run from repo root or src-tauri)
            std::path::PathBuf::from("moon_hook.dll"),
            std::path::PathBuf::from("bin\\moon_hook.dll"),
            std::path::PathBuf::from("src-tauri\\bin\\moon_hook.dll"),
            std::path::PathBuf::from("src-tauri\\hook-dll\\build\\Release\\moon_hook.dll"),
            std::path::PathBuf::from("hook-dll\\build\\Release\\moon_hook.dll"),
        ];

        // Compile-time crate dir → always find repo hook-dll in dev
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        candidates.push(manifest_dir.join("bin").join("moon_hook.dll"));
        candidates.push(
            manifest_dir
                .join("hook-dll")
                .join("build")
                .join("Release")
                .join("moon_hook.dll"),
        );
        candidates.push(
            manifest_dir
                .join("hook-dll")
                .join("build")
                .join("Debug")
                .join("moon_hook.dll"),
        );

        for candidate in &candidates {
            if candidate.exists() {
                return candidate
                    .canonicalize()
                    .unwrap_or_else(|_| candidate.clone())
                    .to_str()
                    .ok_or_else(|| "Invalid path".to_string())
                    .map(|s| s.to_string());
            }
        }

        Err(format!(
            "moon_hook.dll not found. Place it next to the app or under src-tauri/hook-dll/build/Release/. Searched {} paths.",
            candidates.len()
        ))
    }

    /// Open shared memory created by the hook DLL.
    /// SAFETY: Opens a named file mapping and maps it into our address space.
    /// - Magic number is validated to ensure shared memory is valid
    /// - On failure, resources are cleaned up immediately
    fn open_shared_memory(&mut self, pid: u32) -> Result<(), String> {
        unsafe {
            let name = shared_memory_name(pid);
            let name_wide = to_wide(&name);
            let handle = OpenFileMappingW(
                FILE_MAP_ALL_ACCESS.0 as u32,
                false,
                PCWSTR(name_wide.as_ptr()),
            )
            .map_err(|e| format!("OpenFileMappingW({}) failed: {}", name, e))?;

            let view = MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, SHARED_MEMORY_SIZE);

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

    /// Cleanup shared memory resources.
    /// SAFETY: Unmaps view and closes handle. Sets pointers to null after cleanup.
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

// ── 4d: H-Code module resolution helper ───────────────────────────────

/// Find a module's base address in a remote process by case-insensitive
/// basename match (e.g. "game.exe" matches "C:\\path\\game.exe").
///
/// Uses `EnumProcessModules` + `GetModuleFileNameExW` to walk every loaded
/// module in the target process and compare basenames. Returns the module
/// base address as `usize` (cast from HMODULE), or `None` if not found.
///
/// SAFETY: Caller must ensure `process` has `PROCESS_QUERY_INFORMATION |
/// PROCESS_VM_READ` permissions.
unsafe fn find_remote_module_base(
    process: HANDLE,
    _pid: u32,
    target_basename: &str,
) -> Result<Option<usize>, String> {
    use windows::Win32::System::ProcessStatus::EnumProcessModules;

    let mut mods: [HMODULE; 1024] = [HMODULE::default(); 1024];
    let mut needed = 0u32;
    if EnumProcessModules(process, mods.as_mut_ptr(), std::mem::size_of_val(&mods) as u32, &mut needed)
        .is_err()
    {
        return Ok(None);
    }
    let count = (needed as usize / std::mem::size_of::<HMODULE>()).min(mods.len());

    let target_lower = target_basename.to_ascii_lowercase();
    for m in &mods[..count] {
        if m.is_invalid() {
            continue;
        }
        let path = module_file_path(process, *m);
        if let Some(path) = path {
            // Extract basename (handle both \ and /).
            let basename = path.rsplit(|c| c == '\\' || c == '/').next().unwrap_or("");
            if basename.to_ascii_lowercase() == target_lower {
                return Ok(Some(m.0 as usize));
            }
        }
    }
    Ok(None)
}

// ── 4b: HookStats JSON parser ─────────────────────────────────────────

/// Parse the JSON string emitted by the DLL's `HookGetStats` export.
///
/// We use a tiny hand-rolled parser instead of pulling in `serde_json` for
/// a single fixed-shape object — the DLL's output format is stable and
/// documented in `hook_text.cpp::HookGetStats`.
fn parse_stats_json(json: &str) -> Result<HookStats, String> {
    let mut stats = HookStats::default();
    // Strip braces.
    let inner = json.trim().trim_start_matches('{').trim_end_matches('}');
    for field in inner.split(',') {
        let Some((key, value)) = field.split_once(':') else {
            continue;
        };
        let key = key.trim().trim_matches('"');
        let value = value.trim();
        match key {
            "modulesScanned" => stats.modules_scanned = parse_u64(value),
            "iatHits" => stats.iat_hits = parse_u64(value),
            "lateLoadedPatched" => stats.late_loaded_patched = parse_u64(value),
            "sendTextCalls" => stats.send_text_calls = parse_u64(value),
            "sendTextFiltered" => stats.send_text_filtered = parse_u64(value),
            "sendTextEjectBlocked" => stats.send_text_eject_blocked = parse_u64(value),
            "inlineHooks" => stats.inline_hooks = parse_u64(value),
            "hooksInstalled" => stats.hooks_installed = value == "true",
            "ldrCookie" => stats.ldr_cookie = value == "true",
            _ => {}
        }
    }
    Ok(stats)
}

fn parse_u64(s: &str) -> u64 {
    s.trim().parse().unwrap_or(0)
}

#[cfg(test)]
mod stats_parser_tests {
    use super::*;

    #[test]
    fn parse_full_stats_json() {
        let json = r#"{"modulesScanned":42,"iatHits":128,"lateLoadedPatched":3,"sendTextCalls":1024,"sendTextFiltered":200,"sendTextEjectBlocked":0,"inlineHooks":2,"hooksInstalled":true,"ldrCookie":true}"#;
        let stats = parse_stats_json(json).unwrap();
        assert_eq!(stats.modules_scanned, 42);
        assert_eq!(stats.iat_hits, 128);
        assert_eq!(stats.late_loaded_patched, 3);
        assert_eq!(stats.send_text_calls, 1024);
        assert_eq!(stats.send_text_filtered, 200);
        assert_eq!(stats.send_text_eject_blocked, 0);
        assert_eq!(stats.inline_hooks, 2);
        assert!(stats.hooks_installed);
        assert!(stats.ldr_cookie);
    }

    #[test]
    fn parse_stats_after_eject() {
        let json = r#"{"modulesScanned":42,"iatHits":128,"lateLoadedPatched":3,"sendTextCalls":1024,"sendTextFiltered":200,"sendTextEjectBlocked":5,"inlineHooks":0,"hooksInstalled":false,"ldrCookie":false}"#;
        let stats = parse_stats_json(json).unwrap();
        assert_eq!(stats.send_text_eject_blocked, 5);
        assert!(!stats.hooks_installed);
        assert!(!stats.ldr_cookie);
    }

    #[test]
    fn parse_stats_empty_object() {
        let stats = parse_stats_json("{}").unwrap();
        assert_eq!(stats.modules_scanned, 0);
        assert!(!stats.hooks_installed);
    }

    #[test]
    fn parse_stats_with_whitespace() {
        let json = r#"{"modulesScanned":  7 , "iatHits": 11}"#;
        let stats = parse_stats_json(json).unwrap();
        assert_eq!(stats.modules_scanned, 7);
        assert_eq!(stats.iat_hits, 11);
    }

    #[test]
    fn parse_stats_unknown_field_ignored() {
        let json = r#"{"modulesScanned":1,"unknownField":"ignore me"}"#;
        let stats = parse_stats_json(json).unwrap();
        assert_eq!(stats.modules_scanned, 1);
    }

    #[test]
    fn hook_install_params_size_is_24_bytes() {
        // Layout assertion: must match DLL's #pragma pack(8) struct.
        // 8 (target_addr) + 4 (data_offset) + 4 (deref_levels) + 4 (code_page) + 4 (text_type) = 24
        assert_eq!(std::mem::size_of::<HookInstallParams>(), 24);
    }
}

// ── S5-5: remote_uninstall RVA safety helpers ──────────────────────────

/// Get the full file path of a loaded module in a (possibly remote) process.
/// Returns None if GetModuleFileNameExW fails or yields an empty/non-UTF8 path.
/// Used to compare the local and remote hook DLL file paths before rebasing RVA.
fn module_file_path(process: HANDLE, module: HMODULE) -> Option<String> {
    // SAFETY: GetModuleFileNameExW writes into a caller-provided buffer; we
    // bound it with a fixed-size array and treat the result as UTF-16.
    unsafe {
        let mut buf = [0u16; 1024];
        let len = GetModuleFileNameExW(process, module, &mut buf);
        if len == 0 {
            return None;
        }
        String::from_utf16(&buf[..len as usize]).ok()
    }
}

/// S5-5: compare local and remote hook DLL metadata before rebasing RVA.
///
/// Three signals are checked, in order of strictness:
/// 1. `SizeOfImage` (always available) — catches most rebuilds because
///    section layout changes ripple through the image size.
/// 2. File path equality (when both paths are available) — catches the
///    edge case where SizeOfImage coincidentally matches across builds.
/// 3. File size on disk (when both paths stat successfully) — catches
///    same-path-different-content (e.g. local rebuild overwrote the DLL
///    the remote process loaded earlier).
///
/// Returns `Ok(())` if all available signals match, `Err(message)` on any
/// mismatch. Missing signals (None) are skipped — they don't fail the check
/// but also don't contribute confidence. At minimum SizeOfImage is required.
fn compare_dll_metadata(
    local_path: Option<&str>,
    local_file_size: Option<u64>,
    local_image_size: u32,
    remote_path: Option<&str>,
    remote_file_size: Option<u64>,
    remote_image_size: u32,
) -> Result<(), String> {
    // 1. SizeOfImage — always present, mandatory check.
    if local_image_size != remote_image_size {
        return Err(format!(
            "DLL image size mismatch (local={} vs remote={}) — possible version mismatch, refusing RVA rebase",
            local_image_size, remote_image_size
        ));
    }

    // 2. File path — optional, only checked when both sides resolve.
    if let (Some(lp), Some(rp)) = (local_path, remote_path) {
        // Case-insensitive on Windows (NTFS is case-insensitive by default).
        if !lp.eq_ignore_ascii_case(rp) {
            return Err(format!(
                "DLL path mismatch (local='{}' vs remote='{}') — refusing RVA rebase",
                lp, rp
            ));
        }
    }

    // 3. File size on disk — optional, only checked when both stats succeed.
    if let (Some(ls), Some(rs)) = (local_file_size, remote_file_size) {
        if ls != rs {
            return Err(format!(
                "DLL file size mismatch (local={} vs remote={}) — same path but different content, refusing RVA rebase",
                ls, rs
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod s5_5_tests {
    use super::*;

    // ── SizeOfImage checks ─────────────────────────────────────────────

    #[test]
    fn image_size_match_passes() {
        assert!(compare_dll_metadata(None, None, 0x50000, None, None, 0x50000).is_ok());
    }

    #[test]
    fn image_size_mismatch_fails() {
        let err = compare_dll_metadata(None, None, 0x50000, None, None, 0x60000).unwrap_err();
        assert!(err.contains("image size mismatch"), "got: {err}");
        assert!(err.contains("refusing RVA rebase"), "got: {err}");
    }

    // ── File path checks ───────────────────────────────────────────────

    #[test]
    fn path_match_passes() {
        let path = "C:\\app\\moon_hook.dll";
        assert!(compare_dll_metadata(
            Some(path), None, 0x50000,
            Some(path), None, 0x50000,
        )
        .is_ok());
    }

    #[test]
    fn path_case_insensitive_match_passes() {
        // NTFS is case-insensitive; MOON_HOOK.dll and moon_hook.dll are the same file.
        assert!(compare_dll_metadata(
            Some("C:\\app\\MOON_HOOK.dll"), None, 0x50000,
            Some("C:\\app\\moon_hook.dll"), None, 0x50000,
        )
        .is_ok());
    }

    #[test]
    fn path_mismatch_fails() {
        let err = compare_dll_metadata(
            Some("C:\\app\\moon_hook.dll"), None, 0x50000,
            Some("D:\\other\\moon_hook.dll"), None, 0x50000,
        )
        .unwrap_err();
        assert!(err.contains("path mismatch"), "got: {err}");
    }

    #[test]
    fn missing_remote_path_skips_path_check() {
        // Path check is skipped when remote path is unavailable (e.g. access denied).
        // SizeOfImage match alone should pass.
        assert!(compare_dll_metadata(
            Some("C:\\app\\moon_hook.dll"), None, 0x50000,
            None, None, 0x50000,
        )
        .is_ok());
    }

    // ── File size checks ───────────────────────────────────────────────

    #[test]
    fn file_size_match_passes() {
        assert!(compare_dll_metadata(
            Some("C:\\app\\moon_hook.dll"), Some(123456), 0x50000,
            Some("C:\\app\\moon_hook.dll"), Some(123456), 0x50000,
        )
        .is_ok());
    }

    #[test]
    fn file_size_mismatch_fails() {
        let err = compare_dll_metadata(
            Some("C:\\app\\moon_hook.dll"), Some(123456), 0x50000,
            Some("C:\\app\\moon_hook.dll"), Some(789012), 0x50000,
        )
        .unwrap_err();
        assert!(err.contains("file size mismatch"), "got: {err}");
        assert!(err.contains("different content"), "got: {err}");
    }

    #[test]
    fn missing_file_size_skips_size_check() {
        // File stat may fail (e.g. file deleted after load); should not fail the check.
        assert!(compare_dll_metadata(
            Some("C:\\app\\moon_hook.dll"), Some(123456), 0x50000,
            Some("C:\\app\\moon_hook.dll"), None, 0x50000,
        )
        .is_ok());
    }

    // ── Combined: multiple signals ─────────────────────────────────────

    #[test]
    fn all_three_signals_match_passes() {
        assert!(compare_dll_metadata(
            Some("C:\\app\\moon_hook.dll"), Some(123456), 0x50000,
            Some("C:\\app\\moon_hook.dll"), Some(123456), 0x50000,
        )
        .is_ok());
    }

    #[test]
    fn image_size_mismatch_short_circuits_before_path_check() {
        // Even if paths match, SizeOfImage mismatch should fail first.
        let err = compare_dll_metadata(
            Some("C:\\app\\moon_hook.dll"), Some(123456), 0x50000,
            Some("C:\\app\\moon_hook.dll"), Some(123456), 0x60000,
        )
        .unwrap_err();
        assert!(err.contains("image size mismatch"), "got: {err}");
    }
}
