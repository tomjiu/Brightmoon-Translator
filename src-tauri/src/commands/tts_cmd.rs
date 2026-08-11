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
    let fish = config.fish_tts.clone();
    drop(config);

    let voice_arg = voice
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(std::string::ToString::to_string);
    let preferred_trim = preferred.trim().to_string();

    let resolved_voice = voice_arg
        .clone()
        .or_else(|| {
            if preferred_trim.is_empty() {
                None
            } else {
                Some(preferred_trim.clone())
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
        "fish" | "fish_audio" | "fishaudio" => {
            // reference_id: call voice > tts_voice (if not Edge Neural) > fish_tts.reference_id
            let rid = voice_arg
                .or_else(|| {
                    if !preferred_trim.is_empty() && !preferred_trim.contains("Neural") {
                        Some(preferred_trim)
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| fish.reference_id.clone());
            tts::synthesize_fish(
                &text,
                &fish.api_key,
                &fish.model,
                &rid,
                &fish.format,
                fish.speed,
            )
            .await
            .map_err(|e| format!("Fish Audio TTS failed: {e}"))?
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
pub async fn get_tts_voices(provider: Option<String>) -> Result<Vec<tts::TtsVoice>, String> {
    let p = provider.unwrap_or_default().trim().to_ascii_lowercase();
    Ok(match p.as_str() {
        "fish" | "fish_audio" | "fishaudio" => tts::default_fish_voices(),
        _ => tts::default_voices(),
    })
}
