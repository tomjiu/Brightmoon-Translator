use crate::tts;
use crate::AppState;
use base64::Engine;
use tauri::State;

#[tauri::command]
pub async fn text_to_speech(
    state: State<'_, AppState>,
    text: String,
    lang: String,
    voice: Option<String>,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("Text is empty".to_string());
    }

    let config = state.system.config.lock().await;
    let provider = config.tts_provider.clone();
    let edge_token = config.edge_tts_token.clone();
    let preferred = config.tts_voice.clone();
    let openai = config.openai_tts.clone();
    drop(config);

    let resolved_voice = voice
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            if preferred.trim().is_empty() {
                None
            } else {
                Some(preferred)
            }
        })
        .unwrap_or_else(|| tts::get_voice_for_lang(&lang).to_string());

    let provider_norm = provider.trim().to_ascii_lowercase();
    let audio_data = match provider_norm.as_str() {
        "openai" => {
            let v = if resolved_voice.is_empty() || resolved_voice.contains("Neural") {
                openai.voice.clone()
            } else {
                resolved_voice
            };
            tts::synthesize_openai(
                &text,
                &openai.api_key,
                &openai.base_url,
                &openai.model,
                &v,
                openai.speed,
            )
            .await
            .map_err(|e| format!("OpenAI TTS failed: {e}"))?
        },
        "youdao" => tts::synthesize_youdao_dictvoice(&text, &lang)
            .await
            .map_err(|e| format!("Youdao TTS failed: {e}"))?,
        _ => tts::synthesize_with_token(&text, &resolved_voice, &edge_token)
            .await
            .map_err(|e| format!("TTS failed: {e}"))?,
    };

    let base64_audio = base64::engine::general_purpose::STANDARD.encode(&audio_data);
    Ok(base64_audio)
}

#[tauri::command]
pub async fn get_tts_voices() -> Result<Vec<tts::TtsVoice>, String> {
    Ok(tts::default_voices())
}
