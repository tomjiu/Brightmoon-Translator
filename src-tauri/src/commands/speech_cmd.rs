use crate::speech;
use crate::AppState;
use tauri::State;

/// Start speech recognition with the specified language
#[tauri::command]
pub async fn start_speech_recognition(
    lang: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let speech_state = state.speech_state.clone();

    speech::start_recognition(speech_state, &lang)
        .await
        .map_err(|e| format!("Failed to start speech recognition: {}", e))?;

    Ok(())
}

/// Stop speech recognition
#[tauri::command]
pub async fn stop_speech_recognition(state: State<'_, AppState>) -> Result<(), String> {
    let speech_state = state.speech_state.clone();

    speech::stop_recognition(speech_state)
        .await
        .map_err(|e| format!("Failed to stop speech recognition: {}", e))?;

    Ok(())
}

/// Get speech recognition status
#[tauri::command]
pub async fn get_speech_recognition_status(
    state: State<'_, AppState>,
) -> Result<speech::SpeechRecognitionStatus, String> {
    let speech_state = state.speech_state.clone();
    let status = speech::get_status(speech_state).await;
    Ok(status)
}

/// Get available speech recognition languages
#[tauri::command]
pub async fn get_speech_languages() -> Result<Vec<String>, String> {
    Ok(speech::get_available_languages())
}
