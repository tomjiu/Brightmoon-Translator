pub mod adapters;
pub mod browser_translation;
pub mod document_translation;
pub mod input_replacement;
pub mod ocr_translation;
pub mod selection_translation;

// Re-export key types for convenient access
pub use adapters::{
    AppContext, DomSelection, DomSelectionProvider, EmbeddedAppAdapter,
    EmbeddedAppType, InputAdapter, TargetAppDetector,
};
pub use browser_translation::{
    BrowserTranslation, BrowserTranslationMode, BrowserTranslationResult,
    BrowserTranslationSource, BrowserTranslateOptions,
};
pub use document_translation::{
    DocumentContent, DocumentFormat, DocumentTranslateOptions, DocumentTranslation,
    TranslatedSegment,
};
pub use input_replacement::{InputReplacement, ReplacementResult};
pub use ocr_translation::{OcrTranslation, OcrTranslationResult, OcrTranslateOptions};
pub use selection_translation::{
    SelectionTranslateOptions, SelectionTranslation, SelectionTranslationResult,
};
