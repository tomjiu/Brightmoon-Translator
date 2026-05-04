use serde::{Deserialize, Serialize};
use std::fmt;

/// Unified translation error type
/// Replaces ad-hoc String errors with a structured enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "detail")]
pub enum TranslationError {
    /// No translation engine available
    NoEngine,
    /// All engines failed to translate
    AllEnginesFailed { errors: Vec<String> },
    /// Engine-specific error
    EngineError { engine: String, message: String },
    /// Rate limited by engine
    RateLimited { engine: String, retry_after_ms: Option<u64> },
    /// Invalid input text
    InvalidInput(String),
    /// Configuration error
    ConfigError(String),
    /// Network/HTTP error
    NetworkError(String),
    /// Cache operation failed
    CacheError(String),
    /// Plugin error
    PluginError { name: String, message: String },
    /// Streaming not supported by current engine
    StreamingNotSupported,
    /// Generic internal error
    Internal(String),
}

impl fmt::Display for TranslationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoEngine => write!(f, "No translation engine available"),
            Self::AllEnginesFailed { errors } => {
                write!(f, "All engines failed: {}", errors.join("; "))
            }
            Self::EngineError { engine, message } => {
                write!(f, "{} engine error: {}", engine, message)
            }
            Self::RateLimited { engine, retry_after_ms } => {
                write!(f, "{} rate limited", engine)?;
                if let Some(ms) = retry_after_ms {
                    write!(f, " (retry after {}ms)", ms)?;
                }
                Ok(())
            }
            Self::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            Self::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::CacheError(msg) => write!(f, "Cache error: {}", msg),
            Self::PluginError { name, message } => {
                write!(f, "Plugin '{}' error: {}", name, message)
            }
            Self::StreamingNotSupported => {
                write!(f, "Streaming not supported by current engine")
            }
            Self::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for TranslationError {}

/// Convert from anyhow::Error for backward compatibility
impl From<anyhow::Error> for TranslationError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

/// Convert from String for backward compatibility
impl From<String> for TranslationError {
    fn from(err: String) -> Self {
        Self::Internal(err)
    }
}

/// API error response structure
#[derive(Serialize)]
pub struct ApiError {
    pub error: String,
}

impl From<&TranslationError> for ApiError {
    fn from(err: &TranslationError) -> Self {
        Self {
            error: err.to_string(),
        }
    }
}
