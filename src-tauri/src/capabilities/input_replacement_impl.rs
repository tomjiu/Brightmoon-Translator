use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::input_replacement::{InputReplacement, ReplacementResult};
use crate::config::AppConfig;
use crate::models::error::TranslationError;
use crate::selection::SelectionProviderManager;
use crate::services::TranslationService;

/// Default desktop implementation of InputReplacement.
/// Composes: SelectionProviderManager -> TranslationService -> clipboard/type replace.
pub struct DefaultInputReplacement {
    selection_manager: Arc<SelectionProviderManager>,
    translation_service: Arc<TranslationService>,
    in_flight: AtomicBool,
    /// Shared cancel flag (also passed into type-SendInput loop).
    cancel: Arc<AtomicBool>,
    /// Config for dynamic exclude_processes (same pattern as DefaultSelectionTranslation).
    config: Arc<Mutex<AppConfig>>,
}

impl DefaultInputReplacement {
    pub fn new(
        selection_manager: Arc<SelectionProviderManager>,
        translation_service: Arc<TranslationService>,
        config: Arc<Mutex<AppConfig>>,
    ) -> Self {
        Self {
            selection_manager,
            translation_service,
            in_flight: AtomicBool::new(false),
            cancel: Arc::new(AtomicBool::new(false)),
            config,
        }
    }

    /// STranslate-style: if already running, request cancel and do not start a new job.
    fn try_begin(&self) -> bool {
        if self.in_flight.swap(true, Ordering::AcqRel) {
            self.cancel.store(true, Ordering::Release);
            tracing::info!("[replace_translate] already running — cancel requested, skip new");
            return false;
        }
        self.cancel.store(false, Ordering::Release);
        true
    }

    fn end(&self) {
        self.in_flight.store(false, Ordering::Release);
        self.cancel.store(false, Ordering::Release);
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Acquire)
    }
}

#[async_trait]
impl InputReplacement for DefaultInputReplacement {
    async fn get_selected_text(&self) -> Result<String, TranslationError> {
        let exclude = {
            let c = self.config.lock().await;
            c.selection_ux.exclude_processes.clone()
        };
        let selection =
            self.selection_manager
                .get_selection_routed(&exclude)
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
        use_clipboard_output: bool,
    ) -> Result<ReplacementResult, TranslationError> {
        if !self.try_begin() {
            return Ok(ReplacementResult {
                original: String::new(),
                replacement: String::new(),
                success: false,
                error: Some("cancelled".to_string()),
                fallback_to_overlay: false,
            });
        }

        let outcome = async {
            let original = self.get_selected_text().await?;
            if self.is_cancelled() {
                return Ok(ReplacementResult {
                    original,
                    replacement: String::new(),
                    success: false,
                    error: Some("cancelled".to_string()),
                    fallback_to_overlay: false,
                });
            }
            tracing::info!(
                "[replace_translate] Selected text: {} chars",
                original.len()
            );

            let translated = self
                .translation_service
                .run_primary(
                    crate::models::translation::TranslateChannel::Replace,
                    &original,
                    from,
                    to,
                )
                .await?;
            if self.is_cancelled() {
                return Ok(ReplacementResult {
                    original,
                    replacement: translated,
                    success: false,
                    error: Some("cancelled".to_string()),
                    fallback_to_overlay: true,
                });
            }
            tracing::info!(
                "[replace_translate] Translated: '{}' -> '{}' (clipboard={})",
                original.chars().take(50).collect::<String>(),
                translated.chars().take(50).collect::<String>(),
                use_clipboard_output
            );

            let cancel = self.cancel.clone();
            let text = translated.clone();
            let result = tokio::task::spawn_blocking(move || {
                super::platform::deliver_replacement_text(
                    &text,
                    use_clipboard_output,
                    Some(&cancel),
                )
            })
            .await
            .map_err(|e| TranslationError::Internal(format!("Task join error: {}", e)))?;

            match result {
                Ok(()) => Ok(ReplacementResult {
                    original,
                    replacement: translated,
                    success: true,
                    error: None,
                    fallback_to_overlay: false,
                }),
                Err(e) if e == "cancelled" => Ok(ReplacementResult {
                    original,
                    replacement: translated,
                    success: false,
                    error: Some("cancelled".to_string()),
                    fallback_to_overlay: true,
                }),
                Err(e) => Ok(ReplacementResult {
                    original,
                    replacement: translated,
                    success: false,
                    error: Some(e),
                    fallback_to_overlay: true,
                }),
            }
        }
        .await;

        self.end();
        outcome
    }

    async fn replace_text(
        &self,
        text: &str,
        use_clipboard_output: bool,
    ) -> Result<bool, TranslationError> {
        if !self.try_begin() {
            return Ok(false);
        }
        let cancel = self.cancel.clone();
        let result = tokio::task::spawn_blocking({
            let text = text.to_string();
            move || {
                super::platform::deliver_replacement_text(
                    &text,
                    use_clipboard_output,
                    Some(&cancel),
                )
            }
        })
        .await
        .map_err(|e| TranslationError::Internal(format!("Task join error: {}", e)));
        self.end();
        match result {
            Ok(Ok(())) => Ok(true),
            Ok(Err(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn translate_and_replace(
        &self,
        text: &str,
        from: &str,
        to: &str,
        use_clipboard_output: bool,
    ) -> Result<ReplacementResult, TranslationError> {
        if text.trim().is_empty() {
            return Err(TranslationError::InvalidInput("Text is empty".to_string()));
        }
        if !self.try_begin() {
            return Ok(ReplacementResult {
                original: text.to_string(),
                replacement: String::new(),
                success: false,
                error: Some("cancelled".to_string()),
                fallback_to_overlay: false,
            });
        }

        let outcome = async {
            let translated = self
                .translation_service
                .run_primary(
                    crate::models::translation::TranslateChannel::Replace,
                    text,
                    from,
                    to,
                )
                .await?;
            if self.is_cancelled() {
                return Ok(ReplacementResult {
                    original: text.to_string(),
                    replacement: translated,
                    success: false,
                    error: Some("cancelled".to_string()),
                    fallback_to_overlay: true,
                });
            }

            let cancel = self.cancel.clone();
            let result = tokio::task::spawn_blocking({
                let t = translated.clone();
                move || {
                    super::platform::deliver_replacement_text(
                        &t,
                        use_clipboard_output,
                        Some(&cancel),
                    )
                }
            })
            .await
            .map_err(|e| TranslationError::Internal(format!("Task join error: {}", e)))?;

            match result {
                Ok(()) => Ok(ReplacementResult {
                    original: text.to_string(),
                    replacement: translated,
                    success: true,
                    error: None,
                    fallback_to_overlay: false,
                }),
                Err(e) => Ok(ReplacementResult {
                    original: text.to_string(),
                    replacement: translated,
                    success: false,
                    error: Some(e),
                    fallback_to_overlay: true,
                }),
            }
        }
        .await;

        self.end();
        outcome
    }
}
