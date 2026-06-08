# Moon Translator - Cross-Platform Support Guide

## Overview

This document analyzes the platform-specific code in Moon Translator and provides a roadmap for macOS and Linux support. The application is built with Tauri v2, which provides cross-platform window management, but several core features rely on Windows-specific APIs.

## Current Windows Dependencies

### 1. Text Capture (Selection Detection)

**Files:**
- `src-tauri/src/selection/uiautomation.rs` - UI Automation text selection
- `src-tauri/src/selection/clipboard.rs` - Clipboard-based text selection
- `src-tauri/src/selection/manager.rs` - Provider chain manager

**Windows APIs Used:**
- `IUIAutomation`, `IUIAutomationTextPattern`, `IUIAutomationValuePattern`
- `GetForegroundWindow`, `GetWindowTextW`
- `OpenClipboard`, `GetClipboardData`, `SetClipboardData`
- `SendInput` (keyboard simulation for Ctrl+C)

**Current Architecture:**
```
SelectionProviderManager
  ├── UiAutomationSelectionProvider (priority: 10)
  └── ClipboardSelectionProvider (priority: 100)
```

### 2. Hook Monitor (Real-time Text Capture)

**File:** `src-tauri/src/capabilities/hook_monitor.rs`

**Windows APIs Used:**
- **UI Automation Source:** `IUIAutomation`, `UIA_TextPatternId`
- **Clipboard Source:** `AddClipboardFormatListener`, `WM_CLIPBOARDUPDATE`
- **OCR Source:** `GetClientRect`, `ClientToScreen`, GDI capture
- **Event Hook Source:** `SetWinEventHook`, `EVENT_OBJECT_TEXTSELECTIONCHANGED`, `EVENT_OBJECT_VALUECHANGE`

**Capture Sources:**
1. UIA (TextPattern polling) - Best for supported apps
2. Clipboard (event-driven) - Passive watch for copy events
3. OCR (screenshot + WinRT OCR) - Fallback for unsupported apps
4. Win32 Event Hook - `SetWinEventHook` for text changes

### 3. OCR Engine

**Files:**
- `src-tauri/src/ocr_engine.rs` - WinRT OCR engine
- `src-tauri/src/commands/capture.rs` - OCR commands

**Windows APIs Used:**
- `Windows.Media.Ocr.OcrEngine`
- `Windows.Graphics.Imaging.BitmapDecoder`
- `Windows.Storage.StorageFile`

**Features:**
- Per-line OCR with bounding boxes
- Word-level bounding boxes
- Multi-language support

### 4. Screen Capture

**File:** `src-tauri/src/commands/capture.rs`

**Windows APIs Used:**
- `GetDC`, `CreateCompatibleDC`, `CreateCompatibleBitmap`
- `BitBlt`, `GetDIBits`
- `ReleaseDC`, `DeleteDC`, `DeleteObject`

**Features:**
- Full screen capture
- Region capture
- GDI-based capture (faster than `screenshots` crate on Windows)

### 5. Foreground App Detection

**Files:**
- `src-tauri/src/capabilities/platform/windows.rs`
- `src-tauri/src/capabilities/platform/app_detector.rs`

**Windows APIs Used:**
- `GetForegroundWindow`, `GetWindowThreadProcessId`
- `OpenProcess`, `QueryFullProcessImageNameW`
- `GetClassNameW`

**Features:**
- Process name detection
- Window title detection
- Embedded app classification (Electron, WebView2, CEF)

### 6. Clipboard Operations

**Files:**
- `src-tauri/src/capabilities/platform/windows.rs`
- `src-tauri/src/commands/window.rs`

**Windows APIs Used:**
- `OpenClipboard`, `CloseClipboard`, `EmptyClipboard`
- `GetClipboardData`, `SetClipboardData`
- `GlobalAlloc`, `GlobalLock`, `GlobalUnlock`

### 7. Cursor Position

**Files:**
- `src-tauri/src/commands/window.rs`
- `src-tauri/src/capabilities/selection_translation_impl.rs`

**Windows APIs Used:**
- `GetCursorPos`

### 8. Screen Metrics

**File:** `src-tauri/src/commands/window.rs`

**Windows APIs Used:**
- `GetSystemMetrics` (SM_CXSCREEN, SM_CYSCREEN)

---

## Cross-Platform Abstraction Layer Design

### Interface Definitions

```rust
// src-tauri/src/platform/mod.rs

pub trait PlatformClipboard: Send + Sync {
    fn get_text(&self) -> Result<Option<String>, String>;
    fn set_text(&self, text: &str) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;
}

pub trait PlatformScreenCapture: Send + Sync {
    fn capture_area(&self, x: i32, y: i32, width: u32, height: u32) -> Result<Vec<u8>, String>;
    fn capture_full_screen(&self) -> Result<Vec<u8>, String>;
    fn get_screen_size(&self) -> Result<(u32, u32), String>;
}

pub trait PlatformOcrEngine: Send + Sync {
    fn recognize(&self, image_bytes: &[u8], lang: Option<&str>) -> Result<String, String>;
    fn recognize_detailed(&self, image_bytes: &[u8], lang: Option<&str>) -> Result<OcrResultDetailed, String>;
}

pub trait PlatformTextCapture: Send + Sync {
    fn get_selection(&self) -> Option<SelectionResult>;
    fn get_foreground_window_text(&self) -> Option<(String, String)>; // (text, window_title)
}

pub trait PlatformAppDetector: Send + Sync {
    fn get_foreground_app(&self) -> Option<ForegroundAppInfo>;
    fn classify_embedded_app(&self, app_name: &str, window_class: &str) -> Option<EmbeddedAppType>;
}

pub trait PlatformInputSimulator: Send + Sync {
    fn simulate_copy(&self) -> Result<(), String>;
    fn simulate_paste(&self) -> Result<(), String>;
    fn get_cursor_position(&self) -> Result<(i32, i32), String>;
}
```

---

## macOS Implementation Guide

### 1. Text Capture - Accessibility API

**替代方案:** macOS Accessibility API (AXUIElement)

**依赖:**
```toml
[target.'cfg(target_os = "macos")'.dependencies]
cocoa = "0.25"
core-foundation = "0.9"
accessibility-sys = "0.1"
```

**实现思路:**
```rust
// src-tauri/src/platform/macos/text_capture.rs

use accessibility_sys::*;
use core_foundation::string::CFString;

pub struct MacOsTextCapture;

impl PlatformTextCapture for MacOsTextCapture {
    fn get_selection(&self) -> Option<SelectionResult> {
        unsafe {
            // Get the focused application
            let system_wide = AXUIElementCreateSystemWide();
            let focused_app = AXUIElementCopyAttributeValue(
                system_wide,
                kAXFocusedApplicationAttribute as _,
            )?;

            // Get the focused UI element
            let focused_element = AXUIElementCopyAttributeValue(
                focused_app,
                kAXFocusedUIElementAttribute as _,
            )?;

            // Try to get selected text
            let selected_text = AXUIElementCopyAttributeValue(
                focused_element,
                kAXSelectedTextAttribute as _,
            )?;

            // Get value pattern for full text (fallback)
            // ...

            Some(SelectionResult {
                text: selected_text.to_string(),
                source_app: get_app_name(focused_app),
                window_title: get_window_title(focused_app),
                bounds: get_bounds(focused_element),
                confidence: 0.9,
                provider: "accessibility",
            })
        }
    }
}
```

**关键差异:**
- macOS Accessibility API 需要辅助功能权限 (System Preferences > Privacy > Accessibility)
- 没有直接的 TextPattern 概念，需要通过 `AXSelectedText`, `AXValue` 等属性
- Electron/Chromium 应用通常支持 AX API

### 2. OCR - Vision Framework

**替代方案:** Apple Vision Framework (VNRecognizeTextRequest)

**依赖:**
```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc = "0.2"
cocoa = "0.25"
```

**实现思路:**
```rust
// src-tauri/src/platform/macos/ocr.rs

use objc::runtime::*;
use objc::sel;
use objc::sel_impl;
use objc::msg_send;

pub struct MacOsOcrEngine;

impl PlatformOcrEngine for MacOsOcrEngine {
    fn recognize(&self, image_bytes: &[u8], lang: Option<&str>) -> Result<String, String> {
        unsafe {
            // Create CGImage from PNG bytes
            let image = create_cgimage_from_png(image_bytes)?;

            // Create VNRecognizeTextRequest
            let request: *mut Object = msg_send![class!(VNRecognizeTextRequest), new];

            // Set language if specified
            if let Some(lang) = lang {
                let lang_str = NSString::new(lang);
                let _: () = msg_send![request, setRecognitionLanguages: lang_str];
            }

            // Set recognition level to accurate
            let _: () = msg_send![request, setRecognitionLevel: 1]; // VNRequestTextRecognitionLevelAccurate

            // Create VNImageRequestHandler
            let handler: *mut Object = msg_send![
                class!(VNImageRequestHandler),
                initWithCGImage: image
                options: nil
            ];

            // Perform request
            let mut error: *mut Object = nil;
            let success: bool = msg_send![handler, performRequests: &[request] error: &mut error];

            if !success {
                return Err("Vision Framework OCR failed".to_string());
            }

            // Get results
            let results: *mut Object = msg_send![request, results];
            let count: usize = msg_send![results, count];
            let mut text = String::new();

            for i in 0..count {
                let observation: *mut Object = msg_send![results, objectAtIndex: i];
                let candidate: *mut Object = msg_send![observation, topCandidates: 1];
                let top_candidate: *mut Object = msg_send![candidate, objectAtIndex: 0];
                let candidate_text: *mut Object = msg_send![top_candidate, string];
                let str: &str = get_nsstring_str(candidate_text);
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(str);
            }

            Ok(text)
        }
    }
}
```

**关键差异:**
- Vision Framework 是 Apple 原生 OCR，质量高，支持多语言
- 需要 macOS 10.15+ (Catalina)
- 支持逐行识别和边界框
- 无需额外安装依赖

### 3. Clipboard - NSPasteboard

**替代方案:** NSPasteboard

**实现思路:**
```rust
// src-tauri/src/platform/macos/clipboard.rs

use cocoa::appkit::NSPasteboard;
use cocoa::base::{id, nil};
use cocoa::foundation::NSString;

pub struct MacOsClipboard;

impl PlatformClipboard for MacOsClipboard {
    fn get_text(&self) -> Result<Option<String>, String> {
        unsafe {
            let pasteboard: id = NSPasteboard::generalPasteboard(nil);
            let types: id = msg_send![pasteboard, types];

            // Check if string type is available
            let string_type = NSString::nil();
            let has_string: bool = msg_send![types, containsObject: string_type];

            if !has_string {
                return Ok(None);
            }

            let text: id = msg_send![pasteboard, stringForType: string_type];
            if text == nil {
                return Ok(None);
            }

            let string: &str = get_nsstring_str(text);
            Ok(Some(string.to_string()))
        }
    }

    fn set_text(&self, text: &str) -> Result<(), String> {
        unsafe {
            let pasteboard: id = NSPasteboard::generalPasteboard(nil);
            let _: () = msg_send![pasteboard, clearContents];

            let ns_string = NSString::new(text);
            let _: () = msg_send![pasteboard, setString: ns_string forType: NSString::nil()];

            Ok(())
        }
    }
}
```

**关键差异:**
- macOS 剪贴板 API 更简洁
- 支持多种数据类型 (UTI)
- 无需打开/关闭剪贴板

### 4. Input Simulation - CGEvent

**替代方案:** Core Graphics Event

**实现思路:**
```rust
// src-tauri/src/platform/macos/input.rs

use core_graphics::event::*;
use core_graphics::event_source::*;

pub struct MacOsInputSimulator;

impl PlatformInputSimulator for MacOsInputSimulator {
    fn simulate_copy(&self) -> Result<(), String> {
        unsafe {
            let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
                .map_err(|e| format!("Failed to create event source: {:?}", e))?;

            // Press Cmd+C
            let key_down = CGEvent::new_keyboard_event(source.clone(), 0x08, true) // 0x08 = 'C'
                .map_err(|e| format!("Failed to create key down event: {:?}", e))?;
            key_down.set_flags(CGEventFlags::CGEventFlagCommand);
            key_down.post(CGEventTapLocation::HID);

            // Release Cmd+C
            let key_up = CGEvent::new_keyboard_event(source, 0x08, false)
                .map_err(|e| format!("Failed to create key up event: {:?}", e))?;
            key_up.post(CGEventTapLocation::HID);

            Ok(())
        }
    }

    fn get_cursor_position(&self) -> Result<(i32, i32), String> {
        unsafe {
            let event = CGEvent::new(None)
                .map_err(|e| format!("Failed to create event: {:?}", e))?;
            let point = event.location();
            Ok((point.x as i32, point.y as i32))
        }
    }
}
```

**关键差异:**
- macOS 使用 Cmd+C 而不是 Ctrl+C
- 需要辅助功能权限来模拟键盘事件
- CGEvent API 更面向对象

### 5. Screen Capture - Core Graphics

**替代方案:** CGDisplayCreateImage

**实现思路:**
```rust
// src-tauri/src/platform/macos/screen_capture.rs

use core_graphics::display::*;
use core_graphics::image::*;

pub struct MacOsScreenCapture;

impl PlatformScreenCapture for MacOsScreenCapture {
    fn capture_area(&self, x: i32, y: i32, width: u32, height: u32) -> Result<Vec<u8>, String> {
        unsafe {
            let rect = CGRect::new(
                &CGPoint::new(x as f64, y as f64),
                &CGSize::new(width as f64, height as f64),
            );

            let image = CGDisplayCreateImage(rect)
                .map_err(|e| format!("Failed to capture screen: {:?}", e))?;

            // Convert to PNG
            let png_data = cgimage_to_png(image)?;
            Ok(png_data)
        }
    }

    fn get_screen_size(&self) -> Result<(u32, u32), String> {
        unsafe {
            let main_display = CGMainDisplayID();
            let width = CGDisplayPixelsWide(main_display) as u32;
            let height = CGDisplayPixelsHigh(main_display) as u32;
            Ok((width, height))
        }
    }
}
```

**关键差异:**
- macOS Retina 显示器需要处理缩放
- CGDisplayCreateImage 返回 CGImage，需要转换为 PNG
- 多显示器支持需要枚举 displays

### 6. Foreground App Detection - NSWorkspace

**替代方案:** NSWorkspace + NSRunningApplication

**实现思路:**
```rust
// src-tauri/src/platform/macos/app_detector.rs

use cocoa::appkit::NSWorkspace;
use cocoa::base::{id, nil};

pub struct MacOsAppDetector;

impl PlatformAppDetector for MacOsAppDetector {
    fn get_foreground_app(&self) -> Option<ForegroundAppInfo> {
        unsafe {
            let workspace: id = NSWorkspace::sharedWorkspace(nil);
            let app: id = msg_send![workspace, frontmostApplication];

            if app == nil {
                return None;
            }

            let name: id = msg_send![app, localizedName];
            let bundle_id: id = msg_send![app, bundleIdentifier];
            let pid: i32 = msg_send![app, processIdentifier];

            Some(ForegroundAppInfo {
                app_name: get_nsstring_str(name).to_string(),
                bundle_id: get_nsstring_str(bundle_id).to_string(),
                pid: pid as u32,
                window_title: get_active_window_title(pid),
            })
        }
    }
}
```

**关键差异:**
- macOS 使用 Bundle Identifier 而不是进程名
- 窗口标题需要通过 Accessibility API 获取
- 没有 window class 概念

---

## Linux Implementation Guide

### 1. Text Capture - AT-SPI

**替代方案:** AT-SPI2 (Assistive Technology Service Provider Interface)

**依赖:**
```toml
[target.'cfg(target_os = "linux")'.dependencies]
atspi = "0.19"
atspi-common = "0.19"
zbus = "3"
```

**实现思路:**
```rust
// src-tauri/src/platform/linux/text_capture.rs

use atspi::accessible::AccessibleProxy;
use atspi::text::TextProxy;
use atspi::collection::CollectionProxy;

pub struct LinuxTextCapture;

impl PlatformTextCapture for LinuxTextCapture {
    fn get_selection(&self) -> Option<SelectionResult> {
        // Connect to AT-SPI bus
        let connection = zbus::Connection::session().ok()?;
        let registry = atspi::registry::RegistryProxy::new(&connection).ok()?;

        // Get focused application
        let focused_app = registry.getFocusedApplication().ok()?;

        // Get focused accessible
        let accessible = AccessibleProxy::new(&connection, focused_app).ok()?;

        // Try to get selected text via Text interface
        if let Ok(text_proxy) = TextProxy::new(&connection, accessible.clone()) {
            if let Ok(selection) = text_proxy.getSelection(0) {
                let text = selection.text;
                if !text.is_empty() {
                    return Some(SelectionResult {
                        text,
                        source_app: get_app_name(&accessible),
                        window_title: get_window_title(&accessible),
                        bounds: get_bounds(&accessible),
                        confidence: 0.85,
                        provider: "atspi",
                    });
                }
            }
        }

        // Fallback: get full value
        if let Ok(value) = accessible.name() {
            if !value.is_empty() {
                return Some(SelectionResult {
                    text: value,
                    source_app: get_app_name(&accessible),
                    window_title: get_window_title(&accessible),
                    bounds: None,
                    confidence: 0.7,
                    provider: "atspi",
                });
            }
        }

        None
    }
}
```

**关键差异:**
- 需要 AT-SPI2 守护进程运行 (`at-spi2-registryd`)
- 需要设置 `ACCESSIBILITY_ENABLED=1` 环境变量
- GNOME/KDE 默认启用，其他桌面环境可能需要手动启用
- Electron/Chromium 应用支持 AT-SPI

### 2. OCR - Tesseract

**替代方案:** Tesseract OCR (via `tesseract` crate 或命令行)

**依赖:**
```toml
[target.'cfg(target_os = "linux")'.dependencies]
tesseract = "0.14"  # 或使用 leptonica-rs
```

**实现思路:**
```rust
// src-tauri/src/platform/linux/ocr.rs

use tesseract::Tesseract;

pub struct LinuxOcrEngine;

impl PlatformOcrEngine for LinuxOcrEngine {
    fn recognize(&self, image_bytes: &[u8], lang: Option<&str>) -> Result<String, String> {
        // Write image to temp file (Tesseract needs file path)
        let temp_path = std::env::temp_dir().join("moontranslator_ocr.png");
        std::fs::write(&temp_path, image_bytes)
            .map_err(|e| format!("Failed to write temp file: {}", e))?;

        let mut tesseract = Tesseract::new(None, lang.unwrap_or("eng"))
            .map_err(|e| format!("Failed to initialize Tesseract: {}", e))?;

        tesseract.set_image(temp_path.to_str().unwrap())
            .map_err(|e| format!("Failed to set image: {}", e))?;

        let text = tesseract.get_text()
            .map_err(|e| format!("OCR failed: {}", e))?;

        // Cleanup
        let _ = std::fs::remove_file(temp_path);

        Ok(text)
    }

    fn recognize_detailed(&self, image_bytes: &[u8], lang: Option<&str>) -> Result<OcrResultDetailed, String> {
        // Similar to recognize(), but use get_hocr() for bounding boxes
        // or use tesseract-sys for more control
        todo!()
    }
}
```

**替代方案 2:** 云 OCR API (Youdao, Baidu, Google Vision)

**关键差异:**
- Tesseract 需要安装系统包 (`sudo apt install tesseract-ocr`)
- 语言数据包需要单独安装 (`tesseract-ocr-chi-sim` for 中文)
- 识别质量不如 WinRT OCR 或 Vision Framework
- 建议保留 Youdao OCR 作为主要 OCR 引擎，Tesseract 作为离线备选

### 3. Clipboard - X11/Wayland

**替代方案:** xclip/xsel (X11) 或 wl-clipboard (Wayland)

**依赖:**
```toml
[target.'cfg(target_os = "linux")'.dependencies]
x11rb = "0.12"  # X11
wl-clipboard-rs = "0.7"  # Wayland
```

**实现思路:**
```rust
// src-tauri/src/platform/linux/clipboard.rs

use std::process::Command;

pub struct LinuxClipboard;

impl PlatformClipboard for LinuxClipboard {
    fn get_text(&self) -> Result<Option<String>, String> {
        // Try xclip first (X11)
        if let Ok(output) = Command::new("xclip")
            .args(["-selection", "clipboard", "-o"])
            .output()
        {
            if output.status.success() {
                return Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()));
            }
        }

        // Try wl-paste (Wayland)
        if let Ok(output) = Command::new("wl-paste")
            .args(["--no-newline"])
            .output()
        {
            if output.status.success() {
                return Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()));
            }
        }

        // Try xsel
        if let Ok(output) = Command::new("xsel")
            .args(["--clipboard", "--output"])
            .output()
        {
            if output.status.success() {
                return Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()));
            }
        }

        Err("No clipboard utility found (install xclip or wl-clipboard)".to_string())
    }

    fn set_text(&self, text: &str) -> Result<(), String> {
        // Try xclip first
        if Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child.stdin.as_mut().unwrap().write_all(text.as_bytes())
            })
            .is_ok()
        {
            return Ok(());
        }

        // Try wl-copy
        if Command::new("wl-copy")
            .arg(text)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Ok(());
        }

        Err("No clipboard utility found".to_string())
    }
}
```

**关键差异:**
- Linux 剪贴板依赖外部工具 (`xclip`, `wl-clipboard`)
- X11 和 Wayland 需要不同的实现
- 建议检测显示服务器类型并选择合适的工具
- 可以使用 `x11rb` crate 直接调用 X11 协议

### 4. Input Simulation - xdotool/ydotool

**替代方案:** xdotool (X11) 或 ydotool (Wayland)

**实现思路:**
```rust
// src-tauri/src/platform/linux/input.rs

use std::process::Command;

pub struct LinuxInputSimulator;

impl PlatformInputSimulator for LinuxInputSimulator {
    fn simulate_copy(&self) -> Result<(), String> {
        // X11: xdotool key ctrl+c
        if Command::new("xdotool")
            .args(["key", "ctrl+c"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Ok(());
        }

        // Wayland: ydotool key ctrl+c
        if Command::new("ydotool")
            .args(["key", "ctrl+c"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Ok(());
        }

        Err("No input simulation tool found (install xdotool or ydotool)".to_string())
    }

    fn get_cursor_position(&self) -> Result<(i32, i32), String> {
        // X11: xdotool getmouselocation
        if let Ok(output) = Command::new("xdotool")
            .args(["getmouselocation"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse "x:123 y:456 screen:0 window:789"
            if let (Some(x), Some(y)) = (
                stdout.find("x:").and_then(|i| stdout[i+2..].split_whitespace().next()),
                stdout.find("y:").and_then(|i| stdout[i+2..].split_whitespace().next()),
            ) {
                if let (Ok(x), Ok(y)) = (x.parse::<i32>(), y.parse::<i32>()) {
                    return Ok((x, y));
                }
            }
        }

        // Fallback
        Ok((100, 100))
    }
}
```

**关键差异:**
- X11 和 Wayland 需要不同的工具
- ydotool 需要 root 权限或 input 组
- 可以使用 `enigo` crate 作为跨平台输入模拟库

### 5. Screen Capture - X11/Wayland

**替代方案:** XGetImage (X11) 或 grim (Wayland)

**依赖:**
```toml
[target.'cfg(target_os = "linux")'.dependencies]
x11rb = "0.12"  # X11
```

**实现思路:**
```rust
// src-tauri/src/platform/linux/screen_capture.rs

use std::process::Command;

pub struct LinuxScreenCapture;

impl PlatformScreenCapture for LinuxScreenCapture {
    fn capture_area(&self, x: i32, y: i32, width: u32, height: u32) -> Result<Vec<u8>, String> {
        // Try grim (Wayland)
        if is_wayland() {
            let geometry = format!("{},{},{},{}", x, y, width, height);
            if let Ok(output) = Command::new("grim")
                .args(["-g", &geometry, "-"])
                .output()
            {
                if output.status.success() {
                    return Ok(output.stdout);
                }
            }
        }

        // Try import (ImageMagick, X11)
        let geometry = format!("{}x{}+{}+{}", width, height, x, y);
        if let Ok(output) = Command::new("import")
            .args(["-window", "root", "-crop", &geometry, "png:-"])
            .output()
        {
            if output.status.success() {
                return Ok(output.stdout);
            }
        }

        // Fallback: screenshots crate
        let screens = screenshots::Screen::all()
            .map_err(|e| format!("Failed to get screens: {}", e))?;
        let screen = screens.first()
            .ok_or_else(|| "No screen found".to_string())?;
        let buffer = screen.capture_area(x, y, width, height)
            .map_err(|e| format!("Failed to capture area: {}", e))?;

        // Convert to PNG
        let mut buf = std::io::Cursor::new(Vec::new());
        screenshots::image::DynamicImage::ImageRgba8(buffer)
            .write_to(&mut buf, screenshots::image::ImageFormat::Png)
            .map_err(|e| format!("Failed to encode PNG: {}", e))?;

        Ok(buf.into_inner())
    }
}
```

**关键差异:**
- Wayland 安全限制更严格，需要使用 grim 等专用工具
- X11 可以使用 XGetImage 或 ImageMagick 的 import 命令
- `screenshots` crate 已经支持 Linux，可以作为备选

### 6. Foreground App Detection - X11/Wayland

**替代方案:** xdotool (X11) 或 sway/wlr-toplevel-unstable (Wayland)

**实现思路:**
```rust
// src-tauri/src/platform/linux/app_detector.rs

use std::process::Command;

pub struct LinuxAppDetector;

impl PlatformAppDetector for LinuxAppDetector {
    fn get_foreground_app(&self) -> Option<ForegroundAppInfo> {
        // X11: xdotool getactivewindow getwindowpid getwindowname
        if let Ok(pid_output) = Command::new("xdotool")
            .args(["getactivewindow", "getwindowpid"])
            .output()
        {
            let pid_str = String::from_utf8_lossy(&pid_output.stdout);
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                let window_name = Command::new("xdotool")
                    .args(["getactivewindow", "getwindowname"])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_default();

                let app_name = get_process_name(pid);

                return Some(ForegroundAppInfo {
                    app_name,
                    window_title: window_name,
                    pid,
                });
            }
        }

        // Wayland: more complex, need to use D-Bus or compositor-specific APIs
        // For Sway: swaymsg -t get_tree
        if let Ok(output) = Command::new("swaymsg")
            .args(["-t", "get_tree"])
            .output()
        {
            // Parse JSON to find focused window
            // ...
        }

        None
    }
}
```

**关键差异:**
- X11 和 Wayland 实现差异很大
- Wayland 安全模型限制窗口信息访问
- 建议使用 `xdotool` (X11) 和 compositor-specific 命令 (Wayland)

---

## Architecture Recommendations

### 1. Feature Flags

```toml
# Cargo.toml
[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]

# Platform-specific features
platform-windows = []
platform-macos = []
platform-linux = []
```

### 2. Conditional Compilation

```rust
// src-tauri/src/platform/mod.rs

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

// Re-export platform-specific implementations
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "linux")]
pub use linux::*;
```

### 3. Platform Detection at Runtime

```rust
// src-tauri/src/platform/detect.rs

pub enum DisplayServer {
    X11,
    Wayland,
    Unknown,
}

pub fn detect_display_server() -> DisplayServer {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        DisplayServer::Wayland
    } else if std::env::var("DISPLAY").is_ok() {
        DisplayServer::X11
    } else {
        DisplayServer::Unknown
    }
}
```

### 4. Graceful Degradation

```rust
// src-tauri/src/ocr_engine.rs

pub fn run_ocr(png_bytes: &[u8], lang: Option<&str>) -> Result<Option<String>, String> {
    // Try platform-specific OCR first
    #[cfg(target_os = "windows")]
    {
        return run_winrt_ocr(png_bytes, lang);
    }

    #[cfg(target_os = "macos")]
    {
        return run_vision_ocr(png_bytes, lang);
    }

    #[cfg(target_os = "linux")]
    {
        // Try Tesseract first
        if let Ok(result) = run_tesseract_ocr(png_bytes, lang) {
            return Ok(Some(result));
        }

        // Fallback to Youdao OCR API
        return run_youdao_ocr(png_bytes, lang);
    }
}
```

---

## Implementation Priority

### Phase 1: Core Abstractions (Week 1-2)
1. Define platform trait interfaces
2. Implement Windows backend (wrap existing code)
3. Add conditional compilation

### Phase 2: macOS Support (Week 3-5)
1. Implement Accessibility API text capture
2. Implement Vision Framework OCR
3. Implement NSPasteboard clipboard
4. Implement CGEvent input simulation
5. Implement Core Graphics screen capture
6. Test and fix issues

### Phase 3: Linux Support (Week 6-8)
1. Implement AT-SPI text capture
2. Implement Tesseract OCR (or keep Youdao as primary)
3. Implement xclip/wl-clipboard
4. Implement xdotool/ydotool input simulation
5. Implement X11/Wayland screen capture
6. Test on GNOME, KDE, Sway

### Phase 4: Polish (Week 9-10)
1. Add installation guides for dependencies
2. Add fallback mechanisms
3. Performance optimization
4. Documentation

---

## Dependencies Installation

### macOS
```bash
# No additional dependencies needed (all Apple frameworks)
# Grant Accessibility permission in System Preferences
```

### Linux (Ubuntu/Debian)
```bash
# OCR
sudo apt install tesseract-ocr tesseract-ocr-chi-sim tesseract-ocr-eng

# Clipboard
sudo apt install xclip wl-clipboard

# Input simulation (X11)
sudo apt install xdotool

# Input simulation (Wayland)
sudo apt install ydotool

# AT-SPI
sudo apt install at-spi2-core libatspi2.0-dev
```

### Linux (Fedora)
```bash
# OCR
sudo dnf install tesseract tesseract-langpack-chi-sim tesseract-langpack-eng

# Clipboard
sudo dnf install xclip wl-clipboard

# Input simulation
sudo dnf install xdotool ydotool
```

---

## Known Limitations

### macOS
- 需要用户手动授予辅助功能权限
- Electron 应用的 Accessibility 支持可能不完整
- Vision Framework 需要 macOS 10.15+

### Linux
- Wayland 安全模型限制了某些功能
- AT-SPI 支持因桌面环境而异
- ydotool 需要特殊权限
- 某些功能在 X11/Wayland 之间需要不同的实现

---

## Testing Checklist

- [ ] Text selection capture (UIA/Accessibility/AT-SPI)
- [ ] Clipboard operations
- [ ] OCR recognition (WinRT/Vision/Tesseract)
- [ ] Screen capture
- [ ] Foreground app detection
- [ ] Input simulation (Ctrl+C/Cmd+C)
- [ ] Overlay window management
- [ ] Global hotkeys
- [ ] Multi-monitor support
- [ ] HiDPI/Retina display support

---

## References

### macOS
- [Apple Accessibility API](https://developer.apple.com/documentation/applicationservices/axuielement_h)
- [Vision Framework](https://developer.apple.com/documentation/vision)
- [Core Graphics Events](https://developer.apple.com/documentation/coregraphics/cgevent)
- [NSPasteboard](https://developer.apple.com/documentation/appkit/nspasteboard)

### Linux
- [AT-SPI2 Documentation](https://wiki.gnome.org/Accessibility/AT-SPI2)
- [Tesseract OCR](https://github.com/tesseract-ocr/tesseract)
- [X11rb (Rust X11)](https://github.com/psychon/x11rb)
- [Wayland Clipboard](https://github.com/bugaevc/wl-clipboard-rs)

### Cross-Platform Rust Crates
- [enigo](https://github.com/enigo-rs/enigo) - Cross-platform input simulation
- [screenshots](https://github.com/nashaofu/screenshots) - Cross-platform screen capture
- [arboard](https://github.com/ArturKovacs/arboard) - Cross-platform clipboard
