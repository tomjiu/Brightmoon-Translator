# Unsafe Code Guidelines for Moon Translator

## Overview

This document defines the guidelines for using `unsafe` code in the Moon Translator project. The project is a Tauri v2 desktop application that extensively uses Windows APIs for screen capture, clipboard operations, UI Automation, and DLL injection.

## When to Use Unsafe

### Acceptable Use Cases

1. **Win32 API Calls**: Direct calls to Windows APIs that require raw pointers
   - Screen capture (GDI functions)
   - Clipboard operations (OpenClipboard, GetClipboardData, etc.)
   - UI Automation (IUIAutomation, COM interfaces)
   - Window management (GetForegroundWindow, GetWindowText, etc.)
   - Input simulation (SendInput)

2. **FFI (Foreign Function Interface)**: Interfacing with C/C++ libraries
   - DLL injection (CreateRemoteThread, WriteProcessMemory)
   - Shared memory access (MapViewOfFile)

3. **Performance-Critical Code**: When safe alternatives would introduce unacceptable overhead
   - Raw pointer arithmetic for buffer processing

### Unacceptable Use Cases

1. **Bypassing Borrow Checker**: Never use unsafe to circumvent Rust's ownership rules
2. **Uninitialized Memory**: Always initialize memory before use
3. **Null Pointer Dereference**: Always check for null before dereferencing
4. **Data Races**: Never create data races through unsafe code

## Safety Invariant Documentation Requirements

Every `unsafe` block MUST include a `// SAFETY:` comment that explains:

1. **Why unsafe is required**: The specific operation that requires unsafe
2. **Preconditions**: What the caller must ensure before the unsafe block
3. **Postconditions**: What the unsafe block guarantees after execution
4. **Invariant Maintenance**: How the code maintains safety invariants

### Template

```rust
// SAFETY: This unsafe block is required for [REASON].
// Preconditions:
//   - [List preconditions]
// Postconditions:
//   - [List postconditions]
// Invariants:
//   - [List invariants maintained]
unsafe {
    // ... unsafe code ...
}
```

## Code Review Checklist

Before merging any PR containing unsafe code, verify:

- [ ] All unsafe blocks have SAFETY comments
- [ ] Null pointer checks are present where needed
- [ ] Error handling is complete (no unwrap() in unsafe blocks)
- [ ] Resources are properly cleaned up (RAII pattern)
- [ ] Thread safety is maintained
- [ ] No undefined behavior is possible

## Module-Specific Guidelines

### hook_monitor.rs

**Purpose**: Monitors foreground window text changes using multiple capture sources.

**Key Unsafe Operations**:
- Win32 API calls for window management
- COM initialization and UI Automation
- Clipboard access
- Message loop management

**Safety Invariants**:
- All Win32 handles are properly released on error paths
- COM is initialized per-thread (apartment threading)
- Message loops exit cleanly via WM_QUIT

### selection/clipboard.rs

**Purpose**: Captures selected text via clipboard simulation.

**Key Unsafe Operations**:
- Clipboard open/close/save/restore
- Input simulation (Ctrl+C)
- Global memory allocation

**Safety Invariants**:
- Clipboard is always closed after opening
- Original clipboard content is restored
- Global memory is properly locked/unlocked

### selection/uiautomation.rs

**Purpose**: Reads selected text via Windows UI Automation.

**Key Unsafe Operations**:
- COM object creation and interface casting
- UIA tree traversal
- SAFEARRAY manipulation

**Safety Invariants**:
- COM is properly initialized before use
- UIA objects are reference-counted (COM lifetime)
- Tree traversal has depth limits to prevent stack overflow

### hook_inject.rs

**Purpose**: Manages DLL injection into target processes.

**Key Unsafe Operations**:
- Process memory allocation (VirtualAllocEx)
- Remote thread creation (CreateRemoteThread)
- Shared memory mapping (MapViewOfFile)
- Raw pointer dereference for shared memory

**Safety Invariants**:
- Process handles are closed after use
- Memory is freed on error paths
- Shared memory magic number is validated
- Bounds checking on message parsing

### commands/capture.rs

**Purpose**: Screen capture using GDI.

**Key Unsafe Operations**:
- GDI object creation and cleanup
- Device context management
- Bitmap operations

**Safety Invariants**:
- All GDI objects are deleted on error paths
- Device contexts are released properly
- Bitmap data is copied before cleanup

### commands/window.rs

**Purpose**: Window management and cursor operations.

**Key Unsafe Operations**:
- GetCursorPos
- GetSystemMetrics
- GetWindowRect

**Safety Invariants**:
- Output buffers are properly sized
- Return values are checked for errors

## Safe Wrapper Patterns

When possible, create safe wrappers around unsafe code:

```rust
/// Safe wrapper for GetCursorPos
fn get_cursor_position() -> Option<(i32, i32)> {
    #[repr(C)]
    struct POINT { x: i32, y: i32 }

    extern "system" {
        fn GetCursorPos(lpPoint: *mut POINT) -> i32;
    }

    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: POINT is a valid output buffer, GetCursorPos is thread-safe
    let result = unsafe { GetCursorPos(&mut point) };
    if result != 0 {
        Some((point.x, point.y))
    } else {
        None
    }
}
```

## Testing Unsafe Code

1. **Unit Tests**: Test unsafe wrappers with valid and invalid inputs
2. **Integration Tests**: Test full workflows (clipboard save/restore, etc.)
3. **Fuzz Testing**: For input parsing code (shared memory messages)
4. **Memory Sanitizers**: Run with AddressSanitizer and MemorySanitizer

## Common Pitfalls

### 1. Forgetting to Release Resources

```rust
// BAD: Resource leak on error
let handle = OpenProcess(...)?;
// ... code that might fail ...
CloseHandle(handle);

// GOOD: RAII wrapper or explicit cleanup
let handle = OpenProcess(...)?;
let result = (|| -> Result<(), Error> {
    // ... code that might fail ...
    Ok(())
})();
CloseHandle(handle);
result?;
```

### 2. Null Pointer Dereference

```rust
// BAD: Potential null dereference
let ptr = GetClipboardData(CF_TEXT);
let data = *ptr;  // CRASH if ptr is null

// GOOD: Check for null
let ptr = GetClipboardData(CF_TEXT);
if ptr.is_null() {
    return Err("No clipboard data".into());
}
let data = *ptr;
```

### 3. Data Races

```rust
// BAD: Unsynchronized access to shared state
static mut COUNTER: u32 = 0;

// GOOD: Use atomic operations
static COUNTER: AtomicU32 = AtomicU32::new(0);
```

## References

- [The Rustonomicon](https://doc.rust-lang.org/nomicon/)
- [Rust Unsafe Code Guidelines](https://rust-lang.github.io/unsafe-code-guidelines/)
- [Windows API Documentation](https://docs.microsoft.com/en-us/windows/win32/)

## Audit History

| Date | Auditor | Changes |
|------|---------|---------|
| 2026-05-25 | Initial Audit | Documented all unsafe blocks, added safety invariants |
