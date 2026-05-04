use async_trait::async_trait;

use crate::models::error::TranslationError;

/// Result from an input replacement operation
#[derive(Debug, Clone)]
pub struct ReplacementResult {
    /// Original text that was replaced
    pub original: String,
    /// Translated text that was inserted
    pub replacement: String,
    /// Whether the replacement was successful
    pub success: bool,
    /// Error message if replacement failed
    pub error: Option<String>,
}

/// Input Replacement capability.
/// Composes: get selection -> translate -> replace text in target app.
///
/// This is the interface for "select text, translate, and replace in-place".
/// Used for inline translation in editors, terminals, etc.
#[async_trait]
pub trait InputReplacement: Send + Sync {
    /// Get the currently selected text in the target application
    async fn get_selected_text(&self) -> Result<String, TranslationError>;

    /// Translate the selected text and replace it in-place
    async fn replace_translate(
        &self,
        from: &str,
        to: &str,
    ) -> Result<ReplacementResult, TranslationError>;

    /// Replace specific text in the target application
    async fn replace_text(&self, text: &str) -> Result<bool, TranslationError>;

    /// Translate and replace specific text
    async fn translate_and_replace(
        &self,
        text: &str,
        from: &str,
        to: &str,
    ) -> Result<ReplacementResult, TranslationError>;
}
