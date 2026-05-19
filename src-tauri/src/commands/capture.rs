use base64::Engine;
use screenshots::Screen;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::io::Cursor;
use std::path::PathBuf;
use tauri::command;
use uuid::Uuid;

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

/// Generate a unique temp file path for OCR to avoid race conditions
fn unique_ocr_temp_path() -> PathBuf {
    let id = Uuid::new_v4().to_string();
    std::env::temp_dir().join(format!("moontranslator_ocr_{}.png", id))
}

/// RAII guard that deletes a temp file on drop.
/// Ensures cleanup even on early `?` return paths.
struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

// ── In-memory snapshot cache ───────────────────────────────────────────────
// Avoids slow disk I/O when the selector window loads the screenshot.

static SNAPSHOT_CACHE: std::sync::OnceLock<std::sync::Mutex<Option<(Vec<u8>, ScreenshotSnapshotInfo)>>> =
    std::sync::OnceLock::new();

fn snapshot_cache() -> &'static std::sync::Mutex<Option<(Vec<u8>, ScreenshotSnapshotInfo)>> {
    SNAPSHOT_CACHE.get_or_init(|| std::sync::Mutex::new(None))
}

fn cache_snapshot(png_bytes: Vec<u8>, info: &ScreenshotSnapshotInfo) {
    if let Ok(mut cache) = snapshot_cache().lock() {
        *cache = Some((png_bytes, info.clone()));
    }
}

fn read_cached_snapshot() -> Option<ScreenshotSnapshot> {
    let cache = snapshot_cache().lock().ok()?;
    let (ref png, ref info) = cache.as_ref()?;
    Some(ScreenshotSnapshot {
        image: encode_png_bytes(png),
        info: info.clone(),
    })
}

#[cfg(target_os = "windows")]
pub fn capture_area_gdi(
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
fn primary_screen_info() -> Result<ScreenshotSnapshotInfo, String> {
    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;
    let screen = screens
        .first()
        .ok_or_else(|| "No screen found".to_string())?;
    Ok(ScreenshotSnapshotInfo {
        screen_x: screen.display_info.x,
        screen_y: screen.display_info.y,
        screen_width: screen.display_info.width,
        screen_height: screen.display_info.height,
        scale_factor: screen.display_info.scale_factor,
        image_width: screen.display_info.width,
        image_height: screen.display_info.height,
    })
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
    // Use spawn_blocking to avoid blocking the async runtime with GDI calls
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let img = capture_area_gdi(x, y, width, height)?;
            return image_to_base64_png(&img);
        }

        #[cfg(not(target_os = "windows"))]
        {
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
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[command]
pub async fn capture_full_screen() -> Result<String, String> {
    // Use spawn_blocking to avoid blocking the async runtime with GDI calls
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let info = primary_screen_info()?;
            let img = capture_area_gdi(
                info.screen_x,
                info.screen_y,
                info.screen_width,
                info.screen_height,
            )?;
            return image_to_base64_png(&img);
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

#[command]
pub async fn prepare_screenshot_snapshot() -> Result<ScreenshotSnapshotInfo, String> {
    // Use spawn_blocking to avoid blocking the async runtime with GDI calls
    tokio::task::spawn_blocking(move || {
        // Capture the screen (platform-specific)
        #[cfg(target_os = "windows")]
        let (info, png_bytes) = {
            tracing::info!("prepare_screenshot_snapshot: capturing primary screen");
            let mut info = primary_screen_info()?;
            tracing::info!("prepare_screenshot_snapshot: screen info {:?}", info);
            let img = capture_area_gdi(
                info.screen_x,
                info.screen_y,
                info.screen_width,
                info.screen_height,
            )?;
            // Update with actual captured image dimensions (may differ from logical screen size on DPI-scaled displays)
            info.image_width = img.width();
            info.image_height = img.height();
            tracing::info!(
                "prepare_screenshot_snapshot: actual image size {}x{}",
                info.image_width,
                info.image_height
            );
            let mut buf = std::io::Cursor::new(Vec::new());
            img.write_to(&mut buf, screenshots::image::ImageFormat::Png)
                .map_err(|e| format!("PNG encode: {}", e))?;
            (info, buf.into_inner())
        };

        #[cfg(not(target_os = "windows"))]
        let (info, png_bytes) = {
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
            let mut buf = std::io::Cursor::new(Vec::new());
            screenshots::image::DynamicImage::ImageRgba8(buffer)
                .write_to(&mut buf, screenshots::image::ImageFormat::Png)
                .map_err(|e| format!("PNG encode: {}", e))?;
            (info, buf.into_inner())
        };

        // Save to disk as backup first (uses reference, no clone needed)
        if let Err(e) = std::fs::write(ocr_snapshot_image_path(), &png_bytes) {
            tracing::warn!("Failed to save OCR snapshot image: {}", e);
        }
        if let Ok(meta) = serde_json::to_vec(&info) {
            if let Err(e) = std::fs::write(ocr_snapshot_meta_path(), meta) {
                tracing::warn!("Failed to save OCR snapshot metadata: {}", e);
            }
        }

        let size_kb = png_bytes.len() / 1024;

        // Cache in memory for instant access by the selector window (moves bytes, no clone)
        cache_snapshot(png_bytes, &info);

        tracing::info!("prepare_screenshot_snapshot: done ({}KB cached)", size_kb);
        Ok(info)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[command]
pub async fn load_screenshot_snapshot() -> Result<ScreenshotSnapshot, String> {
    // Check in-memory cache first (instant, no disk I/O)
    if let Some(cached) = read_cached_snapshot() {
        return Ok(cached);
    }

    // Fallback to disk read
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
    // Use spawn_blocking to avoid blocking async runtime with disk I/O and image processing
    tokio::task::spawn_blocking(move || {
        let raw = std::fs::read(ocr_snapshot_image_path())
            .map_err(|e| format!("Failed to read screenshot snapshot: {}", e))?;
        let image = screenshots::image::load_from_memory(&raw)
            .map_err(|e| format!("Failed to load screenshot snapshot: {}", e))?;
        crop_image_to_base64(&image, left, top, width, height)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[command]
pub async fn capture_screenshot_region(
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    // Use spawn_blocking to avoid blocking the async runtime with GDI calls
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let img = capture_area_gdi(left as i32, top as i32, width, height)?;
            return image_to_base64_png(&img);
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
            let image = screenshots::image::DynamicImage::ImageRgba8(buffer);
            crop_image_to_base64(&image, left, top, width, height)
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

/// Per-line OCR result with bounding box.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrWordResult {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// One OCR line with bounding box and words.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrLineResult {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub words: Vec<OcrWordResult>,
}

/// Full OCR result with per-line details.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResultDetailed {
    pub lines: Vec<OcrLineResult>,
    pub text: String,
}

/// Run Windows.Media.Ocr on a base64 PNG data-URL.
/// Returns recognized text or error.
#[command]
pub async fn system_ocr(base64_data: String, lang: Option<String>) -> Result<String, String> {
    tracing::info!("[WinRT OCR] Starting OCR recognition");
    #[cfg(target_os = "windows")]
    {
        use windows::core::HSTRING;
        use windows::Globalization::Language;
        use windows::Graphics::Imaging::BitmapDecoder;
        use windows::Media::Ocr::OcrEngine;
        use windows::Storage::{FileAccessMode, StorageFile};

        // Decode to temp file (WinRT needs StorageFile path)
        let raw = decode_base64_png(&base64_data)?;
        tracing::info!("[WinRT OCR] Image decoded: {} bytes", raw.len());
        let temp_path = unique_ocr_temp_path();
        std::fs::write(&temp_path, &raw).map_err(|e| format!("Temp write failed: {}", e))?;
        let _guard = TempFileGuard(temp_path.clone());

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

        if text.is_empty() {
            tracing::warn!("[WinRT OCR] OCR returned empty text");
            return Err("OCR returned empty text".to_string());
        }
        tracing::info!("[WinRT OCR] Success: {} chars", text.len());
        Ok(text)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Windows.Media.Ocr is only available on Windows".to_string())
    }
}

/// Run Windows.Media.Ocr on a base64 PNG data-URL, returning per-line details.
/// Returns structured OCR result with bounding boxes for each detected line.
/// Note: OcrLine bounding rect is computed from word bounding rects (union).
#[command]
pub async fn system_ocr_detailed(
    base64_data: String,
    lang: Option<String>,
) -> Result<OcrResultDetailed, String> {
    tracing::info!("[WinRT OCR Detailed] Starting detailed OCR recognition");
    #[cfg(target_os = "windows")]
    {
        use windows::core::HSTRING;
        use windows::Globalization::Language;
        use windows::Graphics::Imaging::BitmapDecoder;
        use windows::Media::Ocr::OcrEngine;
        use windows::Storage::{FileAccessMode, StorageFile};

        // Decode to temp file (WinRT needs StorageFile path)
        let raw = decode_base64_png(&base64_data)?;
        tracing::info!("[WinRT OCR Detailed] Image decoded: {} bytes", raw.len());
        let temp_path = unique_ocr_temp_path();
        std::fs::write(&temp_path, &raw).map_err(|e| format!("Temp write failed: {}", e))?;
        let _guard = TempFileGuard(temp_path.clone());

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

        let lines_vec = result
            .Lines()
            .map_err(|e| format!("Lines: {}", e))?;

        let count = lines_vec.Size().map_err(|e| format!("Lines.Size: {}", e))?;
        let mut line_results = Vec::with_capacity(count as usize);

        for i in 0..count {
            let line = lines_vec
                .GetAt(i)
                .map_err(|e| format!("Lines.GetAt({}): {}", i, e))?;

            let line_text = line
                .Text()
                .map_err(|e| format!("Line.Text: {}", e))?
                .to_string_lossy();

            let words_vec = line
                .Words()
                .map_err(|e| format!("Words: {}", e))?;
            let word_count = words_vec.Size().map_err(|e| format!("Words.Size: {}", e))?;

            let mut word_results = Vec::with_capacity(word_count as usize);
            let mut min_x = f64::MAX;
            let mut min_y = f64::MAX;
            let mut max_r = f64::MIN;
            let mut max_b = f64::MIN;

            for j in 0..word_count {
                let word = words_vec
                    .GetAt(j)
                    .map_err(|e| format!("Words.GetAt({}): {}", j, e))?;
                let wtext = word
                    .Text()
                    .map_err(|e| format!("Word.Text: {}", e))?
                    .to_string_lossy();
                let wrect = word
                    .BoundingRect()
                    .map_err(|e| format!("Word.BoundingRect: {}", e))?;
                let wx = wrect.X as f64;
                let wy = wrect.Y as f64;
                let ww = wrect.Width as f64;
                let wh = wrect.Height as f64;

                // Track line bounding box (union of word rects)
                if wx < min_x { min_x = wx; }
                if wy < min_y { min_y = wy; }
                if wx + ww > max_r { max_r = wx + ww; }
                if wy + wh > max_b { max_b = wy + wh; }

                word_results.push(OcrWordResult {
                    text: wtext,
                    x: wx,
                    y: wy,
                    width: ww,
                    height: wh,
                });
            }

            let (line_x, line_y, line_w, line_h) = if word_count > 0 {
                (min_x, min_y, max_r - min_x, max_b - min_y)
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };

            line_results.push(OcrLineResult {
                text: line_text,
                x: line_x,
                y: line_y,
                width: line_w,
                height: line_h,
                words: word_results,
            });
        }

        let full_text = line_results
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        if full_text.is_empty() {
            tracing::warn!("[WinRT OCR Detailed] OCR returned empty text");
            return Err("OCR returned empty text".to_string());
        }

        tracing::info!("[WinRT OCR Detailed] Success: {} lines, {} chars total", line_results.len(), full_text.len());
        Ok(OcrResultDetailed {
            text: full_text,
            lines: line_results,
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Windows.Media.Ocr is only available on Windows".to_string())
    }
}

// ── Youdao OCR ─────────────────────────────────────────────────────────────

fn youdao_truncate(q: &str) -> String {
    let chars: Vec<char> = q.chars().collect();
    let size = chars.len();
    if size <= 20 {
        return format!("{}{}{}", q, size, q);
    }
    let first: String = chars[..10].iter().collect();
    let last: String = chars[size - 10..].iter().collect();
    format!("{}{}{}", first, size, last)
}

fn youdao_sign(app_key: &str, input: &str, salt: &str, curtime: &str, app_secret: &str) -> String {
    let sign_input = format!(
        "{}{}{}{}{}",
        app_key,
        youdao_truncate(input),
        salt,
        curtime,
        app_secret
    );
    let hash = sha2::Sha256::digest(sign_input.as_bytes());
    format!("{:x}", hash)
}

fn youdao_curtime() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn extract_bounding_box(item: &serde_json::Value) -> (f64, f64, f64, f64) {
    // Try various bounding box formats
    if let Some(bounding) = item.get("bounding") {
        // Format: { "bounding": { "x": 0, "y": 0, "width": 100, "height": 20 } }
        let x = bounding.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = bounding.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let w = bounding.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let h = bounding.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0);
        return (x, y, w, h);
    }
    if let Some(rect) = item.get("rect") {
        // Format: { "rect": { "left": 0, "top": 0, "right": 100, "bottom": 20 } }
        let left = rect.get("left").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let top = rect.get("top").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let right = rect.get("right").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let bottom = rect.get("bottom").and_then(|v| v.as_f64()).unwrap_or(0.0);
        return (left, top, right - left, bottom - top);
    }
    // Try direct fields
    let x = item.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let y = item.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let w = item.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let h = item.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0);
    (x, y, w, h)
}

/// Run Youdao OCR using ocrtran.youdao.com endpoint (same as YoudaoDict).
/// Returns per-line details with bounding boxes.
#[command]
pub async fn youdao_ocr(base64_data: String, lang: Option<String>, app_key: Option<String>, app_secret: Option<String>) -> Result<OcrResultDetailed, String> {
    let raw = decode_base64_png(&base64_data)?;

    // Compress image if too large
    let image_bytes = if raw.len() > 500 * 1024 {  // > 500KB
        tracing::info!("[Youdao OCR] Image too large ({}KB), compressing...", raw.len() / 1024);
        let img = screenshots::image::load_from_memory(&raw)
            .map_err(|e| format!("Failed to load image for compression: {}", e))?;

        let max_dim = 2000u32;
        let (w, h) = (img.width(), img.height());
        let scale = if w > h {
            max_dim as f64 / w as f64
        } else {
            max_dim as f64 / h as f64
        };

        let resized = if scale < 1.0 {
            let new_w = (w as f64 * scale) as u32;
            let new_h = (h as f64 * scale) as u32;
            img.resize(new_w, new_h, screenshots::image::imageops::FilterType::Lanczos3)
        } else {
            img
        };

        let mut buf = std::io::Cursor::new(Vec::new());
        resized.write_to(&mut buf, screenshots::image::ImageFormat::Jpeg)
            .map_err(|e| format!("Failed to compress image: {}", e))?;
        let compressed = buf.into_inner();
        tracing::info!("[Youdao OCR] Compressed to {}KB", compressed.len() / 1024);
        compressed
    } else {
        raw
    };

    // Use YoudaoDict's OCR endpoint: https://ocrtran.youdao.com/ocr/imgtranocr
    let endpoint = "https://ocrtran.youdao.com/ocr/imgtranocr";

    let lang_from = match lang.as_deref() {
        Some("zh") | Some("zh-CN") | Some("zh-CHS") => "zh-CHS",
        Some("en") => "en",
        Some("ja") => "ja",
        Some("ko") => "ko",
        _ => "AUTO",
    };

    tracing::info!("[Youdao OCR] Using ocrtran.youdao.com endpoint");
    tracing::info!("[Youdao OCR] Image size: {}KB, lang: {}", image_bytes.len() / 1024, lang_from);

    // Generate signing parameters
    let salt = uuid::Uuid::new_v4().to_string();
    let curtime = youdao_curtime();
    // Use provided keys or fall back to defaults (YoudaoDict built-in keys)
    let app_key = app_key.unwrap_or_else(|| "3d9fa94028675971".to_string());
    let app_secret = app_secret.unwrap_or_else(|| "5X2CJlMERfGOkOP0PFqokVJkSgDIOD0p".to_string());

    // For OCR, input is empty string
    let sign = youdao_sign(&app_key, "", &salt, &curtime, &app_secret);

    tracing::info!("[Youdao OCR] Signing params: appKey={}, salt={}, curtime={}", &app_key[..8], &salt[..8], curtime);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;

    // Build multipart form data with all required parameters
    // Convert image to base64 for the request
    let image_base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &image_bytes);
    let mystic_time = curtime.clone();
    let key_id = "ocr-web";

    let form = reqwest::multipart::Form::new()
        .text("langFrom", lang_from.to_string())
        .text("langTo", "auto".to_string())
        .text("product", "pc".to_string())
        .text("clientele", "pc".to_string())
        .text("appVersion", "11.2.12.0".to_string())
        .text("appKey", app_key.to_string())
        .text("salt", salt)
        .text("curtime", curtime)
        .text("sign", sign)
        .text("signType", "v3".to_string())
        .text("keyid", key_id.to_string())
        .text("mysticTime", mystic_time)
        .text("imageBase64", image_base64);

    tracing::info!("[Youdao OCR] Sending request to {}", endpoint);

    let resp = match client.post(endpoint).multipart(form).send().await {
        Ok(r) => r,
        Err(e) => {
            let err_msg = format!("Request failed: {}", e);
            tracing::error!("[Youdao OCR] {}", err_msg);
            return Err(err_msg);
        }
    };

    let status = resp.status();
    tracing::info!("[Youdao OCR] Response status: {}", status);

    let body = resp.text().await.map_err(|e| format!("Body read: {}", e))?;
    tracing::info!("[Youdao OCR] Response body ({} bytes): {}", body.len(), &body[..body.len().min(500)]);

    // Parse response - try multiple formats
    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("JSON parse: {}", e))?;

    // Check for error codes
    if let Some(code) = json.get("errorCode").and_then(|v| v.as_str()) {
        if code != "0" && code != "true" {
            return Err(format!("Youdao OCR errorCode={}", code));
        }
    }

    if let Some(code) = json.get("code").and_then(|v| v.as_i64()) {
        if code != 0 && code != 200 {
            return Err(format!("Youdao OCR code={}", code));
        }
    }

    // Extract OCR results from various response formats
    let mut lines: Vec<OcrLineResult> = Vec::new();
    let mut full_text = String::new();

    // Format 1: { "Result": [ { "text": "...", "bounding": {...} } ] }
    if let Some(result) = json.get("Result").and_then(|v| v.as_array()) {
        tracing::info!("[Youdao OCR] Parsing Result array ({} items)", result.len());
        for item in result {
            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                if !text.trim().is_empty() {
                    let (x, y, w, h) = extract_bounding_box(item);
                    lines.push(OcrLineResult {
                        text: text.to_string(),
                        x,
                        y,
                        width: w,
                        height: h,
                        words: vec![],
                    });
                    if !full_text.is_empty() {
                        full_text.push('\n');
                    }
                    full_text.push_str(text);
                }
            }
        }
    }
    // Format 2: { "result": { "regions": [...] } } or { "result": [...] }
    else if let Some(result) = json.get("result") {
        if let Some(arr) = result.as_array() {
            tracing::info!("[Youdao OCR] Parsing result array ({} items)", arr.len());
            for item in arr {
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    if !text.trim().is_empty() {
                        let (x, y, w, h) = extract_bounding_box(item);
                        lines.push(OcrLineResult {
                            text: text.to_string(),
                            x, y, width: w, height: h,
                            words: vec![],
                        });
                        if !full_text.is_empty() { full_text.push('\n'); }
                        full_text.push_str(text);
                    }
                }
            }
        } else if let Some(obj) = result.as_object() {
            tracing::info!("[Youdao OCR] Parsing result object");
            if let Some(regions) = obj.get("regions").and_then(|v| v.as_array()) {
                for region in regions {
                    if let Some(text) = region.get("text").and_then(|v| v.as_str()) {
                        if !text.trim().is_empty() {
                            let (x, y, w, h) = extract_bounding_box(region);
                            lines.push(OcrLineResult {
                                text: text.to_string(),
                                x, y, width: w, height: h,
                                words: vec![],
                            });
                            if !full_text.is_empty() { full_text.push('\n'); }
                            full_text.push_str(text);
                        }
                    }
                }
            }
        }
    }
    // Format 3: { "lines": [...] }
    else if let Some(lines_arr) = json.get("lines").and_then(|v| v.as_array()) {
        tracing::info!("[Youdao OCR] Parsing lines array ({} items)", lines_arr.len());
        for line in lines_arr {
            if let Some(text) = line.get("text").and_then(|v| v.as_str()) {
                if !text.trim().is_empty() {
                    let (x, y, w, h) = extract_bounding_box(line);
                    lines.push(OcrLineResult {
                        text: text.to_string(),
                        x, y, width: w, height: h,
                        words: vec![],
                    });
                    if !full_text.is_empty() { full_text.push('\n'); }
                    full_text.push_str(text);
                }
            }
        }
    }
    // Format 4: { "text": "..." } flat text
    else if let Some(text) = json.get("text").and_then(|v| v.as_str()) {
        tracing::info!("[Youdao OCR] Parsing flat text ({} chars)", text.len());
        if !text.trim().is_empty() {
            lines.push(OcrLineResult {
                text: text.to_string(),
                x: 0.0, y: 0.0, width: 0.0, height: 0.0,
                words: vec![],
            });
            full_text = text.to_string();
        }
    }
    // Format 5: top-level array
    else if let Some(arr) = json.as_array() {
        tracing::info!("[Youdao OCR] Parsing top-level array ({} items)", arr.len());
        for item in arr {
            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                if !text.trim().is_empty() {
                    let (x, y, w, h) = extract_bounding_box(item);
                    lines.push(OcrLineResult {
                        text: text.to_string(),
                        x, y, width: w, height: h,
                        words: vec![],
                    });
                    if !full_text.is_empty() { full_text.push('\n'); }
                    full_text.push_str(text);
                }
            }
        }
    }

    if lines.is_empty() {
        tracing::warn!("[Youdao OCR] No text extracted, response keys: {:?}",
            json.as_object().map(|m| m.keys().collect::<Vec<_>>()));
        return Err("OCR returned empty result".to_string());
    }

    tracing::info!("[Youdao OCR] Success: {} lines, {} chars total", lines.len(), full_text.len());
    Ok(OcrResultDetailed { text: full_text, lines })
}

/// A detected text region in screen coordinates.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRegion {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub line_count: usize,
    pub text_preview: String,
}

/// Auto-detect text regions in the foreground window by running OCR and clustering lines.
#[command]
pub async fn detect_text_regions(hwnd: Option<isize>) -> Result<Vec<TextRegion>, String> {
    // 1. Get target window handle
    let target_hwnd = if let Some(h) = hwnd {
        h
    } else {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
            unsafe { GetForegroundWindow().0 as isize }
        }
        #[cfg(not(target_os = "windows"))]
        {
            return Err("Not supported on this platform".to_string());
        }
    };

    if target_hwnd == 0 {
        return Err("No foreground window".to_string());
    }

    // 2. Get window rect
    #[cfg(target_os = "windows")]
    let rect = {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
        use windows::Win32::Foundation::RECT;
        unsafe {
            let mut rc = RECT::default();
            let hwnd = HWND(target_hwnd as *mut _);
            if GetWindowRect(hwnd, &mut rc).is_err() {
                return Err("GetWindowRect failed".to_string());
            }
            (rc.left, rc.top, rc.right - rc.left, rc.bottom - rc.top)
        }
    };
    #[cfg(not(target_os = "windows"))]
    return Err("Not supported on this platform".to_string());

    let (win_x, win_y, win_w, win_h) = rect;
    if win_w <= 0 || win_h <= 0 {
        return Err("Window has zero size".to_string());
    }

    // Clamp to reasonable size to avoid huge screenshots
    let max_dim = 1920u32;
    let w = (win_w as u32).min(max_dim);
    let h = (win_h as u32).min(max_dim);

    // 3. Capture window screenshot
    let image = capture_area_gdi(win_x, win_y, w, h)
        .map_err(|e| format!("Screenshot failed: {}", e))?;

    // 4. Run OCR detailed
    let base64 = image_to_base64_png(&image)?;
    let ocr_result = system_ocr_detailed_inner(&base64, None).await?;

    if ocr_result.lines.is_empty() {
        return Ok(Vec::new());
    }

    // 5. Cluster lines into regions
    // Convert line bounding boxes to screen coordinates (relative to window)
    let mut ocr_lines: Vec<(i32, i32, i32, i32, String)> = ocr_result
        .lines
        .into_iter()
        .filter(|l| !l.text.trim().is_empty())
        .map(|l| {
            (
                l.x as i32 + win_x,
                l.y as i32 + win_y,
                l.width as i32,
                l.height as i32,
                l.text,
            )
        })
        .collect();

    if ocr_lines.is_empty() {
        return Ok(Vec::new());
    }

    // Sort by Y position
    ocr_lines.sort_by_key(|l| l.1);

    // Cluster: merge lines whose vertical gap < line_height * 1.5
    let mut regions: Vec<TextRegion> = Vec::new();
    let mut current_cluster: Vec<(i32, i32, i32, i32, String)> = Vec::new();

    for line in ocr_lines {
        if let Some(last) = current_cluster.last() {
            let last_bottom = last.1 + last.3;
            let gap = line.1 - last_bottom;
            let avg_height = (last.3 + line.3) / 2;

            if gap > avg_height * 3 / 2 {
                // Gap too large, flush current cluster
                regions.push(build_region(&current_cluster, win_x, win_y));
                current_cluster.clear();
            }
        }
        current_cluster.push(line);
    }
    if !current_cluster.is_empty() {
        regions.push(build_region(&current_cluster, win_x, win_y));
    }

    // Filter out tiny regions (likely noise)
    regions.retain(|r| r.width > 30 && r.height > 15);

    tracing::info!("[detect_text_regions] Found {} regions in window", regions.len());
    Ok(regions)
}

/// Internal: run system_ocr_detailed without being a Tauri command
fn system_ocr_detailed_inner<'a>(
    base64_data: &'a str,
    lang: Option<&'a str>,
) -> impl std::future::Future<Output = Result<OcrResultDetailed, String>> + 'a {
    // We use an async block to match the original function signature
    async move {
        #[cfg(target_os = "windows")]
        {
            use windows::core::HSTRING;
            use windows::Globalization::Language;
            use windows::Graphics::Imaging::BitmapDecoder;
            use windows::Media::Ocr::OcrEngine;
            use windows::Storage::{FileAccessMode, StorageFile};

            let raw = decode_base64_png(base64_data)?;
            let temp_path = unique_ocr_temp_path();
            std::fs::write(&temp_path, &raw).map_err(|e| format!("Failed to write temp file: {}", e))?;

            let path_str = temp_path.to_string_lossy().to_string();
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

            let engine = match lang {
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

            let lines_vec = result.Lines().map_err(|e| format!("Lines: {}", e))?;
            let count = lines_vec.Size().map_err(|e| format!("Lines.Size: {}", e))?;
            let mut result_lines = Vec::with_capacity(count as usize);
            let mut full_text = String::new();

            for i in 0..count {
                let line = lines_vec
                    .GetAt(i)
                    .map_err(|e| format!("Lines.GetAt({}): {}", i, e))?;

                let line_text = line
                    .Text()
                    .map_err(|e| format!("Line.Text: {}", e))?
                    .to_string_lossy();
                if line_text.is_empty() {
                    continue;
                }

                let words_vec = line.Words().map_err(|e| format!("Words: {}", e))?;
                let word_count = words_vec.Size().map_err(|e| format!("Words.Size: {}", e))?;

                let mut word_results = Vec::with_capacity(word_count as usize);
                let mut min_x = f64::MAX;
                let mut min_y = f64::MAX;
                let mut max_r = f64::MIN;
                let mut max_b = f64::MIN;

                for j in 0..word_count {
                    let word = words_vec
                        .GetAt(j)
                        .map_err(|e| format!("Words.GetAt({}): {}", j, e))?;
                    let wtext = word
                        .Text()
                        .map_err(|e| format!("Word.Text: {}", e))?
                        .to_string_lossy();
                    let wrect = word
                        .BoundingRect()
                        .map_err(|e| format!("Word.BoundingRect: {}", e))?;
                    let wx = wrect.X as f64;
                    let wy = wrect.Y as f64;
                    let ww = wrect.Width as f64;
                    let wh = wrect.Height as f64;

                    if wx < min_x { min_x = wx; }
                    if wy < min_y { min_y = wy; }
                    if wx + ww > max_r { max_r = wx + ww; }
                    if wy + wh > max_b { max_b = wy + wh; }

                    word_results.push(OcrWordResult {
                        text: wtext,
                        x: wx,
                        y: wy,
                        width: ww,
                        height: wh,
                    });
                }

                let (line_x, line_y, line_w, line_h) = if word_count > 0 {
                    (min_x, min_y, max_r - min_x, max_b - min_y)
                } else {
                    (0.0, 0.0, 0.0, 0.0)
                };

                if !full_text.is_empty() {
                    full_text.push('\n');
                }
                full_text.push_str(&line_text);

                result_lines.push(OcrLineResult {
                    text: line_text,
                    x: line_x,
                    y: line_y,
                    width: line_w,
                    height: line_h,
                    words: word_results,
                });
            }

            let _ = std::fs::remove_file(&temp_path);

            Ok(OcrResultDetailed {
                text: full_text,
                lines: result_lines,
            })
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (base64_data, lang);
            Err("WinRT OCR not available on this platform".to_string())
        }
    }
}

/// Build a TextRegion from a cluster of lines (in screen coordinates)
fn build_region(cluster: &[(i32, i32, i32, i32, String)], offset_x: i32, offset_y: i32) -> TextRegion {
    let min_x = cluster.iter().map(|l| l.0).min().unwrap_or(0);
    let min_y = cluster.iter().map(|l| l.1).min().unwrap_or(0);
    let max_x = cluster.iter().map(|l| l.0 + l.2).max().unwrap_or(0);
    let max_y = cluster.iter().map(|l| l.1 + l.3).max().unwrap_or(0);

    let preview: String = cluster
        .iter()
        .take(3)
        .map(|l| l.4.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    TextRegion {
        x: min_x - offset_x,
        y: min_y - offset_y,
        width: max_x - min_x,
        height: max_y - min_y,
        line_count: cluster.len(),
        text_preview: if preview.len() > 80 {
            format!("{}...", &preview[..80])
        } else {
            preview
        },
    }
}
