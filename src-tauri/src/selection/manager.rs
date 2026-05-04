use super::{SelectionProvider, SelectionResult};
use std::sync::Arc;

/// Manages multiple selection providers and tries them in priority order.
/// Falls back from higher-priority to lower-priority providers automatically.
pub struct SelectionProviderManager {
    providers: Vec<Arc<dyn SelectionProvider>>,
}

impl SelectionProviderManager {
    /// Create a manager with the default provider chain:
    /// 1. UiAutomationSelectionProvider (priority 10)
    /// 2. ClipboardSelectionProvider (priority 100)
    pub fn with_defaults() -> Self {
        let providers: Vec<Arc<dyn SelectionProvider>> = vec![
            Arc::new(super::uiautomation::UiAutomationSelectionProvider),
            Arc::new(super::clipboard::ClipboardSelectionProvider),
        ];
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

    /// List all registered providers (for diagnostics)
    pub fn list_providers(&self) -> Vec<(&'static str, u32)> {
        self.providers
            .iter()
            .map(|p| (p.name(), p.priority()))
            .collect()
    }
}
