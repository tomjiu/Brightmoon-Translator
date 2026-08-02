/**
 * Moon Translator - Text Hooking DLL
 *
 * Injects into target process and hooks text rendering APIs:
 * - TextOutW / TextOutA
 * - ExtTextOutW / ExtTextOutA
 * - DrawTextW / DrawTextA
 * - DrawTextExW / DrawTextExA
 *
 * Captured text is sent to the main application via shared memory.
 */

#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif

#include <windows.h>
#include <psapi.h>
#include <string>
#include <cstring>
#include <cstdint>
#include <atomic>

#pragma comment(lib, "psapi.lib")

// ============================================================================
// 4a: LdrRegisterDllNotification — catch late-loaded modules and patch their IAT
// ============================================================================
// LdrRegisterDllNotification is an undocumented-but-stable NT API available
// on Vista+. It fires a callback whenever any DLL is mapped into the process.
// We use it to re-run PatchAllModulesIAT on the newly loaded module so that
// hooks stay effective even when the target app LoadLibrary's a plugin DLL
// after our initial InstallHooks() sweep.
//
// Signature (from ntdll.dll, dynamically resolved):
//   NTSTATUS NTAPI LdrRegisterDllNotification(
//       ULONG Flags,                            // must be 0
//       PLDR_DLL_NOTIFICATION_FUNCTION Callback,
//       PVOID Context,
//       PVOID *Cookie);
//   NTSTATUS NTAPI LdrUnregisterDllNotification(PVOID Cookie);

constexpr ULONG LDR_DLL_NOTIFICATION_REASON_LOADED = 1;

typedef struct _LSA_UNICODE_STRING {
    USHORT Length;
    USHORT MaximumLength;
    PWSTR  Buffer;
} LSA_UNICODE_STRING, *PLSA_UNICODE_STRING;

typedef struct _LDR_DLL_LOADED_NOTIFICATION_DATA {
    ULONG Flags;
    PLSA_UNICODE_STRING FullDllName;
    PLSA_UNICODE_STRING BaseDllName;
    PVOID DllBase;
    ULONG SizeOfImage;
} LDR_DLL_LOADED_NOTIFICATION_DATA;

typedef struct _LDR_DLL_NOTIFICATION_DATA {
    union {
        LDR_DLL_LOADED_NOTIFICATION_DATA Loaded;
        // Unloaded variant shares layout; we only care about Loaded.
    };
} LDR_DLL_NOTIFICATION_DATA, *PLDR_DLL_NOTIFICATION_DATA;

typedef VOID (CALLBACK *PLDR_DLL_NOTIFICATION_FUNCTION)(
    ULONG NotificationReason,
    PLDR_DLL_NOTIFICATION_DATA NotificationData,
    PVOID Context);

typedef LONG (NTAPI *LdrRegisterDllNotification_t)(
    ULONG, PLDR_DLL_NOTIFICATION_FUNCTION, PVOID, PVOID*);
typedef LONG (NTAPI *LdrUnregisterDllNotification_t)(PVOID);

static PVOID g_ldr_cookie = nullptr;
static HMODULE g_ntdll = nullptr;
static LdrRegisterDllNotification_t   g_pLdrRegister   = nullptr;
static LdrUnregisterDllNotification_t g_pLdrUnregister = nullptr;

// ============================================================================
// 4b: HookStats — track IAT patch hits + SendText volume for verification
// ============================================================================

struct HookStats {
    std::atomic<uint64_t> total_modules_scanned{0};   // modules walked by PatchAllModulesIAT
    std::atomic<uint64_t> total_iat_hits{0};          // IAT slots successfully patched
    std::atomic<uint64_t> late_loaded_patched{0};     // modules patched via Ldr callback
    std::atomic<uint64_t> send_text_calls{0};         // SendTextToHost invocations
    std::atomic<uint64_t> send_text_filtered{0};      // calls rejected by IsPrintableText
    std::atomic<uint64_t> send_text_eject_blocked{0}; // calls blocked during eject
    std::atomic<uint64_t> inline_hooks_installed{0};  // H-Code inline hooks
};

static HookStats g_stats;

// ============================================================================
// 4c: Eject flow — gate SendText during uninstall + verify IAT rollback
// ============================================================================

// When true, SendTextToHost returns immediately. Set by UninstallHooks before
// restoring IAT slots so no in-flight hook callback can write to shared
// memory while we're tearing it down.
static std::atomic<bool> g_ejecting{false};

// ============================================================================
// Shared Memory IPC
// ============================================================================

#pragma pack(push, 1)
struct SharedMemoryHeader {
    uint32_t magic;           // 0x4D4F4F4E ("MOON")
    uint32_t version;         // 2 (read_offset introduced)
    uint32_t write_offset;    // Current write position
    uint32_t buffer_size;     // Total buffer size
    uint32_t sequence;        // Sequence number for new messages
    uint32_t read_offset;     // Reader's consumed position (P0#4/#5)
    char     process_name[64]; // Target process name
    uint8_t  reserved[36];    // Reserved for future use
};

struct TextMessage {
    uint32_t length;          // Length of text in bytes (UTF-8)
    uint32_t code_page;       // Source code page (e.g., 932 for Shift-JIS)
    int32_t  x;               // Screen X position
    int32_t  y;               // Screen Y position
    uint64_t timestamp;       // GetTickCount64()
    // Followed by `length` bytes of UTF-8 text
};
#pragma pack(pop)

static const uint32_t SHARED_MEMORY_MAGIC = 0x4D4F4F4E; // "MOON"
static const uint32_t SHARED_MEMORY_SIZE = 1024 * 1024; // 1MB
static wchar_t g_shared_mem_name[128] = L"MoonTranslatorHookSharedMem";

static HANDLE g_shared_mem = nullptr;
static SharedMemoryHeader* g_shared_data = nullptr;
static CRITICAL_SECTION g_write_lock;
static bool g_write_lock_inited = false;
static HMODULE g_self_module = nullptr;

static void BuildSharedMemoryName() {
    DWORD pid = GetCurrentProcessId();
    swprintf_s(g_shared_mem_name, L"MoonTranslatorHookSharedMem_PID%lu", (unsigned long)pid);
}

// ============================================================================
// Original function pointers
// ============================================================================

typedef BOOL(WINAPI* TextOutW_t)(HDC, int, int, LPCWSTR, int);
typedef BOOL(WINAPI* TextOutA_t)(HDC, int, int, LPCSTR, int);
typedef BOOL(WINAPI* ExtTextOutW_t)(HDC, int, int, UINT, const RECT*, LPCWSTR, UINT, const INT*);
typedef BOOL(WINAPI* ExtTextOutA_t)(HDC, int, int, UINT, const RECT*, LPCSTR, UINT, const INT*);
typedef int(WINAPI* DrawTextW_t)(HDC, LPCWSTR, int, LPRECT, UINT);
typedef int(WINAPI* DrawTextA_t)(HDC, LPCSTR, int, LPRECT, UINT);
typedef int(WINAPI* DrawTextExW_t)(HDC, LPWSTR, int, LPRECT, UINT, LPDRAWTEXTPARAMS);
typedef int(WINAPI* DrawTextExA_t)(HDC, LPSTR, int, LPRECT, UINT, LPDRAWTEXTPARAMS);

static TextOutW_t     g_orig_TextOutW = nullptr;
static TextOutA_t     g_orig_TextOutA = nullptr;
static ExtTextOutW_t  g_orig_ExtTextOutW = nullptr;
static ExtTextOutA_t  g_orig_ExtTextOutA = nullptr;
static DrawTextW_t    g_orig_DrawTextW = nullptr;
static DrawTextA_t    g_orig_DrawTextA = nullptr;
static DrawTextExW_t  g_orig_DrawTextExW = nullptr;
static DrawTextExA_t  g_orig_DrawTextExA = nullptr;

static bool g_hooks_installed = false;

// ============================================================================
// Helper functions
// ============================================================================

static std::string WideToUtf8(const wchar_t* wstr, int len) {
    if (len <= 0 || !wstr) return "";
    int utf8_len = WideCharToMultiByte(CP_UTF8, 0, wstr, len, nullptr, 0, nullptr, nullptr);
    if (utf8_len <= 0) return "";
    std::string result(utf8_len, '\0');
    WideCharToMultiByte(CP_UTF8, 0, wstr, len, &result[0], utf8_len, nullptr, nullptr);
    return result;
}

static std::string AnsiToUtf8(const char* str, int len, UINT code_page) {
    if (len <= 0 || !str) return "";
    // First convert to wide
    int wide_len = MultiByteToWideChar(code_page, 0, str, len, nullptr, 0);
    if (wide_len <= 0) return "";
    std::wstring wstr(wide_len, L'\0');
    MultiByteToWideChar(code_page, 0, str, len, &wstr[0], wide_len);
    // Then convert to UTF-8
    return WideToUtf8(wstr.c_str(), wide_len);
}

static bool IsPrintableText(const char* utf8, size_t len) {
    if (len == 0) return false;
    // Filter out very short strings (likely just formatting)
    // Count actual characters (not bytes)
    int char_count = 0;
    for (size_t i = 0; i < len; i++) {
        if ((utf8[i] & 0xC0) != 0x80) char_count++;
    }
    return char_count >= 2;
}

// ============================================================================
// Shared Memory IPC
// ============================================================================

static bool InitSharedMemory() {
    BuildSharedMemoryName();
    g_shared_mem = CreateFileMappingW(
        INVALID_HANDLE_VALUE,
        nullptr,
        PAGE_READWRITE,
        0,
        SHARED_MEMORY_SIZE,
        g_shared_mem_name
    );
    if (!g_shared_mem) return false;

    g_shared_data = (SharedMemoryHeader*)MapViewOfFile(
        g_shared_mem,
        FILE_MAP_ALL_ACCESS,
        0, 0,
        SHARED_MEMORY_SIZE
    );
    if (!g_shared_data) {
        CloseHandle(g_shared_mem);
        g_shared_mem = nullptr;
        return false;
    }

    // Initialize header if newly created
    if (g_shared_data->magic != SHARED_MEMORY_MAGIC) {
        memset(g_shared_data, 0, SHARED_MEMORY_SIZE);
        g_shared_data->magic = SHARED_MEMORY_MAGIC;
        g_shared_data->version = 2;
        g_shared_data->buffer_size = SHARED_MEMORY_SIZE;
        g_shared_data->write_offset = sizeof(SharedMemoryHeader);
        g_shared_data->read_offset = sizeof(SharedMemoryHeader);

        // Store process name
        char proc_name[MAX_PATH];
        GetModuleFileNameA(nullptr, proc_name, MAX_PATH);
        // Extract filename only
        char* last_slash = strrchr(proc_name, '\\');
        const char* name = last_slash ? last_slash + 1 : proc_name;
        strncpy_s(g_shared_data->process_name, sizeof(g_shared_data->process_name), name, _TRUNCATE);
    }

    if (!g_write_lock_inited) {
        InitializeCriticalSection(&g_write_lock);
        g_write_lock_inited = true;
    }
    return true;
}

static void CleanupSharedMemory() {
    if (g_write_lock_inited) {
        DeleteCriticalSection(&g_write_lock);
        g_write_lock_inited = false;
    }
    if (g_shared_data) {
        UnmapViewOfFile(g_shared_data);
        g_shared_data = nullptr;
    }
    if (g_shared_mem) {
        CloseHandle(g_shared_mem);
        g_shared_mem = nullptr;
    }
}

static void SendTextToHost(const char* utf8_text, size_t len, int x, int y, UINT code_page) {
    g_stats.send_text_calls.fetch_add(1, std::memory_order_relaxed);

    // 4c: if eject is in progress, drop the message. The shared memory may
    // be partially unmapped and the host pump has already stopped reading.
    if (g_ejecting.load(std::memory_order_acquire)) {
        g_stats.send_text_eject_blocked.fetch_add(1, std::memory_order_relaxed);
        return;
    }

    if (!g_shared_data || !IsPrintableText(utf8_text, len)) {
        g_stats.send_text_filtered.fetch_add(1, std::memory_order_relaxed);
        return;
    }

    EnterCriticalSection(&g_write_lock);

    uint32_t msg_size = sizeof(TextMessage) + (uint32_t)len;
    uint32_t total_size = msg_size;
    const uint32_t header_size = (uint32_t)sizeof(SharedMemoryHeader);

    // Check if we need to wrap around
    uint32_t end_offset = g_shared_data->write_offset + total_size;
    if (end_offset > g_shared_data->buffer_size) {
        // P0#4/#5: if the reader has not consumed everything yet (read_offset
        // is past the header), wrapping to the head would overwrite unread
        // messages — and a reader mid-scan could see a torn half-message.
        // Drop this message instead of corrupting the buffer. The reader is
        // the host pump (fast); losing one text draw under burst is better
        // than re-reading stale text or a torn frame.
        if (g_shared_data->read_offset > header_size) {
            g_stats.send_text_eject_blocked.fetch_add(1, std::memory_order_relaxed);
            LeaveCriticalSection(&g_write_lock);
            return;
        }
        // Safe to wrap: reader has consumed everything so far.
        g_shared_data->write_offset = header_size;
        g_shared_data->sequence++;
    }

    // Write message
    TextMessage* msg = (TextMessage*)((uint8_t*)g_shared_data + g_shared_data->write_offset);
    msg->length = (uint32_t)len;
    msg->code_page = code_page;
    msg->x = x;
    msg->y = y;
    msg->timestamp = GetTickCount64();
    memcpy((uint8_t*)msg + sizeof(TextMessage), utf8_text, len);

    g_shared_data->write_offset += total_size;
    g_shared_data->sequence++;

    LeaveCriticalSection(&g_write_lock);
}

// ============================================================================
// Hook implementations
// ============================================================================

static BOOL WINAPI HookedTextOutW(HDC hdc, int x, int y, LPCWSTR lpString, int c) {
    std::string utf8 = WideToUtf8(lpString, c);
    SendTextToHost(utf8.c_str(), utf8.size(), x, y, CP_UTF8);
    return g_orig_TextOutW(hdc, x, y, lpString, c);
}

static BOOL WINAPI HookedTextOutA(HDC hdc, int x, int y, LPCSTR lpString, int c) {
    UINT cp = GetACP();
    std::string utf8 = AnsiToUtf8(lpString, c, cp);
    SendTextToHost(utf8.c_str(), utf8.size(), x, y, cp);
    return g_orig_TextOutA(hdc, x, y, lpString, c);
}

static BOOL WINAPI HookedExtTextOutW(HDC hdc, int x, int y, UINT options, const RECT* lprect, LPCWSTR lpString, UINT c, const INT* lpDx) {
    std::string utf8 = WideToUtf8(lpString, c);
    SendTextToHost(utf8.c_str(), utf8.size(), x, y, CP_UTF8);
    return g_orig_ExtTextOutW(hdc, x, y, options, lprect, lpString, c, lpDx);
}

static BOOL WINAPI HookedExtTextOutA(HDC hdc, int x, int y, UINT options, const RECT* lprect, LPCSTR lpString, UINT c, const INT* lpDx) {
    UINT cp = GetACP();
    std::string utf8 = AnsiToUtf8(lpString, c, cp);
    SendTextToHost(utf8.c_str(), utf8.size(), x, y, cp);
    return g_orig_ExtTextOutA(hdc, x, y, options, lprect, lpString, c, lpDx);
}

static int WINAPI HookedDrawTextW(HDC hdc, LPCWSTR lpchText, int cchText, LPRECT lprc, UINT format) {
    int len = (cchText == -1) ? (int)wcslen(lpchText) : cchText;
    std::string utf8 = WideToUtf8(lpchText, len);
    SendTextToHost(utf8.c_str(), utf8.size(), lprc->left, lprc->top, CP_UTF8);
    return g_orig_DrawTextW(hdc, lpchText, cchText, lprc, format);
}

static int WINAPI HookedDrawTextA(HDC hdc, LPCSTR lpchText, int cchText, LPRECT lprc, UINT format) {
    int len = (cchText == -1) ? (int)strlen(lpchText) : cchText;
    UINT cp = GetACP();
    std::string utf8 = AnsiToUtf8(lpchText, len, cp);
    SendTextToHost(utf8.c_str(), utf8.size(), lprc->left, lprc->top, cp);
    return g_orig_DrawTextA(hdc, lpchText, cchText, lprc, format);
}

static int WINAPI HookedDrawTextExW(HDC hdc, LPWSTR lpchText, int cchText, LPRECT lprc, UINT format, LPDRAWTEXTPARAMS lpdtp) {
    int len = (cchText == -1) ? (int)wcslen(lpchText) : cchText;
    std::string utf8 = WideToUtf8(lpchText, len);
    SendTextToHost(utf8.c_str(), utf8.size(), lprc->left, lprc->top, CP_UTF8);
    return g_orig_DrawTextExW(hdc, lpchText, cchText, lprc, format, lpdtp);
}

static int WINAPI HookedDrawTextExA(HDC hdc, LPSTR lpchText, int cchText, LPRECT lprc, UINT format, LPDRAWTEXTPARAMS lpdtp) {
    int len = (cchText == -1) ? (int)strlen(lpchText) : cchText;
    UINT cp = GetACP();
    std::string utf8 = AnsiToUtf8(lpchText, len, cp);
    SendTextToHost(utf8.c_str(), utf8.size(), lprc->left, lprc->top, cp);
    return g_orig_DrawTextExA(hdc, lpchText, cchText, lprc, format, lpdtp);
}

// ============================================================================
// IAT Hooking — patch each *loaded module's* imports of gdi32/user32 text APIs.
// Patching gdi32/user32's own IAT does nothing for app code (classic mistake).
// ============================================================================

/// Patch one module's IAT entry for dll_name!func_name → new_func.
/// If *orig_func is null, stores the previous pointer (real API) once.
/// Returns true if a slot was patched in this module.
static bool PatchIAT(HMODULE module, const char* dll_name, const char* func_name,
                     void* new_func, void** orig_func) {
    if (!module || !dll_name || !func_name || !new_func || !orig_func) return false;

    __try {
        PIMAGE_DOS_HEADER dos_header = (PIMAGE_DOS_HEADER)module;
        if (dos_header->e_magic != IMAGE_DOS_SIGNATURE) return false;

        PIMAGE_NT_HEADERS nt_headers = (PIMAGE_NT_HEADERS)((uint8_t*)module + dos_header->e_lfanew);
        if (nt_headers->Signature != IMAGE_NT_SIGNATURE) return false;

        DWORD import_dir_rva =
            nt_headers->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT].VirtualAddress;
        if (import_dir_rva == 0) return false;

        PIMAGE_IMPORT_DESCRIPTOR import_desc =
            (PIMAGE_IMPORT_DESCRIPTOR)((uint8_t*)module + import_dir_rva);

        for (; import_desc->Name; import_desc++) {
            const char* import_dll_name = (const char*)((uint8_t*)module + import_desc->Name);
            // Accept "gdi32.dll" / "GDI32.dll" / "gdi32"
            if (_stricmp(import_dll_name, dll_name) != 0) {
                // also match without .dll
                size_t n = strlen(dll_name);
                if (n > 4 && _stricmp(dll_name + n - 4, ".dll") == 0) {
                    char bare[64];
                    if (n - 4 < sizeof(bare)) {
                        memcpy(bare, dll_name, n - 4);
                        bare[n - 4] = 0;
                        if (_stricmp(import_dll_name, bare) != 0) continue;
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            PIMAGE_THUNK_DATA thunk =
                (PIMAGE_THUNK_DATA)((uint8_t*)module + import_desc->FirstThunk);
            PIMAGE_THUNK_DATA orig_thunk = import_desc->OriginalFirstThunk
                ? (PIMAGE_THUNK_DATA)((uint8_t*)module + import_desc->OriginalFirstThunk)
                : nullptr;

            for (; thunk->u1.Function; ++thunk) {
                const char* name = nullptr;
                if (orig_thunk) {
                    if (IMAGE_SNAP_BY_ORDINAL(orig_thunk->u1.Ordinal)) {
                        ++orig_thunk;
                        continue;
                    }
                    PIMAGE_IMPORT_BY_NAME import_by_name =
                        (PIMAGE_IMPORT_BY_NAME)((uint8_t*)module + orig_thunk->u1.AddressOfData);
                    name = (const char*)import_by_name->Name;
                    ++orig_thunk;
                }
                if (!name || strcmp(name, func_name) != 0) continue;

                // Already hooked this slot
                if ((void*)thunk->u1.Function == new_func) return true;

                DWORD old_protect = 0;
                if (!VirtualProtect(&thunk->u1.Function, sizeof(void*), PAGE_READWRITE, &old_protect)) {
                    return false;
                }
                // Keep first real original for trampoline (all modules share one g_orig_*)
                if (*orig_func == nullptr) {
                    *orig_func = (void*)thunk->u1.Function;
                }
                thunk->u1.Function = (ULONG_PTR)new_func;
                VirtualProtect(&thunk->u1.Function, sizeof(void*), old_protect, &old_protect);
                g_stats.total_iat_hits.fetch_add(1, std::memory_order_relaxed);
                return true;
            }
        }
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        return false;
    }
    return false;
}

struct HookTarget {
    const char* dll;   // e.g. "gdi32.dll"
    const char* name;  // e.g. "TextOutW"
    void* hooked;
    void** orig;
};

static int PatchAllModulesIAT(const HookTarget* targets, int count, HMODULE single_module = nullptr) {
    // single_module != nullptr: patch only that module (used by Ldr callback
    // for late-loaded DLLs). nullptr: walk all loaded modules (original behavior).

    // Skip system DLLs that rarely import TextOut (and our own if named moon_hook)
    auto skip_module = [](HMODULE m) -> bool {
        wchar_t path[MAX_PATH] = {};
        if (!GetModuleFileNameW(m, path, MAX_PATH)) return false;
        // basename
        const wchar_t* base = path;
        for (const wchar_t* p = path; *p; ++p) {
            if (*p == L'\\' || *p == L'/') base = p + 1;
        }
        if (_wcsicmp(base, L"gdi32.dll") == 0) return true;
        if (_wcsicmp(base, L"gdi32full.dll") == 0) return true;
        if (_wcsicmp(base, L"user32.dll") == 0) return true;
        if (_wcsicmp(base, L"ntdll.dll") == 0) return true;
        if (_wcsicmp(base, L"kernel32.dll") == 0) return true;
        if (_wcsicmp(base, L"kernelbase.dll") == 0) return true;
        if (_wcsicmp(base, L"moon_hook.dll") == 0) return true;
        return false;
    };

    auto patch_one = [&](HMODULE m) -> int {
        if (skip_module(m)) return 0;
        g_stats.total_modules_scanned.fetch_add(1, std::memory_order_relaxed);
        int hits = 0;
        for (int i = 0; i < count; ++i) {
            if (PatchIAT(m, targets[i].dll, targets[i].name, targets[i].hooked,
                         targets[i].orig)) {
                ++hits;
            }
        }
        return hits;
    };

    if (single_module) {
        return patch_one(single_module);
    }

    HMODULE mods[1024];
    DWORD needed = 0;
    HANDLE proc = GetCurrentProcess();
    if (!EnumProcessModules(proc, mods, sizeof(mods), &needed)) {
        // Fallback: at least patch the main EXE
        HMODULE exe = GetModuleHandleW(nullptr);
        return exe ? patch_one(exe) : 0;
    }

    int nmods = (int)(needed / sizeof(HMODULE));
    if (nmods > 1024) nmods = 1024;

    int total_hits = 0;
    for (int mi = 0; mi < nmods; ++mi) {
        total_hits += patch_one(mods[mi]);
    }
    return total_hits;
}

// ============================================================================
// Hook installation/removal
// ============================================================================

static void ResolveOriginals() {
    HMODULE gdi32 = GetModuleHandleW(L"gdi32.dll");
    HMODULE user32 = GetModuleHandleW(L"user32.dll");
    if (!gdi32 || !user32) return;

    if (!g_orig_TextOutW) g_orig_TextOutW = (TextOutW_t)GetProcAddress(gdi32, "TextOutW");
    if (!g_orig_TextOutA) g_orig_TextOutA = (TextOutA_t)GetProcAddress(gdi32, "TextOutA");
    if (!g_orig_ExtTextOutW) g_orig_ExtTextOutW = (ExtTextOutW_t)GetProcAddress(gdi32, "ExtTextOutW");
    if (!g_orig_ExtTextOutA) g_orig_ExtTextOutA = (ExtTextOutA_t)GetProcAddress(gdi32, "ExtTextOutA");
    if (!g_orig_DrawTextW) g_orig_DrawTextW = (DrawTextW_t)GetProcAddress(user32, "DrawTextW");
    if (!g_orig_DrawTextA) g_orig_DrawTextA = (DrawTextA_t)GetProcAddress(user32, "DrawTextA");
    if (!g_orig_DrawTextExW) g_orig_DrawTextExW = (DrawTextExW_t)GetProcAddress(user32, "DrawTextExW");
    if (!g_orig_DrawTextExA) g_orig_DrawTextExA = (DrawTextExA_t)GetProcAddress(user32, "DrawTextExA");
}

// Get the current HookTarget table (used by Ldr callback + verify function).
// Returns a pointer to a static array; caller must not retain across calls.
static const HookTarget* GetHookTargets(int* out_count) {
    static const HookTarget targets[] = {
        {"gdi32.dll", "TextOutW", (void*)HookedTextOutW, (void**)&g_orig_TextOutW},
        {"gdi32.dll", "TextOutA", (void*)HookedTextOutA, (void**)&g_orig_TextOutA},
        {"gdi32.dll", "ExtTextOutW", (void*)HookedExtTextOutW, (void**)&g_orig_ExtTextOutW},
        {"gdi32.dll", "ExtTextOutA", (void*)HookedExtTextOutA, (void**)&g_orig_ExtTextOutA},
        {"user32.dll", "DrawTextW", (void*)HookedDrawTextW, (void**)&g_orig_DrawTextW},
        {"user32.dll", "DrawTextA", (void*)HookedDrawTextA, (void**)&g_orig_DrawTextA},
        {"user32.dll", "DrawTextExW", (void*)HookedDrawTextExW, (void**)&g_orig_DrawTextExW},
        {"user32.dll", "DrawTextExA", (void*)HookedDrawTextExA, (void**)&g_orig_DrawTextExA},
    };
    if (out_count) *out_count = (int)(sizeof(targets) / sizeof(targets[0]));
    return targets;
}

// 4a: Loader notification callback — fires whenever any DLL is mapped into
// the process. We re-run PatchAllModulesIAT against the new module so its
// TextOut/DrawText imports get hooked too.
//
// CAVEAT: The loader holds the loader lock during this callback. We must
// NOT call any function that itself may acquire the loader lock (e.g.
// LoadLibrary, GetProcAddress on not-yet-loaded DLLs, SyncMgr RPC).
// PatchAllModulesIAT only touches already-mapped module memory + VirtualProtect,
// both of which are safe under the loader lock.
static VOID CALLBACK LoaderNotificationCallback(
    ULONG NotificationReason,
    PLDR_DLL_NOTIFICATION_DATA NotificationData,
    PVOID /*Context*/)
{
    if (NotificationReason != LDR_DLL_NOTIFICATION_REASON_LOADED) return;
    if (!g_hooks_installed) return;
    if (!NotificationData) return;

    PVOID base = NotificationData->Loaded.DllBase;
    if (!base) return;

    HMODULE mod = (HMODULE)base;
    int count = 0;
    const HookTarget* targets = GetHookTargets(&count);
    int hits = PatchAllModulesIAT(targets, count, mod);
    if (hits > 0) {
        g_stats.late_loaded_patched.fetch_add(1, std::memory_order_relaxed);
        // Log basename so we can verify late-loaded coverage from host side.
        wchar_t path[MAX_PATH] = {};
        if (GetModuleFileNameW(mod, path, MAX_PATH)) {
            const wchar_t* base_name = path;
            for (const wchar_t* p = path; *p; ++p) {
                if (*p == L'\\' || *p == L'/') base_name = p + 1;
            }
            char narrow[MAX_PATH] = {};
            WideCharToMultiByte(CP_UTF8, 0, base_name, -1, narrow, sizeof(narrow), nullptr, nullptr);
            // Send a low-volume marker through shared memory so host sees which
            // late DLLs got hooked. We use a reserved code_page sentinel
            // (0xFFFFFFFF) so the host can route these to a debug channel
            // instead of polluting the captured-text stream. code_page=0 would
            // collide with UTF-16 wide-string semantics.
            static const uint32_t LATE_LOADED_MARKER_CODE_PAGE = 0xFFFFFFFFu;
            SendTextToHost(narrow, strlen(narrow), 0, 0, LATE_LOADED_MARKER_CODE_PAGE);
        }
    }
}

// 4a: Resolve LdrRegisterDllNotification / LdrUnregisterDllNotification from
// ntdll. Returns true if both pointers are populated. Idempotent.
static bool ResolveLdrNotificationApi() {
    if (g_pLdrRegister && g_pLdrUnregister) return true;
    if (!g_ntdll) {
        g_ntdll = GetModuleHandleW(L"ntdll.dll");
        if (!g_ntdll) return false;
    }
    g_pLdrRegister = (LdrRegisterDllNotification_t)
        GetProcAddress(g_ntdll, "LdrRegisterDllNotification");
    g_pLdrUnregister = (LdrUnregisterDllNotification_t)
        GetProcAddress(g_ntdll, "LdrUnregisterDllNotification");
    return g_pLdrRegister && g_pLdrUnregister;
}

// 4c: Walk all modules and count IAT slots still pointing at our hook
// functions. After UninstallHooks, this should be 0. Non-zero means the
// rollback missed slots (e.g. a module was loaded after the initial
// sweep but before Ldr registration, or VirtualProtect failed).
//
// Returns the count of still-hooked slots.
struct VerifyResult {
    int still_hooked_slots;
    int modules_scanned;
};

static VerifyResult VerifyIatRollback() {
    VerifyResult result = {0, 0};

    HMODULE mods[1024];
    DWORD needed = 0;
    HANDLE proc = GetCurrentProcess();
    if (!EnumProcessModules(proc, mods, sizeof(mods), &needed)) return result;
    int nmods = (int)(needed / sizeof(HMODULE));
    if (nmods > 1024) nmods = 1024;

    void* hooked_ptrs[] = {
        (void*)HookedTextOutW, (void*)HookedTextOutA,
        (void*)HookedExtTextOutW, (void*)HookedExtTextOutA,
        (void*)HookedDrawTextW, (void*)HookedDrawTextA,
        (void*)HookedDrawTextExW, (void*)HookedDrawTextExA,
    };
    constexpr int N_HOOKS = sizeof(hooked_ptrs) / sizeof(hooked_ptrs[0]);

    for (int mi = 0; mi < nmods; ++mi) {
        HMODULE module = mods[mi];
        if (!module) continue;

        __try {
            PIMAGE_DOS_HEADER dos = (PIMAGE_DOS_HEADER)module;
            if (dos->e_magic != IMAGE_DOS_SIGNATURE) continue;
            PIMAGE_NT_HEADERS nt = (PIMAGE_NT_HEADERS)((uint8_t*)module + dos->e_lfanew);
            if (nt->Signature != IMAGE_NT_SIGNATURE) continue;
            DWORD import_rva =
                nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT].VirtualAddress;
            if (import_rva == 0) continue;

            PIMAGE_IMPORT_DESCRIPTOR desc =
                (PIMAGE_IMPORT_DESCRIPTOR)((uint8_t*)module + import_rva);
            for (; desc->Name; ++desc) {
                PIMAGE_THUNK_DATA thunk =
                    (PIMAGE_THUNK_DATA)((uint8_t*)module + desc->FirstThunk);
                for (; thunk->u1.Function; ++thunk) {
                    void* fn = (void*)thunk->u1.Function;
                    for (int i = 0; i < N_HOOKS; ++i) {
                        if (fn == hooked_ptrs[i]) {
                            ++result.still_hooked_slots;
                        }
                    }
                }
            }
            ++result.modules_scanned;
        } __except (EXCEPTION_EXECUTE_HANDLER) {
            continue;
        }
    }
    return result;
}

static bool InstallHooks() {
    if (g_hooks_installed) return true;

    ResolveOriginals();
    if (!g_orig_TextOutW && !g_orig_ExtTextOutW && !g_orig_DrawTextW) return false;

    int count = 0;
    const HookTarget* targets = GetHookTargets(&count);
    int hits = PatchAllModulesIAT(targets, count);

    // 4a: register loader notification so late-loaded DLLs get patched too.
    // Failure to register is non-fatal — initial sweep still covers modules
    // loaded at injection time.
    if (ResolveLdrNotificationApi()) {
        PVOID cookie = nullptr;
        LONG status = g_pLdrRegister(0, LoaderNotificationCallback, nullptr, &cookie);
        if (status == 0 /* STATUS_SUCCESS */) {
            g_ldr_cookie = cookie;
        }
    }

    // 4c: clear ejecting flag (in case this is a re-install after eject).
    g_ejecting.store(false, std::memory_order_release);

    // Success if we got at least one IAT hit OR have GetProcAddress originals
    // (hooks may still fire after late-loaded modules if re-installed via HookInstall).
    g_hooks_installed = true;
    return hits > 0 || (g_orig_TextOutW != nullptr || g_orig_ExtTextOutW != nullptr
                        || g_orig_DrawTextW != nullptr);
}

// 4d-rollback: forward declaration — defined later (near InlineHookCtx).
// Restores original prologue bytes for every live H-Code inline hook so the
// DLL can be ejected without leaving dangling trampolines.
// Wrapped in extern "C" to match the definition's language linkage (the
// definition lives inside the extern "C" block below).
extern "C" {
    static void UninstallInlineHooks();
}

/// Restore IAT slots from hooked functions back to g_orig_*.
/// 4c: Returns true if verification confirms all slots rolled back.
static bool UninstallHooks() {
    if (!g_hooks_installed) return true;
    ResolveOriginals();

    // 4c: gate SendText BEFORE touching IAT. Once set, any in-flight hook
    // callback drops its message instead of writing to potentially-torn
    // shared memory during teardown.
    g_ejecting.store(true, std::memory_order_release);

    // 4a: unregister loader callback first so no new module patches happen
    // while we're rolling back.
    if (g_ldr_cookie && g_pLdrUnregister) {
        g_pLdrUnregister(g_ldr_cookie);
        g_ldr_cookie = nullptr;
    }

    // 4d-rollback: restore inline-hook prologues BEFORE touching IAT. Inline
    // hooks are the most dangerous to leave dangling (a call into a freed
    // trampoline crashes the target immediately), so they go first. New calls
    // into patched functions now flow through the original prologue again.
    UninstallInlineHooks();

    // Patch hooked slots back to originals by treating g_orig as the "new" target
    // and temporarily clearing *orig so PatchIAT does not overwrite g_orig.
    auto restore_one = [](const char* dll, const char* name, void* orig) {
        if (!orig) return;
        void* scratch = orig; // already the real API
        // Walk modules and replace Hooked* with orig where present.
        HMODULE mods[1024];
        DWORD needed = 0;
        HANDLE proc = GetCurrentProcess();
        if (!EnumProcessModules(proc, mods, sizeof(mods), &needed)) {
            HMODULE exe = GetModuleHandleW(nullptr);
            if (exe) {
                void* dummy = nullptr;
                // Force-write orig into IAT by patching any slot currently pointing at our hooks
                // Reuse PatchIAT with new_func=orig; dummy orig storage ignored if already set.
                dummy = orig;
                PatchIAT(exe, dll, name, orig, &dummy);
            }
            return;
        }
        int nmods = (int)(needed / sizeof(HMODULE));
        if (nmods > 1024) nmods = 1024;
        for (int mi = 0; mi < nmods; ++mi) {
            void* dummy = orig;
            PatchIAT(mods[mi], dll, name, orig, &dummy);
        }
    };

    restore_one("gdi32.dll", "TextOutW", (void*)g_orig_TextOutW);
    restore_one("gdi32.dll", "TextOutA", (void*)g_orig_TextOutA);
    restore_one("gdi32.dll", "ExtTextOutW", (void*)g_orig_ExtTextOutW);
    restore_one("gdi32.dll", "ExtTextOutA", (void*)g_orig_ExtTextOutA);
    restore_one("user32.dll", "DrawTextW", (void*)g_orig_DrawTextW);
    restore_one("user32.dll", "DrawTextA", (void*)g_orig_DrawTextA);
    restore_one("user32.dll", "DrawTextExW", (void*)g_orig_DrawTextExW);
    restore_one("user32.dll", "DrawTextExA", (void*)g_orig_DrawTextExA);

    g_hooks_installed = false;

    // 4c: verify rollback. If any IAT slot still points at our hook function,
    // the target process may crash on next call (calling into unmapped DLL).
    // Caller (host) can query HookGetStats to see verify_failed count and
    // decide whether to force-kill the target.
    VerifyResult vr = VerifyIatRollback();
    if (vr.still_hooked_slots > 0) {
        // Best-effort: do one more sweep using the shared targets table.
        // If still failing, the host must kill the target process to avoid crash.
        int count = 0;
        const HookTarget* targets = GetHookTargets(&count);
        HMODULE mods[1024];
        DWORD needed = 0;
        if (EnumProcessModules(GetCurrentProcess(), mods, sizeof(mods), &needed)) {
            int nmods = (int)(needed / sizeof(HMODULE));
            if (nmods > 1024) nmods = 1024;
            for (int mi = 0; mi < nmods; ++mi) {
                for (int i = 0; i < count; ++i) {
                    if (targets[i].orig && *targets[i].orig) {
                        void* dummy = *targets[i].orig;
                        PatchIAT(mods[mi], targets[i].dll, targets[i].name,
                                 *targets[i].orig, &dummy);
                    }
                }
            }
        }
        vr = VerifyIatRollback();
    }

    return vr.still_hooked_slots == 0;
}

// ============================================================================
// DLL Entry Point
// ============================================================================

BOOL APIENTRY DllMain(HMODULE hModule, DWORD reason, LPVOID lpReserved) {
    switch (reason) {
    case DLL_PROCESS_ATTACH:
        g_self_module = hModule;
        DisableThreadLibraryCalls(hModule);
        if (InitSharedMemory()) {
            InstallHooks();
        }
        break;

    case DLL_PROCESS_DETACH:
        // If FreeLibraryAndExitThread already cleaned up, this is a no-op.
        if (g_hooks_installed) {
            UninstallHooks();
        }
        CleanupSharedMemory();
        break;

    case DLL_THREAD_ATTACH:
    case DLL_THREAD_DETACH:
        break;
    }
    return TRUE;
}

// ============================================================================
// Exported functions for external control
// ============================================================================

extern "C" {

    __declspec(dllexport) BOOL __cdecl HookInstall() {
        return InstallHooks() ? TRUE : FALSE;
    }

    __declspec(dllexport) BOOL __cdecl HookIsInstalled() {
        return g_hooks_installed ? TRUE : FALSE;
    }

    __declspec(dllexport) const char* __cdecl HookGetProcessName() {
        if (!g_shared_data) return "";
        return g_shared_data->process_name;
    }

    /// 4c: Host calls this via CreateRemoteThread after resolving export in remote module.
    /// Restores IAT, cleans shared memory, then unloads this DLL from the target process.
    ///
    /// Returns:
    ///   0 = full success (IAT rollback verified clean)
    ///   1 = IAT rollback left dangling slots (host should kill target to avoid crash)
    ///   2 = cleanup completed but verify step was skipped (already uninstalled)
    __declspec(dllexport) DWORD __stdcall HookUninstall(LPVOID) {
        bool clean = UninstallHooks();
        CleanupSharedMemory();
        DWORD exit_code = clean ? 0 : 1;
        if (g_self_module) {
            FreeLibraryAndExitThread(g_self_module, exit_code);
        }
        return exit_code;
    }

    // ========================================================================
    // 4b: HookStats query — host reads these to verify hook coverage.
    // Returns a static JSON string (valid until next call).
    // ========================================================================
    __declspec(dllexport) const char* __cdecl HookGetStats() {
        static char json_buf[512];
        snprintf(json_buf, sizeof(json_buf),
            "{\"modulesScanned\":%llu,\"iatHits\":%llu,\"lateLoadedPatched\":%llu,"
            "\"sendTextCalls\":%llu,\"sendTextFiltered\":%llu,"
            "\"sendTextEjectBlocked\":%llu,\"inlineHooks\":%llu,"
            "\"hooksInstalled\":%s,\"ldrCookie\":%s}",
            (unsigned long long)g_stats.total_modules_scanned.load(std::memory_order_relaxed),
            (unsigned long long)g_stats.total_iat_hits.load(std::memory_order_relaxed),
            (unsigned long long)g_stats.late_loaded_patched.load(std::memory_order_relaxed),
            (unsigned long long)g_stats.send_text_calls.load(std::memory_order_relaxed),
            (unsigned long long)g_stats.send_text_filtered.load(std::memory_order_relaxed),
            (unsigned long long)g_stats.send_text_eject_blocked.load(std::memory_order_relaxed),
            (unsigned long long)g_stats.inline_hooks_installed.load(std::memory_order_relaxed),
            g_hooks_installed ? "true" : "false",
            g_ldr_cookie ? "true" : "false");
        return json_buf;
    }

    // ========================================================================
    // 4d: H-Code inline hook installer.
    //
    // Hooks a specific address (typically a function inside the target
    // process's own code section) by overwriting the first bytes with a
    // JMP to our trampoline. The trampoline reads text per `data_offset`
    // and `deref_levels`, calls SendTextToHost, then continues execution
    // at the saved original bytes.
    //
    // This is the standard Luna Hook approach for H-Codes that target
    // custom rendering functions not reachable via IAT patching.
    //
    // Parameters:
    //   target_addr    - absolute address in target process to hook
    //   data_offset    - byte offset from target_addr where text pointer lives
    //                    (positive = after addr, negative = before addr)
    //   deref_levels   - how many pointer dereferences to follow (0 = read
    //                    bytes directly at offset, 1 = read pointer then
    //                    read string, 2 = double deref, etc.)
    //   code_page      - code page for ANSI→UTF-8 conversion (0 = UTF-16,
    //                    932 = Shift-JIS, 936 = GBK, etc.)
    //   text_type      - 0 = null-terminated string, 1 = length-prefixed
    //                    (4-byte length header before string), 2 = fixed
    //                    length (read `deref_levels` bytes as length)
    //
    // Returns:
    //   0 = success
    //   1 = invalid parameters
    //   2 = VirtualProtect failed (cannot make target writable)
    //   3 = trampoline allocation failed
    //   4 = unsupported architecture (x86 only supported on x86 build)
    // ========================================================================

    // 4d: Per-hook context. Allocated on heap; lives until DLL unload.
    // We keep a singly-linked list so UninstallHooks could (in a future
    // iteration) walk and unpatch them. For now, all inline hooks are
    // torn down implicitly when the DLL is unloaded.
    struct InlineHookCtx {
        void* target_addr;
        unsigned char saved_bytes[16];   // original first 16 bytes
        size_t patch_size;               // bytes overwritten (14 on x64)
        void* trampoline;                // allocated via VirtualAlloc
        int32_t data_offset;
        uint32_t deref_levels;
        uint32_t code_page;
        uint32_t text_type;
        InlineHookCtx* next;
    };

    static InlineHookCtx* g_inline_hooks = nullptr;
    static CRITICAL_SECTION g_inline_lock;
    static bool g_inline_lock_inited = false;

    static void EnsureInlineLock() {
        if (!g_inline_lock_inited) {
            InitializeCriticalSection(&g_inline_lock);
            g_inline_lock_inited = true;
        }
    }

    // 4d-rollback: Restore original prologue bytes for every live inline hook.
    //
    // WHY THIS EXISTS: HookInstallAtAddress overwrites the target function's
    // first bytes with a JMP into a trampoline that calls ReadAndSend (which
    // lives inside this DLL). If the DLL is ejected (FreeLibrary) while those
    // patches are still in place, the next call into the target function
    // jumps into unmapped memory and crashes the host process.
    //
    // This walks g_inline_hooks and writes the saved_bytes back so new calls
    // bypass the trampoline entirely. We do NOT free the trampoline or ctx
    // nodes: a thread currently executing inside the trampoline must still be
    // able to return through `call ReadAndSend` → saved bytes → jmp target+N.
    // Freeing would create a use-after-free. The ~256 bytes per hook leak
    // harmlessly (bounded by the number of H-Code installs) and are reclaimed
    // with the process on exit.
    //
    // Idempotent: a node whose target_addr has been nulled is skipped.
    static void UninstallInlineHooks() {
        EnsureInlineLock();
        EnterCriticalSection(&g_inline_lock);

        InlineHookCtx* node = g_inline_hooks;
        while (node) {
            if (node->target_addr && node->patch_size > 0 && node->patch_size <= sizeof(node->saved_bytes)) {
                __try {
                    DWORD old_protect = 0;
                    if (VirtualProtect(node->target_addr, node->patch_size,
                                       PAGE_EXECUTE_READWRITE, &old_protect)) {
                        memcpy(node->target_addr, node->saved_bytes, node->patch_size);
                        DWORD restored = 0;
                        VirtualProtect(node->target_addr, node->patch_size,
                                       old_protect, &restored);
                        FlushInstructionCache(GetCurrentProcess(),
                                              node->target_addr, node->patch_size);
                    }
                } __except (EXCEPTION_EXECUTE_HANDLER) {
                    // Best-effort: a page may have been unmapped by the target.
                    // Skip it; the host will see the dangling-slot count.
                }
                // Mark unpatched so a second UninstallHooks pass is a no-op.
                node->target_addr = nullptr;
            }
            node = node->next;
        }

        LeaveCriticalSection(&g_inline_lock);
    }

    // 4d: The trampoline body. Reads text per ctx params, calls SendTextToHost,
    // then jumps back to (target_addr + patch_size) to continue execution.
    //
    // We can't easily write this as a function in C++ because it needs to
    // execute the SAVED bytes of the original function (which may contain
    // relative addresses that would be wrong if copied naively).
    //
    // PRAGMATIC APPROACH: We allocate an executable trampoline that:
    //   1. Pushes all registers
    //   2. Calls a C function (ReadAndSend) with the target address as arg
    //   3. Pops registers
    //   4. Copies the saved bytes verbatim
    //   5. Jumps back to (target + patch_size)
    //
    // RISK: If saved bytes contain a RIP-relative instruction, copying them
    // to a new address will compute wrong targets. For function prologues
    // (mov rbp, rsp; sub rsp, X; push regs) this is safe. For functions
    // starting with a call/jmp to a relative address, this WILL break.
    //
    // The host should verify the target function's first bytes are
    // position-independent before calling HookInstallAtAddress.
    // 4d-helper: reads text from the target per ctx params. Split out of
    // ReadAndSend so the __try block contains no C++ objects with destructors
    // (MSVC C2712 forbids __try in functions requiring object unwinding, even
    // under /EHa in newer toolchains). This helper holds the std::string; the
    // caller wraps the invocation in __try. /EHa ensures an SEH exception
    // raised here still unwinds std::string correctly before reaching the
    // caller's __except.
    //
    // Writes UTF-8 text into `buf` (up to buf_size-1 bytes, NUL-terminated)
    // and returns the byte length (0 = no text produced).
    static size_t ReadTargetText(void* target_addr, InlineHookCtx* ctx,
                                 char* buf, size_t buf_size) {
        if (!ctx || !target_addr || buf_size == 0) return 0;

        uint8_t* src = (uint8_t*)target_addr + ctx->data_offset;
        std::string text;

        if (ctx->deref_levels == 0) {
            // Read bytes directly (treat as raw bytes, not a pointer)
            if (ctx->text_type == 0) {
                // null-terminated
                const char* s = (const char*)src;
                text = std::string(s, strnlen_s(s, 4096));
            } else if (ctx->text_type == 1) {
                // length-prefixed: 4-byte length header before string
                uint32_t len = *(uint32_t*)src;
                if (len > 0 && len <= 4096) {
                    text = std::string((const char*)(src + 4), len);
                }
            }
        } else {
            // Follow pointer dereferences
            void* ptr = *(void**)src;
            for (uint32_t i = 1; i < ctx->deref_levels && ptr; ++i) {
                ptr = *(void**)ptr;
            }
            if (ptr) {
                if (ctx->code_page == 0) {
                    // UTF-16
                    const wchar_t* wstr = (const wchar_t*)ptr;
                    size_t wlen = wcsnlen_s(wstr, 2048);
                    text = WideToUtf8(wstr, (int)wlen);
                } else {
                    // ANSI with given code page
                    const char* astr = (const char*)ptr;
                    size_t alen = strnlen_s(astr, 4096);
                    text = AnsiToUtf8(astr, (int)alen, ctx->code_page);
                }
            }
        }

        if (text.empty()) return 0;

        size_t copy_len = text.size();
        if (copy_len >= buf_size) copy_len = buf_size - 1;
        memcpy(buf, text.c_str(), copy_len);
        buf[copy_len] = '\0';
        return copy_len;
    }

    static void ReadAndSend(void* target_addr, InlineHookCtx* ctx) {
        if (!ctx || !target_addr) return;

        // POD-only inside __try so MSVC accepts it (no C++ unwinding needed).
        // The actual text extraction (with std::string) lives in ReadTargetText.
        char buf[8192];
        size_t len = 0;
        __try {
            len = ReadTargetText(target_addr, ctx, buf, sizeof(buf));
            if (len > 0) {
                SendTextToHost(buf, len, 0, 0, ctx->code_page);
            }
        } __except (EXCEPTION_EXECUTE_HANDLER) {
            // Swallow — bad pointer in target's data, not our problem.
        }
    }

    __declspec(dllexport) int __cdecl HookInstallAtAddress(
        void* target_addr,
        int32_t data_offset,
        uint32_t deref_levels,
        uint32_t code_page,
        uint32_t text_type)
    {
        if (!target_addr) return 1;

        EnsureInlineLock();
        EnterCriticalSection(&g_inline_lock);

#ifdef _WIN64
        // x64: 14-byte patch (mov rax, imm64; jmp rax)
        const size_t patch_size = 14;
#else
        // x86: 5-byte relative jmp (e9 xx xx xx xx)
        const size_t patch_size = 5;
#endif

        // Allocate context
        InlineHookCtx* ctx = new InlineHookCtx{};
        ctx->target_addr = target_addr;
        ctx->patch_size = patch_size;
        ctx->data_offset = data_offset;
        ctx->deref_levels = deref_levels;
        ctx->code_page = code_page;
        ctx->text_type = text_type;
        ctx->next = nullptr;

        // Save original bytes
        memcpy(ctx->saved_bytes, target_addr, patch_size);

        // Allocate trampoline near target (within 2GB for x86 rel32 jmp)
        // Trampoline layout:
        //   pushfq                          ; save flags
        //   push rax/rcx/rdx/r8..r15        ; save volatile regs (simplified: just rax,rcx,rdx,r8,r9,r10,r11)
        //   lea rcx, [target_addr]          ; arg 0 = target_addr
        //   lea rdx, [ctx]                  ; arg 1 = ctx
        //   mov rax, &ReadAndSend
        //   call rax
        //   pop r11..rax                    ; restore
        //   popfq
        //   <saved bytes>                   ; original instructions
        //   mov rax, target_addr + patch_size
        //   jmp rax

        // For simplicity + safety, allocate enough space (256 bytes)
        const size_t trampoline_size = 256;
        ctx->trampoline = VirtualAlloc(nullptr, trampoline_size,
            MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
        if (!ctx->trampoline) {
            delete ctx;
            LeaveCriticalSection(&g_inline_lock);
            return 3;
        }

        // Build trampoline body (architecture-specific)
        uint8_t* code = (uint8_t*)ctx->trampoline;

#ifdef _WIN64
        // x64 trampoline:
        //   9c                   pushfq
        //   50 51 52 53 56 57    push rax,rcx,rdx,rbx,rsi,rdi
        //   41 50..41 57          push r8-r15 (8 pushes)
        //   48 b9 <target_addr>   mov rcx, target_addr
        //   48 ba <ctx_ptr>       mov rdx, ctx
        //   48 b8 <readsend>      mov rax, &ReadAndSend
        //   ff d0                 call rax
        //   41 5f..41 58          pop r15-r8 (reverse order)
        //   5f 5e 5b 5a 59 58     pop rdi,rsi,rbx,rdx,rcx,rax
        //   9d                    popfq
        //   <saved bytes>         ; 14 bytes of original code
        //   48 b8 <target+14>     mov rax, target+14
        //   ff e0                 jmp rax
        size_t off = 0;
        code[off++] = 0x9c; // pushfq
        code[off++] = 0x50; code[off++] = 0x51; code[off++] = 0x52; code[off++] = 0x53;
        code[off++] = 0x56; code[off++] = 0x57;
        code[off++] = 0x41; code[off++] = 0x50; // push r8
        code[off++] = 0x41; code[off++] = 0x51; // push r9
        code[off++] = 0x41; code[off++] = 0x52; // push r10
        code[off++] = 0x41; code[off++] = 0x53; // push r11
        // mov rcx, target_addr
        code[off++] = 0x48; code[off++] = 0xb9;
        *(uint64_t*)(code + off) = (uint64_t)target_addr; off += 8;
        // mov rdx, ctx
        code[off++] = 0x48; code[off++] = 0xba;
        *(uint64_t*)(code + off) = (uint64_t)ctx; off += 8;
        // mov rax, &ReadAndSend
        code[off++] = 0x48; code[off++] = 0xb8;
        *(uint64_t*)(code + off) = (uint64_t)&ReadAndSend; off += 8;
        // sub rsp, 0x28 (40 bytes: 32 shadow + 8 for 16-byte alignment after 11 pushes)
        code[off++] = 0x48; code[off++] = 0x83; code[off++] = 0xec; code[off++] = 0x28;
        // call rax
        code[off++] = 0xff; code[off++] = 0xd0;
        // add rsp, 0x28
        code[off++] = 0x48; code[off++] = 0x83; code[off++] = 0xc4; code[off++] = 0x28;
        // pop r11..r8 (reverse order)
        code[off++] = 0x41; code[off++] = 0x5b; // pop r11
        code[off++] = 0x41; code[off++] = 0x5a; // pop r10
        code[off++] = 0x41; code[off++] = 0x59; // pop r9
        code[off++] = 0x41; code[off++] = 0x58; // pop r8
        // pop rdi,rsi,rbx,rdx,rcx,rax
        code[off++] = 0x5f; code[off++] = 0x5e; code[off++] = 0x5b;
        code[off++] = 0x5a; code[off++] = 0x59; code[off++] = 0x58;
        // popfq
        code[off++] = 0x9d;
        // Copy saved bytes
        memcpy(code + off, ctx->saved_bytes, patch_size);
        off += patch_size;
        // mov rax, target+patch_size
        code[off++] = 0x48; code[off++] = 0xb8;
        *(uint64_t*)(code + off) = (uint64_t)((uint8_t*)target_addr + patch_size); off += 8;
        // jmp rax
        code[off++] = 0xff; code[off++] = 0xe0;
#else
        // x86 trampoline: similar but 32-bit absolute addresses
        size_t off = 0;
        code[off++] = 0x9c; // pushfq
        code[off++] = 0x50; code[off++] = 0x51; code[off++] = 0x52; code[off++] = 0x53;
        code[off++] = 0x56; code[off++] = 0x57;
        // mov ecx, target_addr
        code[off++] = 0xb9;
        *(uint32_t*)(code + off) = (uint32_t)target_addr; off += 4;
        // mov edx, ctx
        code[off++] = 0xba;
        *(uint32_t*)(code + off) = (uint32_t)ctx; off += 4;
        // mov eax, &ReadAndSend
        code[off++] = 0xb8;
        *(uint32_t*)(code + off) = (uint32_t)&ReadAndSend; off += 4;
        // call eax
        code[off++] = 0xff; code[off++] = 0xd0;
        // pop regs
        code[off++] = 0x5f; code[off++] = 0x5e; code[off++] = 0x5b;
        code[off++] = 0x5a; code[off++] = 0x59; code[off++] = 0x58;
        // popfq
        code[off++] = 0x9d;
        // Copy saved bytes
        memcpy(code + off, ctx->saved_bytes, patch_size);
        off += patch_size;
        // jmp target+patch_size (e9 rel32)
        code[off++] = 0xe9;
        int32_t rel = (int32_t)((uint8_t*)target_addr + patch_size - (code + off + 4));
        *(int32_t*)(code + off) = rel; off += 4;
#endif

        // Patch target: write JMP to trampoline
        DWORD old_protect = 0;
        if (!VirtualProtect(target_addr, patch_size, PAGE_EXECUTE_READWRITE, &old_protect)) {
            VirtualFree(ctx->trampoline, 0, MEM_RELEASE);
            delete ctx;
            LeaveCriticalSection(&g_inline_lock);
            return 2;
        }

#ifdef _WIN64
        // x64: mov rax, trampoline; jmp rax (14 bytes)
        uint8_t* p = (uint8_t*)target_addr;
        p[0] = 0x48; p[1] = 0xb8; // mov rax, imm64
        *(uint64_t*)(p + 2) = (uint64_t)ctx->trampoline;
        p[10] = 0xff; p[11] = 0xe0; // jmp rax
        // bytes 12, 13 are padding (nop) to reach 14 bytes
        p[12] = 0x90; p[13] = 0x90;
#else
        // x86: jmp rel32 (5 bytes)
        uint8_t* p = (uint8_t*)target_addr;
        p[0] = 0xe9;
        int32_t rel_target = (int32_t)((uint8_t*)ctx->trampoline - (p + 5));
        *(int32_t*)(p + 1) = rel_target;
#endif

        VirtualProtect(target_addr, patch_size, old_protect, &old_protect);
        FlushInstructionCache(GetCurrentProcess(), target_addr, patch_size);

        // Add to linked list
        ctx->next = g_inline_hooks;
        g_inline_hooks = ctx;
        g_stats.inline_hooks_installed.fetch_add(1, std::memory_order_relaxed);

        LeaveCriticalSection(&g_inline_lock);
        return 0;
    }

    // ========================================================================
    // 4d-2: HookInstallAtAddressStruct — single-pointer variant for
    // CreateRemoteThread invocation.
    //
    // CreateRemoteThread can only pass one lpParameter (which lands in rcx
    // on x64). The 5-arg HookInstallAtAddress above is callable from the
    // target process's own code (e.g. from a hooked function), but not from
    // a remote thread. This variant takes a pointer to a struct that the
    // host allocates in the remote process and writes via WriteProcessMemory
    // before CreateRemoteThread.
    //
    // The struct layout must match `HookInstallParams` in
    // `src-tauri/src/hook_inject.rs`. Layout stability is enforced by
    // `#[repr(C)]` on both sides + a compile-time size assertion in the
    // host's `remote_install_h_code` function.
    // ========================================================================
    #pragma pack(push, 8)
    struct HookInstallParams {
        void*    target_addr;   // absolute VA in this process
        int32_t  data_offset;   // byte offset (positive=after, negative=before)
        uint32_t deref_levels;  // 0=read bytes, 1=single deref, 2+=chain
        uint32_t code_page;     // 0=UTF-16, 932=Shift-JIS, 936=GBK, ...
        uint32_t text_type;     // 0=null-terminated, 1=length-prefixed
    };
    #pragma pack(pop)

    __declspec(dllexport) int __cdecl HookInstallAtAddressStruct(HookInstallParams* params) {
        if (!params) return 1;
        return HookInstallAtAddress(
            params->target_addr,
            params->data_offset,
            params->deref_levels,
            params->code_page,
            params->text_type);
    }

} // extern "C"
