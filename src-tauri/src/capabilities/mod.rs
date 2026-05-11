pub mod adapters;
pub mod browser_translation;
pub mod document_translation;
pub mod hook_monitor;
pub mod input_replacement;
pub mod input_replacement_impl;
pub mod ocr_translation;
pub mod platform;
pub mod selection_translation;
pub mod selection_translation_impl;

// Re-export key types for convenient access
pub use adapters::{
    AppContext, DomSelection, DomSelectionProvider, EmbeddedAppAdapter, EmbeddedAppType,
    InputAdapter, TargetAppDetector,
};
pub use browser_translation::{
    handle_browser_request, hover_payload_to_dom, mock_handle_browser_request,
    selection_payload_to_dom, BrowserTranslateOptions, BrowserTranslation, BrowserTranslationMode,
    BrowserTranslationResult, BrowserTranslationSource,
};
pub use document_translation::{
    DocumentContent, DocumentFormat, DocumentTranslateOptions, DocumentTranslation,
    TranslatedSegment,
};
pub use hook_monitor::{HookMonitor, MonitoredText};
pub use input_replacement::{InputReplacement, ReplacementResult};
pub use input_replacement_impl::DefaultInputReplacement;
pub use ocr_translation::{OcrTranslateOptions, OcrTranslation, OcrTranslationResult};
pub use platform::WindowsTargetAppDetector;
pub use selection_translation::{
    SelectionTranslateOptions, SelectionTranslation, SelectionTranslationResult,
};
pub use selection_translation_impl::DefaultSelectionTranslation;
