pub mod dictionary_source;
pub mod japanese_dictionary;
pub mod multi_dictionary;
pub mod translation;

pub use dictionary_source::{DictEntryResult, DictSourceConfig, DictionarySource, SourceRegistry};
pub use japanese_dictionary::{JapaneseDictionary, JapaneseEntry, JapaneseMeaning};
pub use multi_dictionary::{DictionaryEntry, MultiSourceDictionary};
pub use translation::TranslationService;

// Re-export shared types from models
pub use crate::models::translation::{
    TranslateChannel, TranslateOutcome, TranslateRequest, TranslationContext, TranslationJob,
    TranslationMode,
};
