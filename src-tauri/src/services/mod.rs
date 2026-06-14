pub mod multi_dictionary;
pub mod translation;

pub use multi_dictionary::{DictionaryEntry, MultiSourceDictionary};
pub use translation::TranslationService;

// Re-export shared types from models
pub use crate::models::translation::{TranslationContext, TranslationJob, TranslationMode};
