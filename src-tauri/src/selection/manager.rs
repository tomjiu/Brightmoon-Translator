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

    /// Get the current selection by trying providers in priority order.
    /// Returns the first successful result, or None if all providers fail.
    pub async fn get_selection(&self) -> Option<SelectionResult> {
        let mut tried: Vec<&str> = Vec::new();
        for provider in &self.providers {
            let name = provider.name();
            tried.push(name);
            log::debug!("[selection_manager] Trying provider '{}' (priority {})", name, provider.priority());
            match provider.get_selection().await {
                Some(result) => {
                    if !result.text.trim().is_empty() {
                        log::info!(
                            "[selection_manager] Provider '{}' succeeded: {} chars from '{}'",
                            name, result.text.len(), result.source_app
                        );
                        return Some(result);
                    } else {
                        log::debug!("[selection_manager] Provider '{}' returned empty text", name);
                    }
                }
                None => {
                    log::debug!("[selection_manager] Provider '{}' returned None", name);
                }
            }
        }
        log::warn!("[selection_manager] All providers failed. Tried: {:?}", tried);
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
                log::debug!("[selection_manager] Skipping provider '{}' (excluded)", name);
                continue;
            }
            tried.push(name);
            log::debug!("[selection_manager] Trying provider '{}' (priority {})", name, provider.priority());
            match provider.get_selection().await {
                Some(result) => {
                    if !result.text.trim().is_empty() {
                        log::info!(
                            "[selection_manager] Provider '{}' succeeded: {} chars from '{}'",
                            name, result.text.len(), result.source_app
                        );
                        return Some(result);
                    } else {
                        log::debug!("[selection_manager] Provider '{}' returned empty text", name);
                    }
                }
                None => {
                    log::debug!("[selection_manager] Provider '{}' returned None", name);
                }
            }
        }
        log::warn!(
            "[selection_manager] All providers failed. Tried: {:?}, Skipped: {:?}",
            tried, skipped
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
