use super::process_class::{foreground_process, SelectionStrategy};
use super::{SelectionProvider, SelectionResult};
use std::sync::Arc;

/// Manages multiple selection providers and tries them in priority order.
/// Falls back from higher-priority to lower-priority providers automatically.
/// Lower priority number = higher priority (tried first).
pub struct SelectionProviderManager {
    providers: Vec<Arc<dyn SelectionProvider>>,
}

impl SelectionProviderManager {
    /// Create a manager with the default provider chain, sorted by priority.
    pub fn with_defaults() -> Self {
        let mut providers: Vec<Arc<dyn SelectionProvider>> = vec![
            Arc::new(super::uiautomation::UiAutomationSelectionProvider),
            Arc::new(super::clipboard::ClipboardSelectionProvider),
        ];
        // Sort by priority: lower number = higher priority (tried first)
        providers.sort_by_key(|p| p.priority());
        Self { providers }
    }

    /// Create a manager with custom providers, sorted by priority.
    pub fn new(mut providers: Vec<Arc<dyn SelectionProvider>>) -> Self {
        providers.sort_by_key(|p| p.priority());
        Self { providers }
    }

    /// Get selection with Easydict-style process routing (no exclude list).
    pub async fn get_selection(&self) -> Option<SelectionResult> {
        self.get_selection_routed(&[]).await
    }

    /// Get selection with optional process-name exclude list (from settings).
    pub async fn get_selection_routed(
        &self,
        exclude_processes: &[String],
    ) -> Option<SelectionResult> {
        let fg = foreground_process();
        let strategy = fg
            .as_ref()
            .map(|p| p.strategy(exclude_processes))
            .unwrap_or(SelectionStrategy::UiaThenClipboard);

        if let Some(ref p) = fg {
            tracing::info!(
                "[selection_manager] fg='{}' pid={} electron={} browser={} terminal={} strategy={:?}",
                p.process_name,
                p.process_id,
                p.is_electron,
                p.is_browser,
                p.is_terminal,
                strategy
            );
        }

        // Easydict: non-text clipboard spam → suppress full selection for process (5 min)
        let suppressed = fg
            .as_ref()
            .map(|p| super::clipboard::is_process_clipboard_suppressed(&p.process_name))
            .unwrap_or(false);
        if suppressed {
            tracing::debug!(
                "[selection_manager] Skip — process clipboard suppressed (non-text history)"
            );
            return None;
        }

        match strategy {
            SelectionStrategy::Skip => {
                tracing::debug!("[selection_manager] Skip (self or excluded process)");
                None
            },
            SelectionStrategy::UiaOnly => self.try_providers_in_order(&["uiautomation"]).await,
            SelectionStrategy::ClipboardThenUia => {
                self.try_providers_in_order(&["clipboard", "uiautomation"])
                    .await
            },
            SelectionStrategy::UiaThenClipboard => {
                self.try_providers_in_order(&["uiautomation", "clipboard"])
                    .await
            },
        }
    }

    async fn try_providers_in_order(&self, order: &[&str]) -> Option<SelectionResult> {
        let mut tried: Vec<&str> = Vec::new();
        for want in order {
            let Some(provider) = self.providers.iter().find(|p| p.name() == *want) else {
                continue;
            };
            let name = provider.name();
            tried.push(name);
            tracing::debug!("[selection_manager] Trying provider '{}' (routed)", name);
            match provider.get_selection().await {
                Some(result) if !result.text.trim().is_empty() => {
                    tracing::info!(
                        "[selection_manager] Provider '{}' succeeded: {} chars from '{}'",
                        name,
                        result.text.len(),
                        result.source_app
                    );
                    return Some(result);
                },
                Some(_) => {
                    tracing::debug!(
                        "[selection_manager] Provider '{}' returned empty text",
                        name
                    );
                },
                None => {
                    tracing::debug!("[selection_manager] Provider '{}' returned None", name);
                },
            }
        }
        tracing::warn!(
            "[selection_manager] All routed providers failed. Tried: {:?}",
            tried
        );
        None
    }

    /// Get the current selection, skipping providers whose names are in `exclude`.
    /// Used for strategy dispatch: e.g., skip UIA for embedded apps where it won't work.
    pub async fn get_selection_excluding(&self, exclude: &[&str]) -> Option<SelectionResult> {
        let mut tried: Vec<&str> = Vec::new();
        let mut skipped: Vec<&str> = Vec::new();
        for provider in &self.providers {
            let name = provider.name();
            if exclude.contains(&name) {
                skipped.push(name);
                tracing::debug!(
                    "[selection_manager] Skipping provider '{}' (excluded)",
                    name
                );
                continue;
            }
            tried.push(name);
            tracing::debug!(
                "[selection_manager] Trying provider '{}' (priority {})",
                name,
                provider.priority()
            );
            match provider.get_selection().await {
                Some(result) => {
                    if !result.text.trim().is_empty() {
                        tracing::info!(
                            "[selection_manager] Provider '{}' succeeded: {} chars from '{}'",
                            name,
                            result.text.len(),
                            result.source_app
                        );
                        return Some(result);
                    } else {
                        tracing::debug!(
                            "[selection_manager] Provider '{}' returned empty text",
                            name
                        );
                    }
                },
                None => {
                    tracing::debug!("[selection_manager] Provider '{}' returned None", name);
                },
            }
        }
        tracing::warn!(
            "[selection_manager] All providers failed. Tried: {:?}, Skipped: {:?}",
            tried,
            skipped
        );
        None
    }

    /// List all registered providers (for diagnostics)
    pub fn list_providers(&self) -> Vec<(&'static str, u32)> {
        self.providers
            .iter()
            .map(|p| (p.name(), p.priority()))
            .collect()
    }
}
