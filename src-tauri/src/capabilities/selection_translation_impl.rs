use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::adapters::TargetAppDetector;
use super::selection_translation::{
    SelectionTranslateOptions, SelectionTranslation, SelectionTranslationResult,
};
use crate::config::AppConfig;
use crate::models::error::TranslationError;
use crate::overlay;
use crate::overlay::FollowController;
use crate::selection::SelectionProviderManager;
use crate::services::TranslationService;

/// Default desktop implementation of SelectionTranslation.
/// Composes: TargetAppDetector -> SelectionProviderManager -> TranslationService -> OverlayPresenter.
///
/// Uses the full provider chain (UIA → clipboard) for all apps, including embedded apps.
/// Modern Electron/Chromium apps often support UIA TextPattern, so we try UIA first
/// and let the provider chain naturally fall back to clipboard if needed.
pub struct DefaultSelectionTranslation {
    selection_manager: Arc<SelectionProviderManager>,
    translation_service: Arc<TranslationService>,
    config: Arc<Mutex<AppConfig>>,
    app_handle: tauri::AppHandle,
    app_detector: Arc<dyn TargetAppDetector>,
    follow_controller: Arc<FollowController>,
}

impl DefaultSelectionTranslation {
    pub fn new(
        selection_manager: Arc<SelectionProviderManager>,
        translation_service: Arc<TranslationService>,
        config: Arc<Mutex<AppConfig>>,
        app_handle: tauri::AppHandle,
        app_detector: Arc<dyn TargetAppDetector>,
        follow_controller: Arc<FollowController>,
    ) -> Self {
        Self {
            selection_manager,
            translation_service,
            config,
            app_handle,
            app_detector,
            follow_controller,
        }
    }

    /// Show the overlay window with translation result and start following.
    async fn show_overlay(
        &self,
        source_text: &str,
        translated_text: &str,
        source_app: &str,
        window_title: &str,
        bounds: Option<&crate::selection::SelectionBounds>,
        overlay_level: Option<u8>,
    ) -> Result<(), String> {
        // tokio Mutex: await — blocking_lock panics inside the async runtime.
        let config = self.config.lock().await;
        let config_level = config.overlay_level;
        let dismiss_ms = config.overlay_auto_dismiss_ms;
        let overlay_follow_mode = config.overlay_follow_mode.clone();
        drop(config);

        let level: overlay::OverlayLevel = overlay_level.unwrap_or(config_level).into();

        // Position overlay: prefer selection bounds, fall back to cursor
        let (cursor_x, cursor_y) = get_cursor_position();
        let pos = overlay::positioner::calculate_position(bounds, cursor_x, cursor_y);

        let content = overlay::OverlayContent {
            source: source_text.to_string(),
            translated: translated_text.to_string(),
            source_app: Some(source_app.to_string()),
            window_title: Some(window_title.to_string()),
        };
        let (mut w, h) = overlay::window_manager::estimate_mt_card_size(&translated_text);
        w = w.max(pos.width.min(460.0));
        let html = overlay::html_builder::build_html(&content, level, dismiss_ms, None);
        overlay::window_manager::create_overlay_window(
            &self.app_handle,
            &html,
            pos.x,
            pos.y,
            w,
            h,
            true,
        )?;

        // Determine overlay state based on level
        let overlay_state = match level {
            overlay::OverlayLevel::Minimal => overlay::OverlayState::Transient,
            overlay::OverlayLevel::Standard => overlay::OverlayState::Interactive,
            overlay::OverlayLevel::Full => overlay::OverlayState::Interactive,
        };

        // Determine follow mode from dedicated overlay_follow_mode config
        let follow_mode = match overlay_follow_mode.as_str() {
            "cursor" => overlay::FollowMode::Cursor,
            "target_bounds" => overlay::FollowMode::TargetBounds,
            _ => overlay::FollowMode::None,
        };

        // Start following (non-blocking)
        let fc = self.follow_controller.clone();
        let target_bounds = bounds.map(|b| overlay::TargetBounds {
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        });
        tokio::spawn(async move {
            fc.update_target_bounds(target_bounds).await;
            fc.start(follow_mode, overlay_state).await;
        });

        Ok(())
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
            .run_full(
                crate::models::translation::TranslateChannel::Selection,
                &selection.text,
                &from,
                &to,
            )
            .await?;

        // Step 3: Show overlay — multi-engine join when router returns >1 (parity with auto_watch)
        if options.show_overlay {
            let display = response.display_text();
            if !display.is_empty() {
                let source_app = app_ctx
                    .as_ref()
                    .map(|ctx| ctx.app_name.clone())
                    .unwrap_or_else(|| selection.source_app.clone());

                let window_title = app_ctx
                    .as_ref()
                    .map(|ctx| ctx.window_title.clone())
                    .unwrap_or_else(|| selection.window_title.clone());

                let _ = self.show_overlay(
                    &selection.text,
                    &display,
                    &source_app,
                    &window_title,
                    selection.bounds.as_ref(),
                    options.overlay_level,
                )
                .await;
            }
        }

        let level: overlay::OverlayLevel = match options.overlay_level {
            Some(l) => l.into(),
            None => self.config.lock().await.overlay_level.into(),
        };

        Ok(SelectionTranslationResult {
            source_text: selection.text,
            source_app: app_ctx
                .map(|ctx| ctx.app_name)
                .unwrap_or_else(|| selection.source_app),
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
            .run_full(
                crate::models::translation::TranslateChannel::Selection,
                text,
                &from,
                &to,
            )
            .await?;

        // Show overlay if requested (no bounds; multi-engine join like translate_selection)
        if options.show_overlay {
            let display = response.display_text();
            if !display.is_empty() {
                let _ = self
                    .show_overlay(text, &display, "direct", "", None, options.overlay_level)
                    .await;
            }
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
