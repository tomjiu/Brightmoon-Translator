pub mod browser_protocol;
pub mod config;
pub mod detection;
pub mod dictionary;
pub mod error;
pub mod glossary;
pub mod memory;
pub mod translation;

// Re-export all types for convenient access via `models::*`
pub use browser_protocol::*;
pub use config::*;
pub use detection::DetectionResult;
pub use dictionary::{Definition, DictionaryResult, Meaning};
pub use error::{ApiError, TranslationError};
pub use glossary::GlossaryEntry;
pub use memory::{HistoryItem, WordBookItem};
pub use translation::*;
