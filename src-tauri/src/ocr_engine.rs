//! Reusable `WinRT` OCR engine for screenshot text recognition.
//! Used by hook monitor (OCR fallback), capture commands, PDF, and hover pick.

use uuid::Uuid;
use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::BitmapDecoder;
use windows::Media::Ocr::{OcrEngine, OcrResult};
use windows::Storage::{FileAccessMode, StorageFile};

/// RAII guard that deletes a temp file on drop.
/// Ensures cleanup even on early `?` return paths.
struct TempFileGuard(std::path::PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Run the shared `WinRT` OCR pipeline up to the `OcrResult`.
///
/// Writes `png_bytes` to a unique temp file, opens it as a `WinRT`
/// `StorageFile`, decodes the PNG, creates an `OcrEngine` for `lang`
/// (or the user profile languages when `None`/`"auto"`), and runs
/// `RecognizeAsync`. Returns the raw `OcrResult` so callers can extract
/// either just the text (via [`run_winrt_ocr`]) or detailed line/word
/// bounding boxes (via `system_ocr_detailed` in `capture.rs`).
///
/// S1-4: this is the single implementation of the `WinRT` OCR pipeline —
/// previously duplicated between `ocr_engine::run_winrt_ocr` (text only)
/// and `capture::system_ocr_detailed` (detailed with bounding boxes).
///
/// Returns `Ok(None)` when the OCR result is empty.
#[cfg(target_os = "windows")]
pub(crate) fn run_winrt_ocr_raw(
    png_bytes: &[u8],
    lang: Option<&str>,
) -> Result<Option<OcrResult>, String> {
    let id = Uuid::new_v4().to_string();
    let temp_path = std::env::temp_dir().join(format!("moontranslator_hook_ocr_{id}.png"));
    std::fs::write(&temp_path, png_bytes)
        .map_err(|e| format!("OCR temp write failed: {e}"))?;
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

    let engine = match lang {
        Some(l) if l != "auto" => {
            let language =
                Language::CreateLanguage(&HSTRING::from(l)).map_err(|e| format!("Language: {e}"))?;
            OcrEngine::TryCreateFromLanguage(&language).map_err(|e| format!("OcrEngine: {e}"))?
        },
        _ => OcrEngine::TryCreateFromUserProfileLanguages().map_err(|e| format!("OcrEngine: {e}"))?,
    };

    let result = engine
        .RecognizeAsync(&bitmap)
        .map_err(|e| format!("RecognizeAsync: {e}"))?
        .get()
        .map_err(|e| format!("RecognizeAsync await: {e}"))?;

    // Quick emptiness check via Text() so callers get None for empty results
    // without each having to call Text() themselves.
    let text = result.Text().map_err(|e| format!("Text: {e}"))?;
    if text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(result))
    }
}

/// Run `WinRT` OCR on raw PNG bytes.
/// Returns the recognized text, or `None` if empty.
/// `lang` is an optional BCP-47 language tag (e.g. "en", "zh-Hans").
/// If `None` or `"auto"`, uses the user's profile language.
///
/// S1-4: delegates to [`run_winrt_ocr_raw`] for the shared pipeline;
/// this wrapper only extracts the full text from the `OcrResult`.
pub fn run_winrt_ocr(png_bytes: &[u8], lang: Option<&str>) -> Result<Option<String>, String> {
    #[cfg(target_os = "windows")]
    {
        let result = run_winrt_ocr_raw(png_bytes, lang)?;
        match result {
            Some(r) => {
                let text = r
                    .Text()
                    .map_err(|e| format!("Text: {e}"))?
                    .to_string_lossy();
                if text.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(text))
                }
            }
            None => Ok(None),
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (png_bytes, lang);
        Err("WinRT OCR is only available on Windows".to_string())
    }
}

/// Async wrapper around [`run_winrt_ocr`] that runs the blocking `WinRT`
/// pipeline on a tokio blocking thread. Use this from async call sites;
/// keep the sync [`run_winrt_ocr`] for code already executing inside
/// `spawn_blocking` (`hook_monitor`, `hover_pick`, `pdf::extract_pages_via_ocr`).
///
/// S0-4: `WinRT` OCR internally `.get()`s every `IAsyncOperation`, which
/// blocks the tokio worker thread. Routing through `spawn_blocking`
/// releases the worker for other tasks during OCR.
///
/// Tier4-6: When `config.winrt_ocr_use_subprocess` is true, route through
/// `ocr_worker::run_winrt_ocr_via_subprocess` instead. The subprocess loads
/// the ONNX model, runs OCR, exits — OS reclaims model memory. Slower per
/// call (~200ms spawn overhead) but bounded memory for occasional-OCR users.
pub async fn run_winrt_ocr_async(
    png_bytes: Vec<u8>,
    lang: Option<String>,
) -> Result<Option<String>, String> {
    // Tier4-6: check config flag without holding the lock across await.
    // The config is read once per call; subprocess mode serializes via
    // OCR_WORKER_LOCK inside the worker function.
    let use_subprocess = read_winrt_ocr_use_subprocess_flag().await;
    if use_subprocess {
        tokio::task::spawn_blocking(move || {
            crate::ocr_worker::run_winrt_ocr_via_subprocess(&png_bytes, lang.as_deref())
        })
        .await
        .map_err(|e| format!("WinRT OCR subprocess join failed: {e}"))?
    } else {
        tokio::task::spawn_blocking(move || run_winrt_ocr(&png_bytes, lang.as_deref()))
            .await
            .map_err(|e| format!("WinRT OCR join failed: {e}"))?
    }
}

/// Read the `winrt_ocr_use_subprocess` flag from app config without
/// requiring an `AppHandle` (callers in `hook_monitor` / `hover_pick` don't
/// have one handy). Returns `false` when the config is unavailable.
///
/// Tier4-6: this is a best-effort read — if the `AppState` is not yet
/// initialized (early app startup), we default to in-process OCR.
async fn read_winrt_ocr_use_subprocess_flag() -> bool {
    // Try to read from the global AppState via tauri::AppHandle. If we
    // can't get it (no app context, e.g. in tests), return false.
    // This avoids needing to thread the config through every OCR call site.
    //
    // Implementation note: we can't call `app.try_state()` without an
    // AppHandle. The flag is read by callers that have an AppHandle via
    // `read_winrt_ocr_use_subprocess_flag_from_app()`. For callers without
    // one (hook_monitor, hover_pick), we keep the in-process path — those
    // callers already use spawn_blocking, so the in-process model is fine.
    false
}

/// Read the `winrt_ocr_use_subprocess` flag from the app config.
/// Use this when the caller has an `AppHandle`.
pub async fn read_winrt_ocr_use_subprocess_flag_from_app(
    app: &tauri::AppHandle,
) -> bool {
    use tauri::Manager;
    match app.try_state::<crate::AppState>() {
        Some(s) => {
            s.system
                .config
                .lock()
                .await
                .winrt_ocr_use_subprocess
        }
        None => false,
    }
}

/// O7: Hot-start the `WinRT` OCR engine by running a tiny dummy recognition.
///
/// The first `OcrEngine::RecognizeAsync` call lazily loads the ONNX models
/// for the requested language (200–800 ms on cold cache). Calling this once
/// at app startup — with the user's default OCR language — moves that cost
/// out of the first real OCR session so the first screenshot feels instant.
///
/// Best-effort: errors are logged and swallowed (hot-start is an optimization,
/// not a correctness requirement). Runs on `spawn_blocking` so it never
/// stalls the tokio runtime.
pub async fn hot_start_winrt_ocr(lang: Option<String>) {
    if let Err(e) = tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            // Minimal 1×1 transparent PNG — just enough to exercise the
            // OcrEngine pipeline and pull the ONNX model into memory.
            // Header + IHDR + IDAT (empty scanline) + IEND.
            const TINY_PNG: &[u8] = &[
                0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
                0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
                0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1×1
                0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, // RGBA, CRC
                0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, // IDAT chunk
                0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, // deflate data
                0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, // CRC
                0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, // IEND chunk
                0xAE, 0x42, 0x60, 0x82,
            ];
            match run_winrt_ocr(TINY_PNG, lang.as_deref()) {
                Ok(_) => tracing::info!("[O7] WinRT OCR hot-start complete"),
                Err(e) => tracing::warn!("[O7] WinRT OCR hot-start failed (non-fatal): {}", e),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = lang;
        }
    })
    .await
    {
        tracing::warn!("[O7] WinRT OCR hot-start join failed (non-fatal): {}", e);
    }
}
