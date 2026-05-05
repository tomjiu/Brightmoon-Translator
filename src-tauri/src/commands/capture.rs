use base64::Engine;
use screenshots::Screen;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::PathBuf;
use tauri::command;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotSnapshotInfo {
    pub screen_x: i32,
    pub screen_y: i32,
    pub screen_width: u32,
    pub screen_height: u32,
    pub scale_factor: f32,
    pub image_width: u32,
    pub image_height: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotSnapshot {
    pub image: String,
    pub info: ScreenshotSnapshotInfo,
}

fn image_to_base64_png(image: &screenshots::image::DynamicImage) -> Result<String, String> {
    let mut buf = Cursor::new(Vec::new());
    image
        .write_to(&mut buf, screenshots::image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;
    let base64_str = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    Ok(format!("data:image/png;base64,{}", base64_str))
}

fn ocr_snapshot_image_path() -> PathBuf {
    std::env::temp_dir().join("moontranslator_ocr_snapshot.png")
}

fn ocr_snapshot_meta_path() -> PathBuf {
    std::env::temp_dir().join("moontranslator_ocr_snapshot.json")
}

fn encode_png_bytes(raw: &[u8]) -> String {
    let base64_str = base64::engine::general_purpose::STANDARD.encode(raw);
    format!("data:image/png;base64,{}", base64_str)
}

fn crop_image_to_base64(
    image: &screenshots::image::DynamicImage,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    if width == 0 || height == 0 {
        return Err("Crop area is empty".to_string());
    }
    if left >= image.width() || top >= image.height() {
        return Err("Crop origin is outside image bounds".to_string());
    }

    let crop_width = width.min(image.width() - left);
    let crop_height = height.min(image.height() - top);
    let cropped = image.crop_imm(left, top, crop_width, crop_height);
    image_to_base64_png(&cropped)
}

#[command]
pub async fn capture_screen(x: i32, y: i32, width: u32, height: u32) -> Result<String, String> {
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

    let buffer = screen
        .capture()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;

    let img = screenshots::image::DynamicImage::ImageRgba8(buffer);

    image_to_base64_png(&img)
}

#[command]
pub async fn prepare_screenshot_snapshot() -> Result<ScreenshotSnapshotInfo, String> {
    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;
    let screen = screens
        .first()
        .ok_or_else(|| "No screen found".to_string())?;
    let buffer = screen
        .capture()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;
    let info = ScreenshotSnapshotInfo {
        screen_x: screen.display_info.x,
        screen_y: screen.display_info.y,
        screen_width: screen.display_info.width,
        screen_height: screen.display_info.height,
        scale_factor: screen.display_info.scale_factor,
        image_width: buffer.width(),
        image_height: buffer.height(),
    };

    buffer
        .save(ocr_snapshot_image_path())
        .map_err(|e| format!("Failed to save screenshot snapshot: {}", e))?;
    let meta = serde_json::to_vec(&info)
        .map_err(|e| format!("Failed to serialize screenshot metadata: {}", e))?;
    std::fs::write(ocr_snapshot_meta_path(), meta)
        .map_err(|e| format!("Failed to save screenshot metadata: {}", e))?;
    Ok(info)
}

#[command]
pub async fn load_screenshot_snapshot() -> Result<ScreenshotSnapshot, String> {
    let image_path = ocr_snapshot_image_path();
    let raw = std::fs::read(&image_path)
        .map_err(|e| format!("Failed to read screenshot snapshot: {}", e))?;
    let info = if let Ok(meta) = std::fs::read(ocr_snapshot_meta_path()) {
        serde_json::from_slice::<ScreenshotSnapshotInfo>(&meta)
            .map_err(|e| format!("Failed to parse screenshot metadata: {}", e))?
    } else {
        let image = screenshots::image::load_from_memory(&raw)
            .map_err(|e| format!("Failed to inspect screenshot snapshot: {}", e))?;
        ScreenshotSnapshotInfo {
            screen_x: 0,
            screen_y: 0,
            screen_width: image.width(),
            screen_height: image.height(),
            scale_factor: 1.0,
            image_width: image.width(),
            image_height: image.height(),
        }
    };

    Ok(ScreenshotSnapshot {
        image: encode_png_bytes(&raw),
        info,
    })
}

#[command]
pub async fn crop_screenshot_snapshot(
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let raw = std::fs::read(ocr_snapshot_image_path())
        .map_err(|e| format!("Failed to read screenshot snapshot: {}", e))?;
    let image = screenshots::image::load_from_memory(&raw)
        .map_err(|e| format!("Failed to load screenshot snapshot: {}", e))?;
    crop_image_to_base64(&image, left, top, width, height)
}

#[command]
pub async fn capture_screenshot_region(
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;
    let screen = screens
        .first()
        .ok_or_else(|| "No screen found".to_string())?;
    let buffer = screen
        .capture()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;
    let image = screenshots::image::DynamicImage::ImageRgba8(buffer);
    crop_image_to_base64(&image, left, top, width, height)
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

// ── Screenshot Translation MVP Commands ──────────────────────────────────────

/// Strip the `data:image/png;base64,` prefix and decode.
fn decode_base64_png(data_url: &str) -> Result<Vec<u8>, String> {
    let b64 = data_url
        .strip_prefix("data:image/png;base64,")
        .unwrap_or(data_url);
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("Base64 decode failed: {}", e))
}

/// Capture the full primary screen and return as base64 data-URL.
#[command]
pub async fn screenshot_to_base64() -> Result<String, String> {
    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;
    let screen = screens
        .first()
        .ok_or_else(|| "No screen found".to_string())?;
    let buffer = screen
        .capture()
        .map_err(|e| format!("Capture failed: {}", e))?;
    let img = screenshots::image::DynamicImage::ImageRgba8(buffer);
    image_to_base64_png(&img)
}

/// Crop a region from a base64 PNG data-URL and return the cropped base64 data-URL.
/// Coordinates are in physical pixels of the source image.
#[command]
pub async fn cut_image_base64(
    source_base64: String,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let raw = decode_base64_png(&source_base64)?;
    let img = screenshots::image::load_from_memory(&raw)
        .map_err(|e| format!("Image load failed: {}", e))?;
    crop_image_to_base64(&img, left, top, width, height)
}

/// Run Windows.Media.Ocr on a base64 PNG data-URL.
/// Returns recognized text or error.
#[command]
pub async fn system_ocr(base64_data: String, lang: Option<String>) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::HSTRING;
        use windows::Globalization::Language;
        use windows::Graphics::Imaging::BitmapDecoder;
        use windows::Media::Ocr::OcrEngine;
        use windows::Storage::{FileAccessMode, StorageFile};

        // Decode to temp file (WinRT needs StorageFile path)
        let raw = decode_base64_png(&base64_data)?;
        let temp_path = std::env::temp_dir().join("moontranslator_ocr_temp.png");
        std::fs::write(&temp_path, &raw).map_err(|e| format!("Temp write failed: {}", e))?;

        let path_str = temp_path.to_string_lossy().replace("\\\\?\\", "");

        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(&path_str))
            .map_err(|e| format!("StorageFile: {}", e))?
            .get()
            .map_err(|e| format!("StorageFile await: {}", e))?;

        let stream = file
            .OpenAsync(FileAccessMode::Read)
            .map_err(|e| format!("OpenAsync: {}", e))?
            .get()
            .map_err(|e| format!("OpenAsync await: {}", e))?;

        let decoder = BitmapDecoder::CreateWithIdAsync(
            BitmapDecoder::PngDecoderId().map_err(|e| format!("PngDecoderId: {}", e))?,
            &stream,
        )
        .map_err(|e| format!("BitmapDecoder: {}", e))?
        .get()
        .map_err(|e| format!("BitmapDecoder await: {}", e))?;

        let bitmap = decoder
            .GetSoftwareBitmapAsync()
            .map_err(|e| format!("SoftwareBitmap: {}", e))?
            .get()
            .map_err(|e| format!("SoftwareBitmap await: {}", e))?;

        let engine = match lang.as_deref() {
            Some(l) if l != "auto" => {
                let language = Language::CreateLanguage(&HSTRING::from(l))
                    .map_err(|e| format!("Language: {}", e))?;
                OcrEngine::TryCreateFromLanguage(&language)
                    .map_err(|e| format!("OcrEngine: {}", e))?
            }
            _ => OcrEngine::TryCreateFromUserProfileLanguages()
                .map_err(|e| format!("OcrEngine: {}", e))?,
        };

        let result = engine
            .RecognizeAsync(&bitmap)
            .map_err(|e| format!("RecognizeAsync: {}", e))?
            .get()
            .map_err(|e| format!("RecognizeAsync await: {}", e))?;

        let text = result
            .Text()
            .map_err(|e| format!("Text: {}", e))?
            .to_string_lossy();

        let _ = std::fs::remove_file(&temp_path);

        if text.is_empty() {
            return Err("OCR returned empty text".to_string());
        }
        Ok(text)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Windows.Media.Ocr is only available on Windows".to_string())
    }
}
