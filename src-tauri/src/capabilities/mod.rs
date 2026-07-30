pub mod adapters;
pub mod browser_translation;
pub mod document_translation;
pub mod hook_monitor;
pub mod input_replacement;
pub mod input_replacement_impl;
pub mod platform;
pub mod selection_translation;
pub mod selection_translation_impl;

// Re-export key types for convenient access
pub use adapters::{AppContext, TargetAppDetector};
pub use browser_translation::handle_browser_request;
pub use hook_monitor::HookMonitor;
pub use input_replacement::InputReplacement;
pub use input_replacement_impl::DefaultInputReplacement;
pub use platform::WindowsTargetAppDetector;
pub use selection_translation::{SelectionTranslateOptions, SelectionTranslation};
pub use selection_translation_impl::DefaultSelectionTranslation;
