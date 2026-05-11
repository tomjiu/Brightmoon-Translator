//! Reusable WinRT OCR engine for screenshot text recognition.
//! Used by hook monitor (OCR fallback) and capture commands.

use std::io::Cursor;
use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::BitmapDecoder;
use windows::Media::Ocr::OcrEngine;
use windows::Storage::{FileAccessMode, StorageFile};

/// Run WinRT OCR on raw PNG bytes.
/// Returns the recognized text, or None if empty.
/// `lang` is an optional BCP-47 language tag (e.g. "en", "zh-Hans").
/// If None or "auto", uses the user's profile language.
pub fn run_winrt_ocr(png_bytes: &[u8], lang: Option<&str>) -> Result<Option<String>, String> {
    // Write to temp file (WinRT BitmapDecoder needs a StorageFile path)
    let temp_path = std::env::temp_dir().join("moontranslator_hook_ocr.png");
    std::fs::write(&temp_path, png_bytes)
        .map_err(|e| format!("OCR temp write failed: {}", e))?;

    let path_str = temp_path.to_string_lossy().replace("\\\\?\\", "");

    let result = (|| -> Result<Option<String>, String> {
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

        let text = result
            .Text()
            .map_err(|e| format!("Text: {}", e))?
            .to_string_lossy();

        if text.is_empty() {
            Ok(None)
        } else {
            Ok(Some(text))
        }
    })();

    let _ = std::fs::remove_file(&temp_path);
    result
}

/// Capture a screen area and run OCR on it.
/// Returns the recognized text, or None if empty.
pub fn capture_and_ocr(
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    lang: Option<&str>,
) -> Result<Option<String>, String> {
    use screenshots::image::ImageFormat;

    let img = crate::commands::capture::capture_area_gdi(left, top, width, height)?;

    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("PNG encode: {}", e))?;

    run_winrt_ocr(&buf.into_inner(), lang)
}
