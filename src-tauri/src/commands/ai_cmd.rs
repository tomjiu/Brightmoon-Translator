use crate::ai_enhanced::{AiTermEntry, MultiRoundResult, PolishStyle, TranslationStyle};
use crate::security;
use crate::AppState;
use serde::Deserialize;
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolishRequest {
    pub source_text: String,
    pub translated_text: String,
    pub from_lang: String,
    pub to_lang: String,
    pub style: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractTermsRequest {
    pub texts: Vec<(String, String)>,
    pub from_lang: String,
    pub to_lang: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnStyleRequest {
    pub history: Vec<(String, String)>,
    pub from_lang: String,
    pub to_lang: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextTranslateRequest {
    pub text: String,
    pub from_lang: String,
    pub to_lang: String,
    pub context: Vec<(String, String)>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiRoundRequest {
    pub text: String,
    pub from_lang: String,
    pub to_lang: String,
    pub rounds: u32,
}

/// Polish translation with AI enhancement
#[tauri::command]
pub async fn ai_polish_translation(
    state: State<'_, AppState>,
    request: PolishRequest,
) -> Result<String, String> {
    security::validate_text_length(&request.source_text, security::MAX_TRANSLATION_TEXT_LENGTH)?;
    security::validate_text_length(
        &request.translated_text,
        security::MAX_TRANSLATION_TEXT_LENGTH,
    )?;
    security::validate_language_code(&request.from_lang)?;
    security::validate_language_code(&request.to_lang)?;

    let style = match request.style.as_str() {
        "formal" => PolishStyle::Formal,
        "casual" => PolishStyle::Casual,
        "technical" => PolishStyle::Technical,
        "literary" => PolishStyle::Literary,
        _ => PolishStyle::Natural,
    };

    // Create AI service from config
    let config = state.system.config.lock().await;
    let llm_config = &config.llm;
    let llm_engine = crate::engine::llm::LlmEngine::with_multiple_keys(
        llm_config.all_keys(),
        &llm_config.base_url,
        &llm_config.model,
    );
    drop(config);

    let ai_service = crate::ai_enhanced::AiEnhancedService::new(llm_engine);
    ai_service
        .polish_translation(
            &request.source_text,
            &request.translated_text,
            &request.from_lang,
            &request.to_lang,
            &style,
        )
        .await
}

/// Extract terms from translation pairs
#[tauri::command]
pub async fn ai_extract_terms(
    state: State<'_, AppState>,
    request: ExtractTermsRequest,
) -> Result<Vec<AiTermEntry>, String> {
    security::validate_language_code(&request.from_lang)?;
    security::validate_language_code(&request.to_lang)?;

    if request.texts.is_empty() {
        return Ok(vec![]);
    }

    // Validate each text
    for (source, target) in &request.texts {
        security::validate_text_length(source, security::MAX_TRANSLATION_TEXT_LENGTH)?;
        security::validate_text_length(target, security::MAX_TRANSLATION_TEXT_LENGTH)?;
    }

    let config = state.system.config.lock().await;
    let llm_config = &config.llm;
    let llm_engine = crate::engine::llm::LlmEngine::with_multiple_keys(
        llm_config.all_keys(),
        &llm_config.base_url,
        &llm_config.model,
    );
    drop(config);

    let ai_service = crate::ai_enhanced::AiEnhancedService::new(llm_engine);
    ai_service
        .extract_terms(&request.texts, &request.from_lang, &request.to_lang)
        .await
}

/// Learn translation style from history
#[tauri::command]
pub async fn ai_learn_style(
    state: State<'_, AppState>,
    request: LearnStyleRequest,
) -> Result<TranslationStyle, String> {
    security::validate_language_code(&request.from_lang)?;
    security::validate_language_code(&request.to_lang)?;

    if request.history.len() < 3 {
        return Err("Need at least 3 translation pairs to learn style".to_string());
    }

    // Validate each text
    for (source, target) in &request.history {
        security::validate_text_length(source, security::MAX_TRANSLATION_TEXT_LENGTH)?;
        security::validate_text_length(target, security::MAX_TRANSLATION_TEXT_LENGTH)?;
    }

    let config = state.system.config.lock().await;
    let llm_config = &config.llm;
    let llm_engine = crate::engine::llm::LlmEngine::with_multiple_keys(
        llm_config.all_keys(),
        &llm_config.base_url,
        &llm_config.model,
    );
    drop(config);

    let ai_service = crate::ai_enhanced::AiEnhancedService::new(llm_engine);
    ai_service
        .learn_style(&request.history, &request.from_lang, &request.to_lang)
        .await
}

/// Context-aware translation
#[tauri::command]
pub async fn ai_context_translate(
    state: State<'_, AppState>,
    request: ContextTranslateRequest,
) -> Result<String, String> {
    security::validate_text_length(&request.text, security::MAX_TRANSLATION_TEXT_LENGTH)?;
    security::validate_language_code(&request.from_lang)?;
    security::validate_language_code(&request.to_lang)?;

    // Validate context texts
    for (source, target) in &request.context {
        security::validate_text_length(source, security::MAX_TRANSLATION_TEXT_LENGTH)?;
        security::validate_text_length(target, security::MAX_TRANSLATION_TEXT_LENGTH)?;
    }

    let config = state.system.config.lock().await;
    let llm_config = &config.llm;
    let llm_engine = crate::engine::llm::LlmEngine::with_multiple_keys(
        llm_config.all_keys(),
        &llm_config.base_url,
        &llm_config.model,
    );
    drop(config);

    let ai_service = crate::ai_enhanced::AiEnhancedService::new(llm_engine);
    ai_service
        .translate_with_context(
            &request.text,
            &request.from_lang,
            &request.to_lang,
            &request.context,
        )
        .await
}

/// Multi-round translation optimization
#[tauri::command]
pub async fn ai_multi_round_translate(
    state: State<'_, AppState>,
    request: MultiRoundRequest,
) -> Result<MultiRoundResult, String> {
    security::validate_text_length(&request.text, security::MAX_TRANSLATION_TEXT_LENGTH)?;
    security::validate_language_code(&request.from_lang)?;
    security::validate_language_code(&request.to_lang)?;

    let config = state.system.config.lock().await;
    let llm_config = &config.llm;
    let llm_engine = crate::engine::llm::LlmEngine::with_multiple_keys(
        llm_config.all_keys(),
        &llm_config.base_url,
        &llm_config.model,
    );
    drop(config);

    let ai_service = crate::ai_enhanced::AiEnhancedService::new(llm_engine);
    ai_service
        .multi_round_translate(
            &request.text,
            &request.from_lang,
            &request.to_lang,
            request.rounds,
        )
        .await
}
