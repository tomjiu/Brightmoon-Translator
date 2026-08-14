use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechRecognitionResult {
    pub text: String,
    pub confidence: f64,
    pub is_final: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechRecognitionStatus {
    pub is_listening: bool,
    pub language: String,
    pub error: Option<String>,
}

/// Language code to Windows Speech Recognition locale mapping
pub fn lang_to_locale(lang: &str) -> &str {
    match lang {
        "zh" => "zh-CN",
        "en" => "en-US",
        "ja" => "ja-JP",
        "ko" => "ko-KR",
        "fr" => "fr-FR",
        "de" => "de-DE",
        "es" => "es-ES",
        "ru" => "ru-RU",
        "pt" => "pt-BR",
        "it" => "it-IT",
        "ar" => "ar-SA",
        "th" => "th-TH",
        "vi" => "vi-VN",
        _ => "en-US",
    }
}

/// Get available speech recognition languages
pub fn get_available_languages() -> Vec<String> {
    vec![
        "zh-CN".to_string(),
        "en-US".to_string(),
        "ja-JP".to_string(),
        "ko-KR".to_string(),
        "fr-FR".to_string(),
        "de-DE".to_string(),
        "es-ES".to_string(),
        "ru-RU".to_string(),
        "pt-BR".to_string(),
        "it-IT".to_string(),
        "ar-SA".to_string(),
        "th-TH".to_string(),
        "vi-VN".to_string(),
    ]
}

/// Speech recognition state managed by the application
///
/// `is_listening` / `language` are the application-level contract kept in sync
/// with the frontend; recognition work runs on a dedicated background thread
/// that holds the `WinRT` `SpeechRecognizer` and pushes results via Tauri events.
pub struct SpeechState {
    pub is_listening: bool,
    pub language: String,
    /// Cooperative stop signal shared with the background recognition thread.
    pub stop_flag: Arc<AtomicBool>,
}

impl Default for SpeechState {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeechState {
    pub fn new() -> Self {
        Self {
            is_listening: false,
            language: "en-US".to_string(),
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

// ---------------------------------------------------------------------------
// Windows native recognition worker
//
// The WinRT OCR path in `capture.rs` proves our command thread model can drive
// WinRT async APIs via `.get()` (blocking wait). `SpeechRecognizer` is `Send` +
// `Sync`, so we keep one instance alive on a dedicated thread and run the
// single-shot loop: RecognizeAsync -> emit result -> repeat until stopped.
// ---------------------------------------------------------------------------
const EVENT_RESULT: &str = "speech-recognition-result";
const EVENT_START: &str = "speech-recognition-start";
const EVENT_STOP: &str = "speech-recognition-stop";
const EVENT_ERROR: &str = "speech-recognition-error";

use tauri::Emitter;

/// Start single-shot continuous recognition on a background thread.
pub async fn start_recognition(
    app: tauri::AppHandle,
    state: Arc<Mutex<SpeechState>>,
    lang: &str,
) -> anyhow::Result<()> {
    let locale = lang_to_locale(lang).to_string();

    let mut guard = state.lock().await;
    if guard.is_listening {
        return Ok(());
    }
    guard.language.clone_from(&locale);
    guard.is_listening = true;
    guard.stop_flag.store(false, Ordering::SeqCst);
    let stop_flag = guard.stop_flag.clone();
    drop(guard);

    let state_for_emit = state.clone();
    let _ = app.emit(EVENT_START, serde_json::json!({ "language": locale }));

    std::thread::Builder::new()
        .name("moontranslator-speech".to_string())
        .spawn(move || {
            if let Err(e) = run_recognition_loop(&locale, &stop_flag, &app) {
                tracing::warn!("[speech] recognition loop ended with error: {e}");
                let _ = app.emit(EVENT_ERROR, serde_json::json!({ "error": e.to_string() }));
            }
            let mut guard = state_for_emit.blocking_lock();
            guard.is_listening = false;
            let _ = app.emit(EVENT_STOP, ());
        })
        .map_err(|e| anyhow::anyhow!("failed to spawn speech thread: {e}"))?;

    Ok(())
}

/// Stop the background recognition thread.
pub async fn stop_recognition(state: Arc<Mutex<SpeechState>>) -> anyhow::Result<()> {
    let mut guard = state.lock().await;
    guard.is_listening = false;
    guard.stop_flag.store(true, Ordering::SeqCst);
    Ok(())
}

/// Get current speech recognition status
pub async fn get_status(state: Arc<Mutex<SpeechState>>) -> SpeechRecognitionStatus {
    let state_guard = state.lock().await;
    SpeechRecognitionStatus {
        is_listening: state_guard.is_listening,
        language: state_guard.language.clone(),
        error: None,
    }
}

#[cfg(target_os = "windows")]
fn run_recognition_loop(
    locale: &str,
    stop_flag: &AtomicBool,
    app: &tauri::AppHandle,
) -> anyhow::Result<()> {
    use windows::core::HSTRING;
    use windows::Globalization::Language;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

    // SpeechRecognizer must run on a COM-initialized thread (MTA is fine for
    // the non-UI path). S_FALSE / RPC_E_CHANGED_MODE mean the thread already
    // has a compatible apartment — not errors.
    use windows::core::HRESULT;
    const S_FALSE: HRESULT = HRESULT(0x0000_0001);
    const RPC_E_CHANGED_MODE: HRESULT = HRESULT(-2_147_417_850);
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    let com_initialized = hr.is_ok() || hr == S_FALSE || hr == RPC_E_CHANGED_MODE;
    if !com_initialized {
        return Err(anyhow::anyhow!("CoInitializeEx failed: {hr}"));
    }
    let com_ours = hr.is_ok();

    let worker_result = (|| -> anyhow::Result<()> {
        let language = Language::CreateLanguage(&HSTRING::from(locale))
            .map_err(|e| anyhow::anyhow!("create language {locale}: {e}"))?;
        let recognizer = create_recognizer(&language)?;
        recognize_with_timeouts(&recognizer, stop_flag, app)?;
        let _ = recognizer.Close();
        Ok(())
    })();

    if com_ours {
        // SAFETY: CoUninitialize only for the apartment we just initialized.
        unsafe { CoUninitialize() };
    }
    worker_result
}

#[cfg(not(target_os = "windows"))]
fn run_recognition_loop(
    _locale: &str,
    _stop_flag: &AtomicBool,
    _app: &tauri::AppHandle,
) -> anyhow::Result<()> {
    Err(anyhow::anyhow!(
        "native speech recognition is only supported on Windows"
    ))
}

#[cfg(target_os = "windows")]
fn create_recognizer(
    language: &windows::Globalization::Language,
) -> anyhow::Result<windows::Media::SpeechRecognition::SpeechRecognizer> {
    use windows::Media::SpeechRecognition::{SpeechRecognizer, SpeechRecognitionResultStatus};

    // Factory Create(language) validates the tag against installed speech packs.
    let recognizer = SpeechRecognizer::Create(language)
        .map_err(|e| anyhow::anyhow!("SpeechRecognizer::Create failed: {e}"))?;

    // No explicit constraints => free dictation grammar is used.
    let compiled = recognizer
        .CompileConstraintsAsync()
        .map_err(|e| anyhow::anyhow!("CompileConstraintsAsync failed: {e}"))?
        .get()
        .map_err(|e| anyhow::anyhow!("CompileConstraintsAsync await failed: {e}"))?;
    let status = compiled
        .Status()
        .map_err(|e| anyhow::anyhow!("compilation status failed: {e}"))?;

    // Success = 0; other values (e.g. unavailable grammar) are logged, the
    // loop will surface per-utterance statuses and the FE can retry.
    if status != SpeechRecognitionResultStatus(0) {
        tracing::warn!("[speech] constraint compilation status: {status:?}");
    }
    Ok(recognizer)
}

#[cfg(target_os = "windows")]
fn recognize_with_timeouts(
    recognizer: &windows::Media::SpeechRecognition::SpeechRecognizer,
    stop_flag: &AtomicBool,
    app: &tauri::AppHandle,
) -> anyhow::Result<()> {
    use windows::Foundation::TimeSpan;
    use windows::Media::SpeechRecognition::SpeechRecognitionResultStatus;

    // ~1.0s initial silence, 1.5s trailing end silence -> stop latency stays low.
    if let Ok(timeouts) = recognizer.Timeouts() {
        let _ = timeouts.SetInitialSilenceTimeout(TimeSpan {
            Duration: 10_000_000,
        });
        let _ = timeouts.SetEndSilenceTimeout(TimeSpan {
            Duration: 15_000_000,
        });
    }

    let mut last_transcript: Option<String> = None;
    loop {
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        let op = recognizer
            .RecognizeAsync()
            .map_err(|e| anyhow::anyhow!("RecognizeAsync failed: {e}"))?;
        let recognized = match op.get() {
            Ok(r) => r,
            Err(e) if e.code() == windows::core::HRESULT(-2_147_199_736) => {
                // User was silent within the timeout — keep listening.
                continue;
            }
            Err(e) => return Err(anyhow::anyhow!("RecognizeAsync await failed: {e}")),
        };

        let text = recognized.Text().map(|t| t.to_string()).unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }
        let confidence = recognized.RawConfidence().unwrap_or(0.0);
        let status = recognized
            .Status()
            .unwrap_or(SpeechRecognitionResultStatus(0));

        // Only emit meaningful results; non-zero statuses are treated as a
        // transient no-match and we simply keep listening.
        if status != SpeechRecognitionResultStatus(0) {
            tracing::debug!("[speech] result status code {}", status.0);
            continue;
        }

        let payload = serde_json::json!({
            "text": text,
            "confidence": confidence,
            "isFinal": true,
        });
        let _ = app.emit(EVENT_RESULT, payload);

        if stop_flag.load(Ordering::SeqCst) {
            break;
        }
        last_transcript = Some(text);
    }

    if let Some(t) = last_transcript {
        tracing::info!("[speech] last transcript: {t}");
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn recognize_with_timeouts(
    _recognizer: &(),
    _stop_flag: &AtomicBool,
    _app: &tauri::AppHandle,
) -> anyhow::Result<()> {
    Ok(())
}