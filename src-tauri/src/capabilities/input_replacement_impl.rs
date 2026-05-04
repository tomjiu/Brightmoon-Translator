use async_trait::async_trait;
use std::sync::Arc;

use super::input_replacement::{InputReplacement, ReplacementResult};
use crate::models::error::TranslationError;
use crate::selection::SelectionProviderManager;
use crate::services::TranslationService;

/// Default desktop implementation of InputReplacement.
/// Composes: SelectionProviderManager -> TranslationService -> clipboard replace.
pub struct DefaultInputReplacement {
    selection_manager: Arc<SelectionProviderManager>,
    translation_service: Arc<TranslationService>,
}

impl DefaultInputReplacement {
    pub fn new(
        selection_manager: Arc<SelectionProviderManager>,
        translation_service: Arc<TranslationService>,
    ) -> Self {
        Self {
            selection_manager,
            translation_service,
        }
    }
}

#[async_trait]
impl InputReplacement for DefaultInputReplacement {
    async fn get_selected_text(&self) -> Result<String, TranslationError> {
        let selection = self
            .selection_manager
            .get_selection()
            .await
            .ok_or(TranslationError::InvalidInput(
                "No text selected".to_string(),
            ))?;

        if selection.text.trim().is_empty() {
            return Err(TranslationError::InvalidInput(
                "Selected text is empty".to_string(),
            ));
        }

        Ok(selection.text)
    }

    async fn replace_translate(
        &self,
        from: &str,
        to: &str,
    ) -> Result<ReplacementResult, TranslationError> {
        // Get selected text
        let original = self.get_selected_text().await?;

        // Translate
        let translated = self
            .translation_service
            .translate_primary(&original, from, to)
            .await?;

        // Replace in target app via clipboard
        let result = tokio::task::spawn_blocking({
            let text = translated.clone();
            move || super::platform::replace_text_via_clipboard(&text)
        })
        .await
        .map_err(|e| TranslationError::Internal(format!("Task join error: {}", e)))?;

        match result {
            Ok(()) => Ok(ReplacementResult {
                original,
                replacement: translated,
                success: true,
                error: None,
            }),
            Err(e) => Ok(ReplacementResult {
                original,
                replacement: translated,
                success: false,
                error: Some(e),
            }),
        }
    }

    async fn replace_text(&self, text: &str) -> Result<bool, TranslationError> {
        let result = tokio::task::spawn_blocking({
            let text = text.to_string();
            move || super::platform::replace_text_via_clipboard(&text)
        })
        .await
        .map_err(|e| TranslationError::Internal(format!("Task join error: {}", e)))?;

        match result {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn translate_and_replace(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<ReplacementResult, TranslationError> {
        if text.trim().is_empty() {
            return Err(TranslationError::InvalidInput("Text is empty".to_string()));
        }

        // Translate
        let translated = self
            .translation_service
            .translate_primary(text, from, to)
            .await?;

        // Replace in target app via clipboard
        let result = tokio::task::spawn_blocking({
            let text = translated.clone();
            move || super::platform::replace_text_via_clipboard(&text)
        })
        .await
        .map_err(|e| TranslationError::Internal(format!("Task join error: {}", e)))?;

        match result {
            Ok(()) => Ok(ReplacementResult {
                original: text.to_string(),
                replacement: translated,
                success: true,
                error: None,
            }),
            Err(e) => Ok(ReplacementResult {
                original: text.to_string(),
                replacement: translated,
                success: false,
                error: Some(e),
            }),
        }
    }
}
