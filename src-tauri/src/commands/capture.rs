use base64::Engine;
#[cfg(not(target_os = "windows"))]
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

#[cfg(target_os = "windows")]
fn capture_area_gdi(
    left: i32,
    top: i32,
    width: u32,
    height: u32,
) -> Result<screenshots::image::DynamicImage, String> {
    use screenshots::image::{ImageBuffer, Rgba};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        HGDIOBJ, SRCCOPY,
    };

    if width == 0 || height == 0 {
        return Err("Capture area is empty".to_string());
    }

    unsafe {
        let hwnd = HWND(std::ptr::null_mut());
        let screen_dc = GetDC(hwnd);
        if screen_dc.0.is_null() {
            return Err("GetDC(NULL) failed".to_string());
        }

        let mem_dc = CreateCompatibleDC(screen_dc);
        if mem_dc.0.is_null() {
            let _ = ReleaseDC(hwnd, screen_dc);
            return Err("CreateCompatibleDC failed".to_string());
        }

        let bitmap = CreateCompatibleBitmap(screen_dc, width as i32, height as i32);
        if bitmap.0.is_null() {
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(hwnd, screen_dc);
            return Err("CreateCompatibleBitmap failed".to_string());
        }

        let old_object = SelectObject(mem_dc, HGDIOBJ(bitmap.0));
        if old_object.0.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(hwnd, screen_dc);
            return Err("SelectObject failed".to_string());
        }

        let blt_result = BitBlt(
            mem_dc,
            0,
            0,
            width as i32,
            height as i32,
            screen_dc,
            left,
            top,
            SRCCOPY,
        );

        if let Err(err) = blt_result {
            let _ = SelectObject(mem_dc, old_object);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(hwnd, screen_dc);
            return Err(format!("BitBlt failed: {}", err));
        }

        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bgra = vec![0u8; (width * height * 4) as usize];
        let rows = GetDIBits(
            mem_dc,
            bitmap,
            0,
            height,
            Some(bgra.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        );

        let _ = SelectObject(mem_dc, old_object);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(mem_dc);
        let _ = ReleaseDC(hwnd, screen_dc);

        if rows == 0 {
            return Err("GetDIBits failed".to_string());
        }

        for px in bgra.chunks_exact_mut(4) {
            px.swap(0, 2);
        }

        let image = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, bgra)
            .ok_or_else(|| "Failed to construct captured image buffer".to_string())?;
        Ok(screenshots::image::DynamicImage::ImageRgba8(image))
    }
}

#[cfg(target_os = "windows")]
fn virtual_screen_rect() -> Result<(i32, i32, u32, u32), String> {
    extern "system" {
        fn GetSystemMetrics(nIndex: i32) -> i32;
    }

    const SM_XVIRTUALSCREEN: i32 = 76;
    const SM_YVIRTUALSCREEN: i32 = 77;
    const SM_CXVIRTUALSCREEN: i32 = 78;
    const SM_CYVIRTUALSCREEN: i32 = 79;

    let x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };

    if width <= 0 || height <= 0 {
        return Err("Virtual screen has invalid dimensions".to_string());
    }

    Ok((x, y, width as u32, height as u32))
}

#[command]
pub async fn capture_screen(x: i32, y: i32, width: u32, height: u32) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let img = capture_area_gdi(x, y, width, height)?;
            image_to_base64_png(&img)
        }

        #[cfg(not(target_os = "windows"))]
        {
            let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;

            let screen = screens
                .first()
                .ok_or_else(|| "No screen found".to_string())?;

            let buffer = screen
                .capture_area(x, y, width, height)
                .map_err(|e| format!("Failed to capture area: {}", e))?;

            let img = screenshots::image::DynamicImage::ImageRgba8(buffer);
            image_to_base64_png(&img)
        }
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[command]
pub async fn capture_full_screen() -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let (x, y, width, height) = virtual_screen_rect()?;
            let img = capture_area_gdi(x, y, width, height)?;
            image_to_base64_png(&img)
        }

        #[cfg(not(target_os = "windows"))]
        {
            let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;

            let screen = screens
                .first()
                .ok_or_else(|| "No screen found".to_string())?;

            let buffer = screen
                .capture()
                .map_err(|e| format!("Failed to capture screen: {}", e))?;

            let img = screenshots::image::DynamicImage::ImageRgba8(buffer);
            image_to_base64_png(&img)
        }
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
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
