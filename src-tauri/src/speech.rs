use serde::{Deserialize, Serialize};
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
pub struct SpeechState {
    pub is_listening: bool,
    pub language: String,
}

impl SpeechState {
    pub fn new() -> Self {
        Self {
            is_listening: false,
            language: "en-US".to_string(),
        }
    }
}

/// Start continuous speech recognition
/// This uses the Web Speech API on the frontend, with this backend providing
/// configuration and status management.
pub async fn start_recognition(state: Arc<Mutex<SpeechState>>, lang: &str) -> anyhow::Result<()> {
    let mut state_guard = state.lock().await;

    // Update state
    state_guard.language = lang.to_string();
    state_guard.is_listening = true;

    tracing::info!("Speech recognition started for language: {}", lang);

    Ok(())
}

/// Stop continuous speech recognition
pub async fn stop_recognition(state: Arc<Mutex<SpeechState>>) -> anyhow::Result<()> {
    let mut state_guard = state.lock().await;

    state_guard.is_listening = false;

    tracing::info!("Speech recognition stopped");

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
