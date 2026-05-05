use base64::Engine;
use screenshots::Screen;
use std::io::Cursor;
use tauri::command;

fn image_to_base64_png(image: &screenshots::image::DynamicImage) -> Result<String, String> {
    let mut buf = Cursor::new(Vec::new());
    image
        .write_to(&mut buf, screenshots::image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;
    let base64_str = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    Ok(format!("data:image/png;base64,{}", base64_str))
}

#[command]
pub async fn capture_screen(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;

    let screen = screens
        .first()
        .ok_or_else(|| "No screen found".to_string())?;

    // Capture the specified region
    let buffer = screen
        .capture_area(x, y, width, height)
        .map_err(|e| format!("Failed to capture area: {}", e))?;

    // Convert to DynamicImage
    let img = screenshots::image::DynamicImage::ImageRgba8(buffer);

    image_to_base64_png(&img)
}

#[command]
pub async fn capture_full_screen() -> Result<String, String> {
    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;

    let screen = screens
        .first()
        .ok_or_else(|| "No screen found".to_string())?;

    let buffer = screen.capture().map_err(|e| format!("Failed to capture screen: {}", e))?;

    let img = screenshots::image::DynamicImage::ImageRgba8(buffer);

    image_to_base64_png(&img)
}

/// Detect the foreground window HWND.
/// Returns the HWND as isize, or 0 if no foreground window.
#[command]
pub async fn detect_foreground_hwnd() -> Result<isize, String> {
    #[cfg(target_os = "windows")]
    {
        extern "system" {
            fn GetForegroundWindow() -> *mut std::ffi::c_void;
        }
        unsafe {
            let hwnd = GetForegroundWindow();
            if !hwnd.is_null() {
                return Ok(hwnd as isize);
            }
        }
    }
    Ok(0)
}

/// Get the window title for a given HWND.
/// Returns the title string, or empty string if not found.
#[command]
pub async fn get_window_title_cmd(hwnd: isize) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::GetWindowTextW;

        unsafe {
            let hwnd = HWND(hwnd as *mut _);
            let mut buf = [0u16; 512];
            let len = GetWindowTextW(hwnd, &mut buf);
            if len > 0 {
                return Ok(String::from_utf16_lossy(&buf[..len as usize]));
            }
        }
    }
    Ok(String::new())
}

/// Get the window rectangle for a given HWND.
/// Returns { x, y, width, height } or null if window not found.
#[command]
pub async fn get_window_rect_cmd(hwnd: isize) -> Result<Option<serde_json::Value>, String> {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct RECT {
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
        }
        extern "system" {
            fn GetWindowRect(hWnd: *mut std::ffi::c_void, lpRect: *mut RECT) -> i32;
        }
        unsafe {
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            let result = GetWindowRect(hwnd as *mut std::ffi::c_void, &mut rect);
            if result != 0 {
                return Ok(Some(serde_json::json!({
                    "x": rect.left,
                    "y": rect.top,
                    "width": rect.right - rect.left,
                    "height": rect.bottom - rect.top,
                })));
            }
        }
    }
    Ok(None)
}
