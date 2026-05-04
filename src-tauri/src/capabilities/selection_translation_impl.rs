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
/// Strategy dispatch based on AppContext:
/// - Standard apps: full provider chain (UIA → clipboard)
/// - Embedded apps (Electron/WebView2/CEF): clipboard-first (UIA typically fails)
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
    fn show_overlay(
        &self,
        source_text: &str,
        translated_text: &str,
        source_app: &str,
        window_title: &str,
        bounds: Option<&crate::selection::SelectionBounds>,
        overlay_level: Option<u8>,
    ) -> Result<(), String> {
        let config = self.config.blocking_lock();
        let config_level = config.overlay_level;
        let dismiss_ms = config.overlay_auto_dismiss_ms;
        let overlay_follow_mode = config.overlay_follow_mode.clone();
        drop(config);

        let level: overlay::OverlayLevel = overlay_level.unwrap_or(config_level).into();

        // Position overlay: prefer selection bounds, fall back to cursor
        let (cursor_x, cursor_y) = get_cursor_position();
        let pos =
            overlay::positioner::calculate_position(bounds, cursor_x, cursor_y);

        let content = overlay::OverlayContent {
            source: source_text.to_string(),
            translated: translated_text.to_string(),
            source_app: Some(source_app.to_string()),
            window_title: Some(window_title.to_string()),
        };
        let html = overlay::html_builder::build_html(&content, level, dismiss_ms);
        overlay::window_manager::create_overlay_window(
            &self.app_handle,
            &html,
            pos.x,
            pos.y,
            pos.width,
            pos.height,
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

        // Step 2: Get selection using strategy based on app type
        // Embedded apps (Electron/WebView2/CEF): skip UIA, use clipboard directly
        // Standard apps: full provider chain (UIA → clipboard)
        let selection = if app_ctx.as_ref().map_or(false, |ctx| ctx.is_embedded) {
            self.selection_manager
                .get_selection_excluding(&["uiautomation"])
                .await
        } else {
            self.selection_manager.get_selection().await
        };

        let selection = selection.ok_or(TranslationError::InvalidInput(
            "No text selected".to_string(),
        ))?;

        let config = self.config.lock().await;
        let from = options.from.clone().unwrap_or_else(|| config.default_from.clone());
        let to = options.to.clone().unwrap_or_else(|| config.default_to.clone());
        drop(config);

        let response = self
            .translation_service
            .translate(&selection.text, &from, &to)
            .await?;

        // Step 3: Show overlay with app-context-aware presentation
        if options.show_overlay {
            if let Some(first) = response.results.first() {
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
                    &first.text,
                    &source_app,
                    &window_title,
                    selection.bounds.as_ref(),
                    options.overlay_level,
                );
            }
        }

        let level: overlay::OverlayLevel = options
            .overlay_level
            .unwrap_or_else(|| self.config.blocking_lock().overlay_level)
            .into();

        Ok(SelectionTranslationResult {
            source_text: selection.text,
            source_app: app_ctx
                .map(|ctx| ctx.app_name)
                .unwrap_or_else(|| selection.source_app),
            response,
            overlay_level: level,
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
        let from = options.from.clone().unwrap_or_else(|| config.default_from.clone());
        let to = options.to.clone().unwrap_or_else(|| config.default_to.clone());
        drop(config);

        let response = self.translation_service.translate(text, &from, &to).await?;

        // Show overlay if requested (no bounds info for direct text)
        if options.show_overlay {
            if let Some(first) = response.results.first() {
                let _ = self.show_overlay(
                    text,
                    &first.text,
                    "direct",
                    "",
                    None,
                    options.overlay_level,
                );
            }
        }

        let level: overlay::OverlayLevel = options
            .overlay_level
            .unwrap_or_else(|| self.config.blocking_lock().overlay_level)
            .into();

        Ok(SelectionTranslationResult {
            source_text: text.to_string(),
            source_app: "direct".to_string(),
            response,
            overlay_level: level,
        })
    }
}

/// Get the current cursor position. Falls back to (100, 100) if unavailable.
fn get_cursor_position() -> (f64, f64) {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct POINT {
            x: i32,
            y: i32,
        }
        extern "system" {
            fn GetCursorPos(lpPoint: *mut POINT) -> i32;
        }
        unsafe {
            let mut point = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut point) != 0 {
                return (point.x as f64, point.y as f64);
            }
        }
    }
    (100.0, 100.0)
}
