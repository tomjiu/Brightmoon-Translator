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
        for provider in &self.providers {
            if let Some(result) = provider.get_selection().await {
                if !result.text.trim().is_empty() {
                    return Some(result);
                }
            }
        }
        None
    }

    /// Get the current selection, skipping providers whose names are in `exclude`.
    /// Used for strategy dispatch: e.g., skip UIA for embedded apps where it won't work.
    pub async fn get_selection_excluding(&self, exclude: &[&str]) -> Option<SelectionResult> {
        for provider in &self.providers {
            if exclude.contains(&provider.name()) {
                continue;
            }
            if let Some(result) = provider.get_selection().await {
                if !result.text.trim().is_empty() {
                    return Some(result);
                }
            }
        }
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
