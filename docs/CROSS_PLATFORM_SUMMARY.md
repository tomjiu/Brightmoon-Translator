# Cross-Platform Support - Summary

## Overview

This document summarizes the cross-platform analysis for Moon Translator and provides actionable recommendations for macOS and Linux support.

## Current State

The application is **heavily Windows-dependent** with the following critical components using Windows-only APIs:

| Component | Windows API | Impact |
|-----------|-------------|--------|
| Text Selection | UI Automation (IUIAutomation) | Critical - Core feature |
| Clipboard Monitoring | AddClipboardFormatListener | Critical - Hook Monitor |
| OCR Engine | Windows.Media.Ocr (WinRT) | High - OCR translation |
| Screen Capture | GDI (BitBlt) | High - Screenshot features |
| Input Simulation | SendInput | Critical - Selection capture |
| Foreground Detection | GetForegroundWindow | Medium - App context |

## Key Findings

### 1. Text Selection (Highest Priority)

**Current Implementation:**
- `UiAutomationSelectionProvider` - Uses Windows UI Automation TextPattern
- `ClipboardSelectionProvider` - Falls back to Ctrl+C simulation

**Platform Gaps:**
- **macOS**: No TextPattern equivalent. Need to use AXSelectedText, AXValue attributes
- **Linux**: AT-SPI2 has Text interface, but coverage varies by app

**Recommendation:**
```rust
// Trait to abstract text selection
trait TextSelectionProvider {
    fn get_selection(&self) -> Option<SelectionResult>;
}

// Implementations
struct WindowsUiAutomation;  // Existing
struct MacOsAccessibility;   // New: AXUIElement API
struct LinuxAtSpi;           // New: AT-SPI2 D-Bus
```

### 2. OCR Engine (High Priority)

**Current Implementation:**
- Windows.Media.Ocr (WinRT) - High quality, per-line/word bounding boxes
- Youdao OCR API - Cloud fallback

**Platform Gaps:**
- **macOS**: No WinRT. Use Vision Framework (VNRecognizeTextRequest)
- **Linux**: No WinRT. Use Tesseract or cloud APIs

**Recommendation:**
```rust
trait OcrEngine {
    fn recognize(&self, image: &[u8], lang: Option<&str>) -> Result<OcrResult>;
}

struct WinRtOcr;         // Existing (Windows)
struct VisionOcr;        // New (macOS) - Apple Vision Framework
struct TesseractOcr;     // New (Linux) - Tesseract CLI
struct YoudaoCloudOcr;   // Existing (Cross-platform fallback)
```

### 3. Clipboard Operations (Critical for Hook Monitor)

**Current Implementation:**
- Direct Win32 clipboard API calls
- AddClipboardFormatListener for event-driven monitoring

**Platform Gaps:**
- **macOS**: Use NSPasteboard, no direct event listener
- **Linux**: Use xclip/wl-clipboard commands, polling-based

**Recommendation:**
```rust
trait ClipboardProvider {
    fn get_text(&self) -> Result<Option<String>>;
    fn set_text(&self, text: &str) -> Result<()>;
    fn watch_changes(&self, callback: Box<dyn Fn(String)>) -> Result<()>;
}
```

### 4. Screen Capture (Medium Priority)

**Current Implementation:**
- GDI BitBlt for performance
- `screenshots` crate as fallback

**Platform Gaps:**
- **macOS**: CGDisplayCreateImage (already supported by screenshots crate)
- **Linux**: X11/Wayland (partially supported by screenshots crate)

**Recommendation:**
- Keep `screenshots` crate as primary cross-platform solution
- Use platform-native APIs only for performance-critical paths
- Current code already has `#[cfg(not(target_os = "windows"))]` fallbacks

### 5. Input Simulation (Critical for Selection Capture)

**Current Implementation:**
- SendInput for Ctrl+C simulation

**Platform Gaps:**
- **macOS**: Need CGEvent for Cmd+C simulation
- **Linux**: Need xdotool/ydotool

**Recommendation:**
```rust
trait InputSimulator {
    fn simulate_copy(&self) -> Result<()>;
    fn simulate_paste(&self) -> Result<()>;
    fn get_cursor_position(&self) -> Result<(i32, i32)>;
}
```

## Implementation Roadmap

### Phase 1: Abstraction Layer (1-2 weeks)

1. Define platform traits in `src-tauri/src/platform/traits.rs`
2. Refactor Windows code into `src-tauri/src/platform/windows/`
3. Add conditional compilation flags
4. Ensure existing tests still pass

### Phase 2: macOS Support (3-4 weeks)

1. **Week 1**: Accessibility API text selection
2. **Week 2**: Vision Framework OCR
3. **Week 3**: NSPasteboard clipboard + CGEvent input
4. **Week 4**: Testing and polish

### Phase 3: Linux Support (3-4 weeks)

1. **Week 1**: AT-SPI2 text selection
2. **Week 2**: Tesseract OCR integration
3. **Week 3**: Clipboard + input simulation
4. **Week 4**: X11/Wayland testing

## Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| macOS Accessibility requires user permission | High | Add clear permission request UI |
| Linux AT-SPI coverage varies | Medium | Keep clipboard fallback |
| Tesseract quality < WinRT OCR | Medium | Use Youdao OCR as primary |
| Wayland security restrictions | High | Document limitations clearly |

## Dependencies Required

### macOS
- No external dependencies (all Apple frameworks)
- System permission: Accessibility

### Linux
```bash
# Required
sudo apt install tesseract-ocr tesseract-ocr-chi-sim
sudo apt install xclip wl-clipboard

# Optional (for input simulation)
sudo apt install xdotool ydotool
```

## Code Changes Required

### New Files to Create

```
src-tauri/src/platform/
├── mod.rs              # Platform detection + re-exports
├── traits.rs           # Trait definitions
├── windows/
│   ├── mod.rs
│   ├── clipboard.rs    # Wrap existing code
│   ├── text_capture.rs # Wrap existing UIA code
│   ├── ocr.rs          # Wrap existing WinRT code
│   ├── screen_capture.rs
│   └── input.rs
├── macos/
│   ├── mod.rs
│   ├── clipboard.rs    # NSPasteboard
│   ├── text_capture.rs # Accessibility API
│   ├── ocr.rs          # Vision Framework
│   ├── screen_capture.rs
│   └── input.rs        # CGEvent
└── linux/
    ├── mod.rs
    ├── clipboard.rs    # xclip/wl-clipboard
    ├── text_capture.rs # AT-SPI2
    ├── ocr.rs          # Tesseract
    ├── screen_capture.rs
    └── input.rs        # xdotool/ydotool
```

### Files to Modify

1. `src-tauri/Cargo.toml` - Add platform-specific dependencies
2. `src-tauri/src/capabilities/hook_monitor.rs` - Use platform traits
3. `src-tauri/src/capabilities/platform/mod.rs` - Add macOS/Linux modules
4. `src-tauri/src/selection/mod.rs` - Use platform traits
5. `src-tauri/src/commands/capture.rs` - Use platform traits
6. `src-tauri/src/commands/window.rs` - Use platform traits

## Testing Strategy

### Unit Tests
- Mock platform traits for cross-platform testing
- Test trait implementations on each platform

### Integration Tests
- Test on macOS 12+ (Monterey)
- Test on Ubuntu 22.04 LTS (GNOME)
- Test on Fedora 38 (KDE)
- Test on Arch Linux (Sway/Wayland)

### Manual Testing Checklist
- [ ] Text selection in Chrome, Firefox, VS Code
- [ ] OCR screenshot translation
- [ ] Clipboard monitoring
- [ ] Overlay window display
- [ ] Global hotkeys
- [ ] Multi-monitor support

## Conclusion

Cross-platform support is feasible but requires significant effort. The recommended approach is:

1. **Abstract first**: Define clean platform traits
2. **Incremental**: Implement one platform at a time
3. **Fallback**: Keep cloud APIs (Youdao OCR) as universal fallback
4. **Document**: Clear setup instructions for each platform

Estimated total effort: **8-10 weeks** for full macOS + Linux support.

## References

- [Full Cross-Platform Guide](./CROSS_PLATFORM.md)
- [macOS Accessibility API](https://developer.apple.com/documentation/applicationservices/axuielement_h)
- [Apple Vision Framework](https://developer.apple.com/documentation/vision)
- [AT-SPI2 Documentation](https://wiki.gnome.org/Accessibility/AT-SPI2)
- [Tesseract OCR](https://github.com/tesseract-ocr/tesseract)
