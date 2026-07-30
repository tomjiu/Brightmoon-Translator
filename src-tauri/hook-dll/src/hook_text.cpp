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

#pragma comment(lib, "psapi.lib")

// ============================================================================
// Shared Memory IPC
// ============================================================================

#pragma pack(push, 1)
struct SharedMemoryHeader {
    uint32_t magic;           // 0x4D4F4F4E ("MOON")
    uint32_t version;         // 1
    uint32_t write_offset;    // Current write position
    uint32_t buffer_size;     // Total buffer size
    uint32_t sequence;        // Sequence number for new messages
    char     process_name[64]; // Target process name
    uint8_t  reserved[40];    // Reserved for future use
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
        g_shared_data->version = 1;
        g_shared_data->buffer_size = SHARED_MEMORY_SIZE;
        g_shared_data->write_offset = sizeof(SharedMemoryHeader);

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
    if (!g_shared_data || !IsPrintableText(utf8_text, len)) return;

    EnterCriticalSection(&g_write_lock);

    uint32_t msg_size = sizeof(TextMessage) + (uint32_t)len;
    uint32_t total_size = msg_size;

    // Check if we need to wrap around
    uint32_t end_offset = g_shared_data->write_offset + total_size;
    if (end_offset > g_shared_data->buffer_size) {
        // Wrap to after header
        g_shared_data->write_offset = sizeof(SharedMemoryHeader);
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

static int PatchAllModulesIAT(const HookTarget* targets, int count) {
    HMODULE mods[1024];
    DWORD needed = 0;
    HANDLE proc = GetCurrentProcess();
    if (!EnumProcessModules(proc, mods, sizeof(mods), &needed)) {
        // Fallback: at least patch the main EXE
        HMODULE exe = GetModuleHandleW(nullptr);
        int hits = 0;
        if (exe) {
            for (int i = 0; i < count; ++i) {
                if (PatchIAT(exe, targets[i].dll, targets[i].name, targets[i].hooked, targets[i].orig)) {
                    ++hits;
                }
            }
        }
        return hits;
    }

    int nmods = (int)(needed / sizeof(HMODULE));
    if (nmods > 1024) nmods = 1024;

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

    int total_hits = 0;
    for (int mi = 0; mi < nmods; ++mi) {
        if (skip_module(mods[mi])) continue;
        for (int i = 0; i < count; ++i) {
            if (PatchIAT(mods[mi], targets[i].dll, targets[i].name, targets[i].hooked,
                         targets[i].orig)) {
                ++total_hits;
            }
        }
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

static bool InstallHooks() {
    if (g_hooks_installed) return true;

    ResolveOriginals();
    if (!g_orig_TextOutW && !g_orig_ExtTextOutW && !g_orig_DrawTextW) return false;

    const HookTarget targets[] = {
        {"gdi32.dll", "TextOutW", (void*)HookedTextOutW, (void**)&g_orig_TextOutW},
        {"gdi32.dll", "TextOutA", (void*)HookedTextOutA, (void**)&g_orig_TextOutA},
        {"gdi32.dll", "ExtTextOutW", (void*)HookedExtTextOutW, (void**)&g_orig_ExtTextOutW},
        {"gdi32.dll", "ExtTextOutA", (void*)HookedExtTextOutA, (void**)&g_orig_ExtTextOutA},
        {"user32.dll", "DrawTextW", (void*)HookedDrawTextW, (void**)&g_orig_DrawTextW},
        {"user32.dll", "DrawTextA", (void*)HookedDrawTextA, (void**)&g_orig_DrawTextA},
        {"user32.dll", "DrawTextExW", (void*)HookedDrawTextExW, (void**)&g_orig_DrawTextExW},
        {"user32.dll", "DrawTextExA", (void*)HookedDrawTextExA, (void**)&g_orig_DrawTextExA},
    };

    int hits = PatchAllModulesIAT(targets, (int)(sizeof(targets) / sizeof(targets[0])));
    // Success if we got at least one IAT hit OR have GetProcAddress originals (hooks may still
    // fire after late-loaded modules if re-installed via HookInstall).
    g_hooks_installed = true;
    return hits > 0 || (g_orig_TextOutW != nullptr || g_orig_ExtTextOutW != nullptr
                        || g_orig_DrawTextW != nullptr);
}

/// Restore IAT slots from hooked functions back to g_orig_* (best-effort).
static void UninstallHooks() {
    if (!g_hooks_installed) return;
    ResolveOriginals();

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

    /// Host calls this via CreateRemoteThread after resolving export in remote module.
    /// Restores IAT, cleans shared memory, then unloads this DLL from the target process.
    __declspec(dllexport) DWORD __stdcall HookUninstall(LPVOID) {
        UninstallHooks();
        CleanupSharedMemory();
        if (g_self_module) {
            FreeLibraryAndExitThread(g_self_module, 0);
        }
        return 0;
    }
}
