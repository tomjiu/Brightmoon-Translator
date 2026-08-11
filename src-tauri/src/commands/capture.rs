use base64::Engine;
#[cfg(not(target_os = "windows"))]
use screenshots::Screen;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::command;
use uuid::Uuid;

// Tier 4 P2: multi-monitor parallel capture types (used by monitor_enum_proc
// callback + capture_virtual_screen_parallel). Imported at module level so the
// extern "system" callback signature can reference them.
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{HDC, HMONITOR};

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
    /// Absolute path for FE `convertFileSrc` (pot-desktop path — no full-screen base64 IPC).
    pub image_path: String,
    pub info: ScreenshotSnapshotInfo,
}

/// pot-style: `CompressionType::Fast` for snapshot files (preview via asset protocol, not base64).
/// Crops / OCR still use base64 of small regions only.
fn encode_png_bytes_fast(image: &screenshots::image::DynamicImage) -> Result<Vec<u8>, String> {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    use image::ImageEncoder;

    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut buf = Vec::with_capacity((w as usize).saturating_mul(h as usize) / 2);
    {
        let encoder =
            PngEncoder::new_with_quality(&mut buf, CompressionType::Fast, FilterType::NoFilter);
        encoder
            .write_image(rgba.as_raw(), w, h, image::ExtendedColorType::Rgba8)
            .map_err(|e| format!("Failed to encode PNG: {e}"))?;
    }
    Ok(buf)
}

fn image_to_base64_png(image: &screenshots::image::DynamicImage) -> Result<String, String> {
    let raw = encode_png_bytes_fast(image)?;
    let base64_str = base64::engine::general_purpose::STANDARD.encode(raw);
    Ok(format!("data:image/png;base64,{base64_str}"))
}

fn ocr_snapshot_image_path() -> PathBuf {
    std::env::temp_dir().join("moontranslator_ocr_snapshot.png")
}

fn ocr_snapshot_meta_path() -> PathBuf {
    std::env::temp_dir().join("moontranslator_ocr_snapshot.json")
}

/// Unique path per capture so asset protocol / `WebView2` never serves a stale freeze
/// (same fixed filename + cache can paint black or old desktop).
fn ocr_snapshot_image_path_unique() -> PathBuf {
    let id = Uuid::new_v4().to_string();
    std::env::temp_dir().join(format!("moontranslator_ocr_snapshot_{id}.png"))
}

fn snapshot_path_string_for(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn snapshot_path_string() -> String {
    snapshot_path_string_for(&ocr_snapshot_image_path())
}

/// Generate a unique temp file path for OCR to avoid race conditions
fn unique_ocr_temp_path() -> PathBuf {
    let id = Uuid::new_v4().to_string();
    std::env::temp_dir().join(format!("moontranslator_ocr_{id}.png"))
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
// Includes timestamp for smart cache expiration (30 seconds default)

use std::time::{SystemTime, UNIX_EPOCH};

struct CachedSnapshot {
    png_bytes: Vec<u8>,
    /// Absolute path written for FE convertFileSrc (unique per capture when possible).
    image_path: PathBuf,
    /// Lazily decoded for crop (avoid re-decode PNG on every `crop_screenshot_snapshot`).
    decoded: Option<screenshots::image::DynamicImage>,
    info: ScreenshotSnapshotInfo,
    timestamp: u64, // Unix timestamp in seconds
}

static SNAPSHOT_CACHE: std::sync::OnceLock<std::sync::Mutex<Option<CachedSnapshot>>> =
    std::sync::OnceLock::new();

fn snapshot_cache() -> &'static std::sync::Mutex<Option<CachedSnapshot>> {
    SNAPSHOT_CACHE.get_or_init(|| std::sync::Mutex::new(None))
}

fn cache_snapshot(png_bytes: Vec<u8>, info: &ScreenshotSnapshotInfo, image_path: PathBuf) {
    if let Ok(mut cache) = snapshot_cache().lock() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        *cache = Some(CachedSnapshot {
            png_bytes,
            image_path,
            decoded: None,
            info: info.clone(),
            timestamp,
        });
    }
}

fn read_cached_snapshot_info() -> Option<ScreenshotSnapshotInfo> {
    let cache = snapshot_cache().lock().ok()?;
    cache.as_ref().map(|c| c.info.clone())
}

fn read_cached_snapshot() -> Option<ScreenshotSnapshot> {
    let mut cache = snapshot_cache().lock().ok()?;
    let c = cache.as_mut()?;
    if !c.image_path.exists() {
        if c.image_path.as_os_str().is_empty() {
            c.image_path = ocr_snapshot_image_path();
        }
        let _ = std::fs::write(&c.image_path, &c.png_bytes);
    }
    Some(ScreenshotSnapshot {
        image_path: snapshot_path_string_for(&c.image_path),
        info: c.info.clone(),
    })
}

/// Check if cache is fresh (within 30 seconds)
fn is_cache_fresh(max_age_secs: u64) -> bool {
    if let Ok(cache) = snapshot_cache().lock() {
        if let Some(ref cached) = *cache {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            return (now - cached.timestamp) < max_age_secs;
        }
    }
    false
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

    // SAFETY: GDI screen capture. All GDI objects are properly cleaned up on error paths.
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
            return Err(format!("BitBlt failed: {err}"));
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
            Some(bgra.as_mut_ptr().cast()),
            &raw mut info,
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
fn virtual_screen_info() -> Result<ScreenshotSnapshotInfo, String> {
    // Use Windows virtual-screen metrics so OCR selection can span monitors
    // positioned left of or above the primary display.
    extern "system" {
        fn GetSystemMetrics(nIndex: i32) -> i32;
        fn GetDpiForSystem() -> u32;
    }

    const SM_XVIRTUALSCREEN: i32 = 76;
    const SM_YVIRTUALSCREEN: i32 = 77;
    const SM_CXVIRTUALSCREEN: i32 = 78;
    const SM_CYVIRTUALSCREEN: i32 = 79;

    let screen_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let screen_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let physical_w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let physical_h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    if physical_w <= 0 || physical_h <= 0 {
        return Err("Virtual screen has invalid dimensions".to_string());
    }

    let dpi = unsafe { GetDpiForSystem() };
    let scale_factor = dpi as f32 / 96.0;

    tracing::info!(
        "virtual_screen_info: origin=({}, {}), physical={}x{}, dpi={}, scale={}",
        screen_x,
        screen_y,
        physical_w,
        physical_h,
        dpi,
        scale_factor
    );

    Ok(ScreenshotSnapshotInfo {
        screen_x,
        screen_y,
        screen_width: physical_w as u32,
        screen_height: physical_h as u32,
        scale_factor,
        image_width: physical_w as u32,
        image_height: physical_h as u32,
    })
}

// ── Tier 4 P2: Multi-monitor parallel capture ──────────────────────────────
// Captures each monitor from its own device DC (CreateDCW) in parallel scoped
// threads, then composites into a single virtual-desktop image. Short-circuits
// to a single capture_area_gdi when only one monitor is present.
//
// Benefit over the single virtual-screen BitBlt: each monitor is captured at
// its native physical resolution via its own device DC, which is correct for
// mixed-DPI multi-monitor rigs and multi-GPU setups where the virtual-screen
// DC may not span all adapters.

#[cfg(target_os = "windows")]
struct PhysicalMonitor {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    device_name: [u16; 32],
}

// SAFETY: EnumDisplayMonitors callback. Writes each monitor's rect + device
// name (via GetMonitorInfoW) into the Vec passed through dwdata. Only touches
// already-mapped memory; safe under the enum call.
#[cfg(target_os = "windows")]
unsafe extern "system" fn monitor_enum_proc(
    hmon: HMONITOR,
    _hdc: HDC,
    lprc: *mut RECT,
    dwdata: LPARAM,
) -> BOOL {
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFOEXW};
    let monitors = &mut *(dwdata.0 as *mut Vec<PhysicalMonitor>);
    if lprc.is_null() {
        return BOOL(1);
    }
    let r = &*lprc;
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    // Cast *mut MONITORINFOEXW → *mut MONITORINFO (first field, #[repr(C)]).
    if GetMonitorInfoW(hmon, (&raw mut info).cast()).as_bool() {
        monitors.push(PhysicalMonitor {
            x: r.left,
            y: r.top,
            width: (r.right - r.left) as u32,
            height: (r.bottom - r.top) as u32,
            device_name: info.szDevice,
        });
    }
    BOOL(1) // continue enumeration
}

#[cfg(target_os = "windows")]
fn enumerate_physical_monitors() -> Vec<PhysicalMonitor> {
    use windows::Win32::Graphics::Gdi::EnumDisplayMonitors;
    let mut monitors: Vec<PhysicalMonitor> = Vec::new();
    // SAFETY: EnumDisplayMonitors with None DC enumerates all monitors. The
    // callback writes to the Vec passed via dwdata LPARAM; no shared mutable
    // state outside that Vec. MONITORENUMPROC is a type alias for
    // Option<unsafe extern "system" fn(...)>, so we pass Some(fn) directly —
    // it is NOT a tuple-struct constructor.
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(monitor_enum_proc),
            LPARAM(&raw mut monitors as isize),
        );
    }
    monitors
}

/// Capture a single monitor at its native resolution via its own device DC.
#[cfg(target_os = "windows")]
fn capture_monitor_dc(monitor: &PhysicalMonitor) -> Result<screenshots::image::DynamicImage, String> {
    use screenshots::image::{ImageBuffer, Rgba};
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateDCW, DeleteDC, DeleteObject,
        GetDIBits, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
        SRCCOPY,
    };

    let width = monitor.width;
    let height = monitor.height;
    if width == 0 || height == 0 {
        return Err("Monitor capture area is empty".to_string());
    }

    // SAFETY: GDI per-monitor capture. CreateDCW creates a DC for the specific
    // monitor device, capturing at its native physical resolution. All GDI
    // objects are released on every path (including error).
    unsafe {
        // windows 0.58: CreateDCW returns HDC directly (not Result). A null HDC
        // means the device DC could not be created (e.g. transient GDI pressure).
        let screen_dc = CreateDCW(PCWSTR(monitor.device_name.as_ptr()), None, None, None);
        if screen_dc.0.is_null() {
            return Err("CreateDCW for monitor failed".to_string());
        }

        let mem_dc = CreateCompatibleDC(screen_dc);
        if mem_dc.0.is_null() {
            let _ = DeleteDC(screen_dc);
            return Err("CreateCompatibleDC failed".to_string());
        }

        let bitmap = CreateCompatibleBitmap(screen_dc, width as i32, height as i32);
        if bitmap.0.is_null() {
            let _ = DeleteDC(mem_dc);
            let _ = DeleteDC(screen_dc);
            return Err("CreateCompatibleBitmap failed".to_string());
        }

        let old_object = SelectObject(mem_dc, HGDIOBJ(bitmap.0));
        if old_object.0.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(mem_dc);
            let _ = DeleteDC(screen_dc);
            return Err("SelectObject failed".to_string());
        }

        let blt_ok =
            BitBlt(mem_dc, 0, 0, width as i32, height as i32, screen_dc, 0, 0, SRCCOPY).is_ok();

        if !blt_ok {
            let _ = SelectObject(mem_dc, old_object);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(mem_dc);
            let _ = DeleteDC(screen_dc);
            return Err("BitBlt failed".to_string());
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
            Some(bgra.as_mut_ptr().cast()),
            &raw mut info,
            DIB_RGB_COLORS,
        );

        let _ = SelectObject(mem_dc, old_object);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(mem_dc);
        let _ = DeleteDC(screen_dc);

        if rows == 0 {
            return Err("GetDIBits failed".to_string());
        }

        for px in bgra.chunks_exact_mut(4) {
            px.swap(0, 2); // BGRA → RGBA
        }

        let image = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, bgra)
            .ok_or_else(|| "Failed to construct monitor image buffer".to_string())?;
        Ok(screenshots::image::DynamicImage::ImageRgba8(image))
    }
}

/// Capture the full virtual desktop, using parallel per-monitor capture when
/// multiple monitors are present. Returns the screen info + composite image.
#[cfg(target_os = "windows")]
fn capture_virtual_screen_parallel(
) -> Result<(ScreenshotSnapshotInfo, screenshots::image::DynamicImage), String> {
    let vs = virtual_screen_info()?;
    let monitors = enumerate_physical_monitors();

    // Single-monitor short-circuit: one BitBlt of the whole virtual screen is
    // simpler and faster than spawning a thread + compositing.
    if monitors.len() <= 1 {
        let img = capture_area_gdi(vs.screen_x, vs.screen_y, vs.screen_width, vs.screen_height)?;
        return Ok((vs, img));
    }

    tracing::info!(
        "capture_virtual_screen_parallel: {} monitors, capturing in parallel",
        monitors.len()
    );

    // Parallel per-monitor capture via scoped threads (no rayon dependency).
    // Each thread creates its own DC via CreateDCW, so there is no GDI handle
    // contention. A failed monitor is skipped (its region stays black in the
    // composite); if ALL fail, we fall back to a single virtual-screen BitBlt.
    let captures: Vec<(usize, Result<screenshots::image::DynamicImage, String>)> =
        std::thread::scope(|s| {
            let handles: Vec<_> = monitors
                .iter()
                .enumerate()
                .map(|(i, m)| s.spawn(move || (i, capture_monitor_dc(m))))
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });

    // Composite per-monitor images into the virtual-desktop master image.
    use screenshots::image::{ImageBuffer, Rgba};
    let mut master = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(vs.screen_width, vs.screen_height);

    let mut ok_count = 0usize;
    for (i, result) in captures {
        match result {
            Ok(img) => {
                let m = &monitors[i];
                let dx = (m.x - vs.screen_x).max(0) as u32;
                let dy = (m.y - vs.screen_y).max(0) as u32;
                // Composite per-monitor frame onto the virtual-desktop master.
                // Manual pixel copy avoids the imageops::replace generic-bound
                // friction across the screenshots-bundled image 0.24 re-export.
                // Per-pixel put_pixel is fine here: a one-time composite, and
                // each failed monitor simply leaves its tile black.
                let region_img = img.to_rgba8();
                for (rx, ry, pixel) in region_img.enumerate_pixels() {
                    let tx = dx + rx;
                    let ty = dy + ry;
                    if tx < master.width() && ty < master.height() {
                        master.put_pixel(tx, ty, *pixel);
                    }
                }
                ok_count += 1;
            }
            Err(e) => {
                tracing::warn!(
                    "capture_virtual_screen_parallel: monitor {} failed: {}",
                    i,
                    e
                );
            }
        }
    }

    tracing::info!(
        "capture_virtual_screen_parallel: composite done ({} ok, {} failed)",
        ok_count,
        monitors.len() - ok_count
    );

    if ok_count == 0 {
        tracing::warn!(
            "capture_virtual_screen_parallel: all monitors failed, falling back to single BitBlt"
        );
        let img = capture_area_gdi(vs.screen_x, vs.screen_y, vs.screen_width, vs.screen_height)?;
        return Ok((vs, img));
    }

    Ok((vs, screenshots::image::DynamicImage::ImageRgba8(master)))
}

#[cfg_attr(target_os = "windows", allow(dead_code))]
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

/// Pixel fingerprint for continuous OCR skip (better than base64 string sampling).
/// Downscale to 24×24 grayscale, FNV-ish hash over luminance samples.
fn fingerprint_dynamic_image(image: &screenshots::image::DynamicImage) -> String {
    use screenshots::image::GenericImageView;
    const GRID: u32 = 24;
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return String::new();
    }
    let mut hash: u32 = 2166136261;
    hash = hash.wrapping_mul(16777619) ^ w;
    hash = hash.wrapping_mul(16777619) ^ h;
    for gy in 0..GRID {
        for gx in 0..GRID {
            let x = (gx * w / GRID).min(w.saturating_sub(1));
            let y = (gy * h / GRID).min(h.saturating_sub(1));
            let p = image.get_pixel(x, y).0;
            // Rec. 601 luma
            let y8 = ((u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000) as u8;
            hash = hash.wrapping_mul(16777619) ^ u32::from(y8);
        }
    }
    format!("{w}x{h}:{hash:x}")
}

fn decode_data_url_image(data_url: &str) -> Result<screenshots::image::DynamicImage, String> {
    let b64 = data_url.split_once(',').map_or(data_url, |(_, b)| b);
    let raw = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("base64 decode: {e}"))?;
    screenshots::image::load_from_memory(&raw).map_err(|e| format!("image decode: {e}"))
}

/// Fingerprint a crop/region image (data URL) for watch-mode skip gate.
#[command]
pub async fn image_data_url_fingerprint(data_url: String) -> Result<String, String> {
    if data_url.is_empty() {
        return Ok(String::new());
    }
    tokio::task::spawn_blocking(move || {
        let image = decode_data_url_image(&data_url)?;
        Ok(fingerprint_dynamic_image(&image))
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[command]
pub async fn capture_screen(x: i32, y: i32, width: u32, height: u32) -> Result<String, String> {
    // Use spawn_blocking to avoid blocking the async runtime with GDI calls
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
    .map_err(|e| format!("Task join error: {e}"))?
}

#[command]
pub async fn prepare_screenshot_snapshot(
    force_refresh: Option<bool>,
) -> Result<ScreenshotSnapshotInfo, String> {
    // Smart cache only when NOT force-refreshing (warmup / non-user paths).
    // User-triggered OCR must pass force_refresh=true so the selector never shows a stale desktop.
    let force = force_refresh.unwrap_or(false);
    if !force && is_cache_fresh(30) {
        if let Some(info) = read_cached_snapshot_info() {
            tracing::info!("prepare_screenshot_snapshot: returning fresh cache (instant response)");
            return Ok(info);
        }
    }

    // Capture fresh screenshot
    tracing::info!(
        "prepare_screenshot_snapshot: capturing fresh screenshot (force={})",
        force
    );

    // Use spawn_blocking to avoid blocking the async runtime with GDI calls
    tokio::task::spawn_blocking(move || {
        // Capture the screen (platform-specific)
        #[cfg(target_os = "windows")]
        let (info, png_bytes) = {
            tracing::info!("prepare_screenshot_snapshot: capturing virtual screen");
            // Tier 4 P2: parallel per-monitor capture for multi-monitor rigs.
            // Single-monitor short-circuits to one BitBlt inside the helper.
            let mut info = virtual_screen_info()?;
            tracing::info!("prepare_screenshot_snapshot: screen info {:?}", info);
            let img = capture_virtual_screen_parallel()
                .map(|(_, image)| image)
                .or_else(|e| {
                    // Defensive: if the parallel path fails entirely, fall back
                    // to a single virtual-screen BitBlt so capture never breaks.
                    tracing::warn!(
                        "prepare_screenshot_snapshot: parallel capture failed ({}), \
                         falling back to single BitBlt",
                        e
                    );
                    capture_area_gdi(
                        info.screen_x,
                        info.screen_y,
                        info.screen_width,
                        info.screen_height,
                    )
                })?;
            // Update with actual captured image dimensions (may differ from logical screen size on DPI-scaled displays)
            info.image_width = img.width();
            info.image_height = img.height();
            tracing::info!(
                "prepare_screenshot_snapshot: actual image size {}x{}",
                info.image_width,
                info.image_height
            );
            let png_bytes = encode_png_bytes_fast(&img)?;
            (info, png_bytes)
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
            let dyn_img = screenshots::image::DynamicImage::ImageRgba8(buffer);
            let png_bytes = encode_png_bytes_fast(&dyn_img)?;
            (info, png_bytes)
        };

        // Unique file each capture so asset:// never hits a stale WebView2 cache entry.
        let image_path = ocr_snapshot_image_path_unique();
        if let Err(e) = std::fs::write(&image_path, &png_bytes) {
            tracing::warn!("Failed to save OCR snapshot image: {}", e);
        }
        // Also keep legacy fixed name for tools / fallbacks.
        let _ = std::fs::write(ocr_snapshot_image_path(), &png_bytes);
        if let Ok(meta) = serde_json::to_vec(&info) {
            if let Err(e) = std::fs::write(ocr_snapshot_meta_path(), meta) {
                tracing::warn!("Failed to save OCR snapshot metadata: {}", e);
            }
        }

        let size_kb = png_bytes.len() / 1024;

        // Cache in memory for instant access by the selector window (moves bytes, no clone)
        cache_snapshot(png_bytes, &info, image_path);

        tracing::info!("prepare_screenshot_snapshot: done ({}KB cached)", size_kb);
        Ok(info)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
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
        .map_err(|e| format!("Failed to read screenshot snapshot: {e}"))?;
    let info = if let Ok(meta) = std::fs::read(ocr_snapshot_meta_path()) {
        serde_json::from_slice::<ScreenshotSnapshotInfo>(&meta)
            .map_err(|e| format!("Failed to parse screenshot metadata: {e}"))?
    } else {
        let image = screenshots::image::load_from_memory(&raw)
            .map_err(|e| format!("Failed to inspect screenshot snapshot: {e}"))?;
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
        image_path: snapshot_path_string(),
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
    // Prefer decoded RGBA; hold lock only for decode + crop_imm; encode outside lock.
    tokio::task::spawn_blocking(move || {
        let cropped: screenshots::image::DynamicImage = {
            if let Ok(mut guard) = snapshot_cache().lock() {
                if let Some(ref mut cached) = *guard {
                    if cached.decoded.is_none() {
                        match screenshots::image::load_from_memory(&cached.png_bytes) {
                            Ok(img) => cached.decoded = Some(img),
                            Err(e) => {
                                return Err(format!("Failed to load screenshot snapshot: {e}"));
                            },
                        }
                    }
                    if let Some(ref decoded) = cached.decoded {
                        let l = left.min(decoded.width().saturating_sub(1));
                        let t = top.min(decoded.height().saturating_sub(1));
                        let w = width.min(decoded.width().saturating_sub(l)).max(1);
                        let h = height.min(decoded.height().saturating_sub(t)).max(1);
                        decoded.crop_imm(l, t, w, h)
                    } else {
                        drop(guard);
                        let raw = std::fs::read(ocr_snapshot_image_path())
                            .map_err(|e| format!("Failed to read screenshot snapshot: {e}"))?;
                        let image = screenshots::image::load_from_memory(&raw)
                            .map_err(|e| format!("Failed to load screenshot snapshot: {e}"))?;
                        let l = left.min(image.width().saturating_sub(1));
                        let t = top.min(image.height().saturating_sub(1));
                        let w = width.min(image.width().saturating_sub(l)).max(1);
                        let h = height.min(image.height().saturating_sub(t)).max(1);
                        image.crop_imm(l, t, w, h)
                    }
                } else {
                    drop(guard);
                    let raw = std::fs::read(ocr_snapshot_image_path())
                        .map_err(|e| format!("Failed to read screenshot snapshot: {e}"))?;
                    let image = screenshots::image::load_from_memory(&raw)
                        .map_err(|e| format!("Failed to load screenshot snapshot: {e}"))?;
                    let l = left.min(image.width().saturating_sub(1));
                    let t = top.min(image.height().saturating_sub(1));
                    let w = width.min(image.width().saturating_sub(l)).max(1);
                    let h = height.min(image.height().saturating_sub(t)).max(1);
                    image.crop_imm(l, t, w, h)
                }
            } else {
                let raw = std::fs::read(ocr_snapshot_image_path())
                    .map_err(|e| format!("Failed to read screenshot snapshot: {e}"))?;
                let image = screenshots::image::load_from_memory(&raw)
                    .map_err(|e| format!("Failed to load screenshot snapshot: {e}"))?;
                let l = left.min(image.width().saturating_sub(1));
                let t = top.min(image.height().saturating_sub(1));
                let w = width.min(image.width().saturating_sub(l)).max(1);
                let h = height.min(image.height().saturating_sub(t)).max(1);
                image.crop_imm(l, t, w, h)
            }
        };
        image_to_base64_png(&cropped)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[command]
pub async fn capture_screenshot_region(
    left: i32,
    top: i32,
    width: u32,
    height: u32,
) -> Result<String, String> {
    // Use spawn_blocking to avoid blocking the async runtime with GDI calls
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let img = capture_area_gdi(left, top, width, height)?;
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
            let image = screenshots::image::DynamicImage::ImageRgba8(buffer);
            crop_image_to_base64(&image, left.max(0) as u32, top.max(0) as u32, width, height)
        }
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
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
        // SAFETY: GetForegroundWindow returns valid HWND or NULL.
        // SAFETY: GetWindowRect is a standard Win32 API. Buffer is stack-allocated.
        // SAFETY: GetWindowRect is a standard Win32 API.
        unsafe {
            let hwnd = GetForegroundWindow();
            if !hwnd.is_null() {
                return Ok(hwnd as isize);
            }
        }
    }
    Ok(0)
}

/// Resolve the top-level window under a physical screen point.
/// Used by OCR follow-mode so binding targets the content under the region
/// instead of the always-on-top OCR frame itself.
#[command]
pub async fn hwnd_from_point(x: i32, y: i32) -> Result<isize, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, WindowFromPoint, GA_ROOT};

        // SAFETY: WindowFromPoint/GetAncestor are standard Win32 APIs.
        // Caller should hide OCR frame first so the hit test reaches content underneath.
        unsafe {
            let point = POINT { x, y };
            let mut hwnd = WindowFromPoint(point);
            if hwnd.0.is_null() {
                return Ok(0);
            }
            let root = GetAncestor(hwnd, GA_ROOT);
            if !root.0.is_null() {
                hwnd = root;
            }

            let mut title_buf = [0u16; 256];
            let title_len =
                windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(hwnd, &mut title_buf);
            if title_len > 0 {
                let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);
                // Keep in sync with window.rs titles (selector is "OCR-v2 Screenshot").
                if title == "OCR Region"
                    || title == "OCR Screenshot"
                    || title == "OCR-v2 Screenshot"
                    || title == "Moon Translator"
                {
                    return Ok(0);
                }
            }

            Ok(hwnd.0 as isize)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (x, y);
        Ok(0)
    }
}

/// Get the window title for a given HWND.
/// Returns the title string, or empty string if not found.
#[command]
pub async fn get_window_title_cmd(hwnd: isize) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::GetWindowTextW;
        // SAFETY: GetWindowTextW is a standard Win32 API. Buffer is stack-allocated.

        // SAFETY: GetWindowRect is a standard Win32 API. Buffer is stack-allocated.
        // SAFETY: GetWindowRect is a standard Win32 API.
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
        struct Rect {
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
        }
        extern "system" {
            fn GetWindowRect(hWnd: *mut std::ffi::c_void, lpRect: *mut Rect) -> i32;
        }
        // SAFETY: GetWindowRect is a standard Win32 API. Buffer is stack-allocated.
        // SAFETY: GetWindowRect is a standard Win32 API.
        unsafe {
            let mut rect = Rect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            let result = GetWindowRect(hwnd as *mut std::ffi::c_void, &raw mut rect);
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
        .map_err(|e| format!("Base64 decode failed: {e}"))
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

/// Synthetic per-line boxes for Rapid/Paddle (sidecars return plain text only).
/// Splits on newlines and stacks equal-height full-width bands so FE
/// `width > 0 && height > 0` overlays work better than a single 1×1 box.
pub fn synthetic_ocr_lines_from_text(text: &str, img_w: f64, img_h: f64) -> OcrResultDetailed {
    let img_w = img_w.max(1.0);
    let img_h = img_h.max(1.0);
    let parts: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let full = text.trim().to_string();
    if parts.is_empty() {
        if full.is_empty() {
            return OcrResultDetailed {
                lines: vec![],
                text: String::new(),
            };
        }
        return OcrResultDetailed {
            lines: vec![OcrLineResult {
                text: full.clone(),
                x: 0.0,
                y: 0.0,
                width: img_w,
                height: img_h,
                words: vec![],
            }],
            text: full,
        };
    }
    let n = parts.len() as f64;
    let line_h = (img_h / n).max(1.0);
    let lines: Vec<OcrLineResult> = parts
        .into_iter()
        .enumerate()
        .map(|(i, line)| OcrLineResult {
            text: line.to_string(),
            x: 0.0,
            y: (i as f64) * line_h,
            width: img_w,
            height: line_h,
            words: vec![],
        })
        .collect();
    let joined = lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    OcrResultDetailed {
        lines,
        text: if full.is_empty() { joined } else { full },
    }
}

fn png_dimensions(png_bytes: &[u8]) -> (f64, f64) {
    image::load_from_memory(png_bytes)
        .map_or((1.0, 1.0), |img| (f64::from(img.width()), f64::from(img.height())))
}

/// Offline Rapid/Paddle sidecar OCR for screenshot path (same as `image_translate`).
/// Sidecars lack boxes — synthesize stacked line bands from newlines + image size.
#[command]
pub async fn offline_ocr(
    base64_data: String,
    backend: Option<String>,
    plugin_dir: Option<String>,
    lang: Option<String>,
) -> Result<OcrResultDetailed, String> {
    let raw = decode_base64_png(&base64_data)?;
    let (img_w, img_h) = png_dimensions(&raw);
    let cfg = crate::config::AppConfig::load();
    let backend = backend
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| cfg.offline_ocr.backend.clone());
    let plugin_dir = plugin_dir
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| cfg.offline_ocr.plugin_dir.clone());
    let lang_owned = lang;
    let text = tokio::task::spawn_blocking(move || {
        crate::ocr_offline::run_offline_ocr(&raw, &backend, &plugin_dir, lang_owned.as_deref())
    })
    .await
    .map_err(|e| format!("Offline OCR join: {e}"))??;
    Ok(synthetic_ocr_lines_from_text(&text, img_w, img_h))
}

/// Run offline (Rapid/Paddle) OCR on raw PNG bytes and return synthetic
/// per-line boxes (sidecar backends return plain text only).
pub async fn run_offline_ocr_detailed(
    png_bytes: &[u8],
    backend: &str,
    plugin_dir: &str,
    lang: Option<String>,
    offset_x: f64,
    offset_y: f64,
) -> Result<OcrResultDetailed, String> {
    let (img_w, img_h) = png_dimensions(png_bytes);
    let backend_owned = backend.to_string();
    let plugin_dir_owned = plugin_dir.to_string();
    let png_owned = png_bytes.to_vec();
    let text = tokio::task::spawn_blocking(move || {
        crate::ocr_offline::run_offline_ocr(&png_owned, &backend_owned, &plugin_dir_owned, lang.as_deref())
    })
    .await
    .map_err(|e| format!("Offline OCR join: {e}"))??;
    let mut result = synthetic_ocr_lines_from_text(&text, img_w, img_h);
    if offset_x != 0.0 || offset_y != 0.0 {
        for line in &mut result.lines {
            line.x += offset_x;
            line.y += offset_y;
            for word in &mut line.words {
                word.x += offset_x;
                word.y += offset_y;
            }
        }
    }
    Ok(result)
}

/// Run `WinRT` OCR on raw PNG bytes (for use by other modules).
pub async fn run_winrt_ocr_detailed_from_bytes(
    png_bytes: &[u8],
    lang: Option<&str>,
) -> Result<OcrResultDetailed, String> {
    let base64_data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png_bytes);
    let data_url = format!("data:image/png;base64,{base64_data}");
    system_ocr_detailed(data_url, lang.map(std::string::ToString::to_string)).await
}

/// Run Youdao OCR on raw PNG bytes (for use by other modules).
/// Uses simpler ocrtransapi endpoint (no signing required).
pub async fn run_youdao_ocr_from_bytes(
    png_bytes: &[u8],
    lang: Option<String>,
    _app_key: Option<String>,
    _app_secret: Option<String>,
    _timeout_secs: u64,
) -> Result<OcrResultDetailed, String> {
    let base64_data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, png_bytes);
    let data_url = format!("data:image/png;base64,{base64_data}");

    // Create a minimal state for the Youdao OCR function
    // Since we can't easily create a Tauri State outside of Tauri, we'll call the internal implementation directly
    let raw = decode_base64_png(&data_url)?;

    // Compress image if too large
    let image_bytes = if raw.len() > 500 * 1024 {
        tracing::info!(
            "[Youdao OCR] Image too large ({}KB), compressing...",
            raw.len() / 1024
        );
        let img = screenshots::image::load_from_memory(&raw)
            .map_err(|e| format!("Failed to load image for compression: {e}"))?;

        let max_dim = 2000u32;
        let (w, h) = (img.width(), img.height());
        let scale = if w > h {
            f64::from(max_dim) / f64::from(w)
        } else {
            f64::from(max_dim) / f64::from(h)
        };

        let resized = if scale < 1.0 {
            let new_w = (f64::from(w) * scale) as u32;
            let new_h = (f64::from(h) * scale) as u32;
            img.resize(
                new_w,
                new_h,
                screenshots::image::imageops::FilterType::Lanczos3,
            )
        } else {
            img
        };

        let mut buf = std::io::Cursor::new(Vec::new());
        resized
            .write_to(&mut buf, screenshots::image::ImageFormat::Jpeg)
            .map_err(|e| format!("Failed to compress image: {e}"))?;
        let compressed = buf.into_inner();
        tracing::info!("[Youdao OCR] Compressed to {}KB", compressed.len() / 1024);
        compressed
    } else {
        raw
    };

    // Use simpler ocrtransapi endpoint (no signing required)
    let image_base64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &image_bytes);
    let lang_str = lang.unwrap_or_else(|| "auto".to_string());
    let form = reqwest::multipart::Form::new()
        .text("img", image_base64)
        .text("lang", lang_str)
        .text("type", "1")
        .text("docType", "json");

    let client = reqwest::Client::new();
    let resp = client
        .post("https://ocrtran.youdao.com/ocrtranapi")
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Youdao OCR request failed: {e}"))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {e}"))?;

    if !status.is_success() {
        return Err(format!("Youdao OCR returned status {status}: {body}"));
    }

    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse Youdao OCR response: {e}"))?;

    let mut lines = Vec::new();
    if let Some(regions) = json.get("regions").and_then(|r| r.as_array()) {
        for region in regions {
            if let Some(line_texts) = region.get("lines").and_then(|l| l.as_array()) {
                for line in line_texts {
                    let text = line
                        .get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    let x = line.get("x").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
                    let y = line.get("y").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
                    let width = line.get("width").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
                    let height = line.get("height").and_then(serde_json::Value::as_f64).unwrap_or(0.0);

                    lines.push(OcrLineResult {
                        text: text.clone(),
                        x,
                        y,
                        width,
                        height,
                        words: vec![OcrWordResult {
                            text,
                            x,
                            y,
                            width,
                            height,
                        }],
                    });
                }
            }
        }
    }

    let full_text = lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    Ok(OcrResultDetailed {
        lines,
        text: full_text,
    })
}

/// Run Windows.Media.Ocr on a base64 PNG data-URL, returning per-line details.
/// Returns structured OCR result with bounding boxes for each detected line.
/// Note: `OcrLine` bounding rect is computed from word bounding rects (union).
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
        std::fs::write(&temp_path, &raw).map_err(|e| format!("Temp write failed: {e}"))?;
        let _guard = TempFileGuard(temp_path.clone());

        let path_str = temp_path.to_string_lossy().replace("\\\\?\\", "");

        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(&path_str))
            .map_err(|e| format!("StorageFile: {e}"))?
            .get()
            .map_err(|e| format!("StorageFile await: {e}"))?;

        let stream = file
            .OpenAsync(FileAccessMode::Read)
            .map_err(|e| format!("OpenAsync: {e}"))?
            .get()
            .map_err(|e| format!("OpenAsync await: {e}"))?;

        let decoder = BitmapDecoder::CreateWithIdAsync(
            BitmapDecoder::PngDecoderId().map_err(|e| format!("PngDecoderId: {e}"))?,
            &stream,
        )
        .map_err(|e| format!("BitmapDecoder: {e}"))?
        .get()
        .map_err(|e| format!("BitmapDecoder await: {e}"))?;

        let bitmap = decoder
            .GetSoftwareBitmapAsync()
            .map_err(|e| format!("SoftwareBitmap: {e}"))?
            .get()
            .map_err(|e| format!("SoftwareBitmap await: {e}"))?;

        let engine = match lang.as_deref() {
            Some(l) if l != "auto" => {
                let language = Language::CreateLanguage(&HSTRING::from(l))
                    .map_err(|e| format!("Language: {e}"))?;
                OcrEngine::TryCreateFromLanguage(&language)
                    .map_err(|e| format!("OcrEngine: {e}"))?
            },
            _ => OcrEngine::TryCreateFromUserProfileLanguages()
                .map_err(|e| format!("OcrEngine: {e}"))?,
        };

        let result = engine
            .RecognizeAsync(&bitmap)
            .map_err(|e| format!("RecognizeAsync: {e}"))?
            .get()
            .map_err(|e| format!("RecognizeAsync await: {e}"))?;

        let lines_vec = result.Lines().map_err(|e| format!("Lines: {e}"))?;

        let count = lines_vec.Size().map_err(|e| format!("Lines.Size: {e}"))?;
        let mut line_results = Vec::with_capacity(count as usize);

        for i in 0..count {
            let line = lines_vec
                .GetAt(i)
                .map_err(|e| format!("Lines.GetAt({i}): {e}"))?;

            let line_text = line
                .Text()
                .map_err(|e| format!("Line.Text: {e}"))?
                .to_string_lossy();

            let words_vec = line.Words().map_err(|e| format!("Words: {e}"))?;
            let word_count = words_vec.Size().map_err(|e| format!("Words.Size: {e}"))?;

            let mut word_results = Vec::with_capacity(word_count as usize);
            let mut min_x = f64::MAX;
            let mut min_y = f64::MAX;
            let mut max_r = f64::MIN;
            let mut max_b = f64::MIN;

            for j in 0..word_count {
                let word = words_vec
                    .GetAt(j)
                    .map_err(|e| format!("Words.GetAt({j}): {e}"))?;
                let wtext = word
                    .Text()
                    .map_err(|e| format!("Word.Text: {e}"))?
                    .to_string_lossy();
                let wrect = word
                    .BoundingRect()
                    .map_err(|e| format!("Word.BoundingRect: {e}"))?;
                let wx = f64::from(wrect.X);
                let wy = f64::from(wrect.Y);
                let ww = f64::from(wrect.Width);
                let wh = f64::from(wrect.Height);

                // Track line bounding box (union of word rects)
                if wx < min_x {
                    min_x = wx;
                }
                if wy < min_y {
                    min_y = wy;
                }
                if wx + ww > max_r {
                    max_r = wx + ww;
                }
                if wy + wh > max_b {
                    max_b = wy + wh;
                }

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

        let full_text = crate::ocr_postprocess::join_text_regions(&line_results);

        if full_text.is_empty() {
            // Empty is a valid UI state (I4) — let FE show retry, do not throw.
            tracing::warn!("[WinRT OCR Detailed] OCR returned empty text");
            return Ok(OcrResultDetailed {
                text: String::new(),
                lines: vec![],
            });
        }

        tracing::info!(
            "[WinRT OCR Detailed] Success: {} lines, {} chars total",
            line_results.len(),
            full_text.len()
        );
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

/// Layout-aware OCR entry point.
///
/// When `layout_detection_enabled` is true in config AND the `layout-detection`
/// Cargo feature is compiled AND the DocLayout-YOLO model is available, this
/// runs layout detection first (filtering figure/table/formula regions), then
/// OCRs each text region separately and merges results with correct bounding
/// box offsets. Otherwise it delegates to the raw full-image OCR path
/// (`system_ocr_detailed` or `offline_ocr`).
///
/// `ocr_backend`: "winrt" (default) or "offline". When "offline", uses the
/// Rapid/Paddle sidecar; the specific backend is read from config.
///
/// This command exists as a **separate entry point** from `system_ocr_detailed`
/// / `offline_ocr` to avoid infinite recursion: the layout pipeline internally
/// calls `run_winrt_ocr_detailed_from_bytes` / `run_offline_ocr_detailed`
/// (the raw helpers), not these commands.
#[command]
pub async fn ocr_image_with_layout(
    app: tauri::AppHandle,
    base64_data: String,
    lang: Option<String>,
    ocr_backend: Option<String>,
) -> Result<OcrResultDetailed, String> {
    let raw = decode_base64_png(&base64_data)?;
    let backend = ocr_backend.as_deref().unwrap_or("winrt");
    crate::ocr_layout_pipeline::ocr_with_layout_detection(
        &app,
        &raw,
        lang.as_deref(),
        backend,
    )
    .await
}

// ── Youdao OCR ─────────────────────────────────────────────────────────────

fn extract_bounding_box(item: &serde_json::Value) -> (f64, f64, f64, f64) {
    // Try various bounding box formats
    if let Some(bounding) = item.get("bounding") {
        // Format: { "bounding": { "x": 0, "y": 0, "width": 100, "height": 20 } }
        let x = bounding.get("x").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
        let y = bounding.get("y").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
        let w = bounding
            .get("width")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let h = bounding
            .get("height")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        return (x, y, w, h);
    }
    if let Some(rect) = item.get("rect") {
        // Format: { "rect": { "left": 0, "top": 0, "right": 100, "bottom": 20 } }
        let left = rect.get("left").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
        let top = rect.get("top").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
        let right = rect.get("right").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
        let bottom = rect.get("bottom").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
        return (left, top, right - left, bottom - top);
    }
    // Try direct fields
    let x = item.get("x").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
    let y = item.get("y").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
    let w = item.get("width").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
    let h = item.get("height").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
    (x, y, w, h)
}

/// Run Youdao OCR using the reverse-engineered imgtranocr endpoint (free, no API key needed).
/// Uses the signing algorithm from `YodaoDict` analysis.
/// Returns per-line details with bounding boxes.
#[command]
pub async fn youdao_ocr(
    base64_data: String,
    lang: Option<String>,
    _app_key: Option<String>,
    _app_secret: Option<String>,
    _state: tauri::State<'_, crate::AppState>,
) -> Result<OcrResultDetailed, String> {
    let raw = decode_base64_png(&base64_data)?;

    // Compress image if too large
    let image_bytes = if raw.len() > 500 * 1024 {
        // > 500KB
        tracing::info!(
            "[Youdao OCR] Image too large ({}KB), compressing...",
            raw.len() / 1024
        );
        let img = screenshots::image::load_from_memory(&raw)
            .map_err(|e| format!("Failed to load image for compression: {e}"))?;

        let max_dim = 2000u32;
        let (w, h) = (img.width(), img.height());
        let scale = if w > h {
            f64::from(max_dim) / f64::from(w)
        } else {
            f64::from(max_dim) / f64::from(h)
        };

        let resized = if scale < 1.0 {
            let new_w = (f64::from(w) * scale) as u32;
            let new_h = (f64::from(h) * scale) as u32;
            img.resize(
                new_w,
                new_h,
                screenshots::image::imageops::FilterType::Lanczos3,
            )
        } else {
            img
        };

        let mut buf = std::io::Cursor::new(Vec::new());
        resized
            .write_to(&mut buf, screenshots::image::ImageFormat::Jpeg)
            .map_err(|e| format!("Failed to compress image: {e}"))?;
        let compressed = buf.into_inner();
        tracing::info!("[Youdao OCR] Compressed to {}KB", compressed.len() / 1024);
        compressed
    } else {
        raw
    };

    // Use reverse-engineered imgtranocr endpoint (free, no API key required)
    // Reference: E:\Code\ai\YodaoDict\youdao_translate.py
    let endpoint = "https://ocrtran.youdao.com/ocr/imgtranocr";

    // OCR key from reverse engineering (public key from desktop client)
    let ocr_key = "VPaHE3kX_vl4BhgYiu2n";

    let lang_from = match lang.as_deref() {
        Some("zh" | "zh-CN" | "zh-CHS") => "zh-CHS",
        Some("en") => "en",
        Some("ja") => "ja",
        Some("ko") => "ko",
        _ => "auto",
    };

    // Generate signature following the Python implementation:
    // raw = f"deskdict{b64str[:10]}{len(b64str)}{b64str[-10:]}{salt}{ocr_key}"
    // sig = hashlib.md5(raw.encode()).hexdigest()
    let salt = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
        .to_string();

    let image_base64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &image_bytes);

    let b64_len = image_base64.len();
    let b64_first10 = if b64_len >= 10 {
        &image_base64[..10]
    } else {
        &image_base64[..]
    };
    let b64_last10 = if b64_len >= 10 {
        &image_base64[b64_len - 10..]
    } else {
        &image_base64[..]
    };

    let sign_raw = format!(
        "deskdict{b64_first10}{b64_len}{b64_last10}{salt}{ocr_key}"
    );
    let sign = format!("{:x}", md5::compute(sign_raw.as_bytes()));

    tracing::info!("[Youdao OCR] Using free imgtranocr endpoint (no API key required)");
    tracing::info!(
        "[Youdao OCR] Image size: {}KB, lang: {}",
        image_bytes.len() / 1024,
        lang_from
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    // Build multipart form with image file
    let part = reqwest::multipart::Part::bytes(image_bytes)
        .file_name("img.png")
        .mime_str("image/png")
        .map_err(|e| format!("Failed to create multipart: {e}"))?;

    let form = reqwest::multipart::Form::new()
        .part("multipartFile", part)
        .text("clientele", "deskdict")
        .text("salt", salt)
        .text("sign", sign)
        .text("from", lang_from.to_string())
        .text("to", "zh-CHS")
        .text("isSaveHistory", "true")
        .text("isSyncSaveHistory", "true")
        .text("funDesc", "photo_translate");

    tracing::info!("[Youdao OCR] Sending request to {}", endpoint);

    let resp = match client.post(endpoint).multipart(form).send().await {
        Ok(r) => r,
        Err(e) => {
            let err_msg = format!("Request failed: {e}");
            tracing::error!("[Youdao OCR] {}", err_msg);
            return Err(err_msg);
        },
    };

    let status = resp.status();
    tracing::info!("[Youdao OCR] Response status: {}", status);

    let body = resp.text().await.map_err(|e| format!("Body read: {e}"))?;
    tracing::info!(
        "[Youdao OCR] Response body ({} bytes): {}",
        body.len(),
        &body[..body.len().min(500)]
    );

    // Parse response - try multiple formats
    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("JSON parse: {e}"))?;

    // Check for error codes
    if let Some(code) = json.get("errorCode").and_then(|v| v.as_str()) {
        if code != "0" && code != "true" {
            return Err(format!("Youdao OCR errorCode={code}"));
        }
    }

    if let Some(code) = json.get("code").and_then(serde_json::Value::as_i64) {
        if code != 0 && code != 200 {
            return Err(format!("Youdao OCR code={code}"));
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
        } else if let Some(obj) = result.as_object() {
            tracing::info!("[Youdao OCR] Parsing result object");
            if let Some(regions) = obj.get("regions").and_then(|v| v.as_array()) {
                for region in regions {
                    if let Some(text) = region.get("text").and_then(|v| v.as_str()) {
                        if !text.trim().is_empty() {
                            let (x, y, w, h) = extract_bounding_box(region);
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
        }
    }
    // Format 3: { "lines": [...] }
    else if let Some(lines_arr) = json.get("lines").and_then(|v| v.as_array()) {
        tracing::info!(
            "[Youdao OCR] Parsing lines array ({} items)",
            lines_arr.len()
        );
        for line in lines_arr {
            if let Some(text) = line.get("text").and_then(|v| v.as_str()) {
                if !text.trim().is_empty() {
                    let (x, y, w, h) = extract_bounding_box(line);
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
    // Format 4: { "text": "..." } flat text
    else if let Some(text) = json.get("text").and_then(|v| v.as_str()) {
        tracing::info!("[Youdao OCR] Parsing flat text ({} chars)", text.len());
        if !text.trim().is_empty() {
            lines.push(OcrLineResult {
                text: text.to_string(),
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
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

    if lines.is_empty() {
        tracing::warn!(
            "[Youdao OCR] No text extracted, response keys: {:?}",
            json.as_object().map(|m| m.keys().collect::<Vec<_>>())
        );
        // Empty is valid for FE I4 retry UI — do not throw.
        return Ok(OcrResultDetailed {
            text: String::new(),
            lines: vec![],
        });
    }

    tracing::info!(
        "[Youdao OCR] Success: {} lines, {} chars total",
        lines.len(),
        full_text.len()
    );
    Ok(OcrResultDetailed {
        text: full_text,
        lines,
    })
}

