use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::adapters::TargetAppDetector;
use super::selection_translation::{
    SelectionTranslateOptions, SelectionTranslation, SelectionTranslationResult,
};
use crate::config::AppConfig;
use crate::models::error::TranslationError;
use crate::models::translation::TranslateResponse;
use crate::overlay;
use crate::selection::SelectionProviderManager;
use crate::services::TranslationService;

/// Default desktop implementation of `SelectionTranslation`.
/// Composes: `TargetAppDetector` -> `SelectionProviderManager` -> `TranslationService` -> `OverlayPresenter`.
///
/// Uses the full provider chain (UIA → clipboard) for all apps, including embedded apps.
/// Modern Electron/Chromium apps often support UIA `TextPattern`, so we try UIA first
/// and let the provider chain naturally fall back to clipboard if needed.
pub struct DefaultSelectionTranslation {
    selection_manager: Arc<SelectionProviderManager>,
    translation_service: Arc<TranslationService>,
    config: Arc<Mutex<AppConfig>>,
    app_handle: tauri::AppHandle,
    app_detector: Arc<dyn TargetAppDetector>,
}

impl DefaultSelectionTranslation {
    pub fn new(
        selection_manager: Arc<SelectionProviderManager>,
        translation_service: Arc<TranslationService>,
        config: Arc<Mutex<AppConfig>>,
        app_handle: tauri::AppHandle,
        app_detector: Arc<dyn TargetAppDetector>,
    ) -> Self {
        Self {
            selection_manager,
            translation_service,
            config,
            app_handle,
            app_detector,
        }
    }

    /// Show the structured translate card (user-initiated 划词 → takes focus;
    /// the FE closes it on blur). Follow is deferred for the new card window.
    async fn show_card(
        &self,
        source_text: &str,
        response: &TranslateResponse,
        bounds: Option<&crate::selection::SelectionBounds>,
    ) -> Result<(), String> {
        // Position: prefer selection bounds (below, or above near the screen
        // bottom), fall back to cursor.
        let (cursor_x, cursor_y) = get_cursor_position();
        let display = response.display_text();
        let (mut w, h) = overlay::window_manager::estimate_mt_card_size(&display);
        let pos = if let Some(b) = bounds {
            let (x, y) = overlay::positioner::place_near_bounds(b, w, h, cursor_x, cursor_y);
            overlay::OverlayPosition::new(x, y, w, h)
        } else {
            overlay::positioner::calculate_position(bounds, cursor_x, cursor_y)
        };
        w = w.max(pos.width.min(460.0));

        let (from, to) = {
            let c = self.config.lock().await;
            (c.default_from.clone(), c.default_to.clone())
        };
        let payload = overlay::translate_card::TranslateCardData::Mt(
            overlay::translate_card::MtCardData {
                source: source_text.to_string(),
                from,
                to,
                response: response.clone(),
                total_engines: self.translation_service.enabled_engine_count().await,
            },
        );
        overlay::translate_card::show_translate_card(
            &self.app_handle,
            &payload,
            pos.x,
            pos.y,
            w,
            h,
            overlay::translate_card::TranslateCardOptions {
                steal_focus: true,
                keep_alive: None,
            },
        )
        .await
    }
}

#[async_trait]
impl SelectionTranslation for DefaultSelectionTranslation {
    async fn translate_selection(
        &self,
        options: SelectionTranslateOptions,
    ) -> Result<SelectionTranslationResult, TranslationError> {
        // Step 1: Detect foreground app for strategy dispatch
        let app_ctx = self.app_detector.detect().await;

        // Step 2: Process-routed selection (Easydict: Electron→clipboard, terminal→UIA only).
        // If empty and OCR force pickup is on (+ optional modifier), OCR near cursor.
        let (ocr_force, exclude) = {
            let c = self.config.lock().await;
            (
                crate::selection::ocr_force_allowed(&c.selection_ux),
                c.selection_ux.exclude_processes.clone(),
            )
        };
        let selection = self.selection_manager.get_selection_routed(&exclude).await;
        let selection = match selection {
            Some(s) if !s.text.trim().is_empty() => s,
            _ => {
                if ocr_force {
                    let pick = tokio::task::spawn_blocking(|| {
                        crate::selection::hover_pick::pick_word_near_cursor_ocr(100, 40)
                    })
                    .await
                    .ok()
                    .flatten();
                    if let Some(p) = pick {
                        tracing::info!(
                            "[selection_translate] OCR force pickup: {} chars via {}",
                            p.word.len(),
                            p.source
                        );
                        crate::selection::SelectionResult {
                            text: p.word,
                            source_app: "ocr-force".into(),
                            window_title: String::new(),
                            bounds: p.bounds,
                            confidence: 0.55,
                            provider: "ocr_force",
                        }
                    } else {
                        return Err(TranslationError::InvalidInput(
                            "No text selected".to_string(),
                        ));
                    }
                } else {
                    return Err(TranslationError::InvalidInput(
                        "No text selected".to_string(),
                    ));
                }
            },
        };

        tracing::info!(
            "[selection_translate] Got selection via '{}': {} chars, app='{}'",
            selection.provider,
            selection.text.len(),
            selection.source_app
        );

        let config = self.config.lock().await;
        let from = options
            .from
            .clone()
            .unwrap_or_else(|| config.default_from.clone());
        let to = options
            .to
            .clone()
            .unwrap_or_else(|| config.default_to.clone());
        drop(config);

        let response = self
            .translation_service
            .run_quick(
                crate::models::translation::TranslateChannel::Selection,
                &selection.text,
                &from,
                &to,
            )
            .await?;

        // Step 3: Show the translate card (user-initiated → takes focus).
        if options.show_overlay
            && !response.display_text().is_empty() {
                let _ = self
                    .show_card(&selection.text, &response, selection.bounds.as_ref())
                    .await;
            }

        let level: overlay::OverlayLevel = match options.overlay_level {
            Some(l) => l.into(),
            None => self.config.lock().await.overlay_level.into(),
        };

        Ok(SelectionTranslationResult {
            source_text: selection.text,
            source_app: app_ctx.map_or_else(|| selection.source_app, |ctx| ctx.app_name),
            response,
            overlay_level: level,
            selection_provider: selection.provider.to_string(),
        })
    }

    async fn translate_text(
        &self,
        text: &str,
        options: SelectionTranslateOptions,
    ) -> Result<SelectionTranslationResult, TranslationError> {
        if text.trim().is_empty() {
            return Err(TranslationError::InvalidInput("Text is empty".to_string()));
        }

        let config = self.config.lock().await;
        let from = options
            .from
            .clone()
            .unwrap_or_else(|| config.default_from.clone());
        let to = options
            .to
            .clone()
            .unwrap_or_else(|| config.default_to.clone());
        drop(config);

        let response = self
            .translation_service
            .run_quick(
                crate::models::translation::TranslateChannel::Selection,
                text,
                &from,
                &to,
            )
            .await?;

        // Show the translate card if requested (no bounds)
        if options.show_overlay
            && !response.display_text().is_empty() {
                let _ = self.show_card(text, &response, None).await;
            }

        let level: overlay::OverlayLevel = match options.overlay_level {
            Some(l) => l.into(),
            None => self.config.lock().await.overlay_level.into(),
        };

        Ok(SelectionTranslationResult {
            source_text: text.to_string(),
            source_app: "direct".to_string(),
            response,
            overlay_level: level,
            selection_provider: "direct".to_string(),
        })
    }
}

/// Get the current cursor position. Falls back to (100, 100) if unavailable.
fn get_cursor_position() -> (f64, f64) {
    // S1-6: delegate to the shared crate::win::cursor_pos() instead of a
    // local GetCursorPos FFI block.
    crate::win::cursor_pos()
}
