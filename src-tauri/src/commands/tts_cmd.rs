use crate::tts;
use crate::AppState;
use base64::Engine;
use tauri::State;

#[tauri::command]
pub async fn text_to_speech(
    state: State<'_, AppState>,
    text: String,
    lang: String,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("Text is empty".to_string());
    }

    let config = state.system.config.lock().await;
    let token = config.edge_tts_token.clone();
    drop(config);

    let voice = tts::get_voice_for_lang(&lang);
    let audio_data = tts::synthesize_with_token(&text, voice, &token)
        .await
        .map_err(|e| format!("TTS failed: {}", e))?;

    // Return base64 encoded audio
    let base64_audio = base64::engine::general_purpose::STANDARD.encode(&audio_data);
    Ok(base64_audio)
}

#[tauri::command]
pub async fn get_tts_voices() -> Result<Vec<tts::TtsVoice>, String> {
    Ok(tts::default_voices())
}
