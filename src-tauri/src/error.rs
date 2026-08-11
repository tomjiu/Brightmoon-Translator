//! Unified application error type.
//!
//! Provides a single `AppError` enum that covers all error domains in the
//! application, with structured variants, user-friendly Chinese messages,
//! and automatic conversions from common error types.

use serde::Serialize;
use std::fmt;

// ---------------------------------------------------------------------------
// AppError: the single error type for the whole application
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // ── Translation errors ────────────────────────────────────────────────
    #[error("No translation engine available")]
    NoEngine,

    #[error("All engines failed: {}", join_errors(.0))]
    AllEnginesFailed(Vec<String>),

    #[error("{} engine error: {}", engine, message)]
    EngineError { engine: String, message: String },

    #[error("{} rate limited{}", engine, fmt_retry(.retry_after_ms))]
    RateLimited {
        engine: String,
        retry_after_ms: Option<u64>,
    },

    #[error("Streaming not supported by current engine")]
    StreamingNotSupported,

    // ── Input validation errors ───────────────────────────────────────────
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Text is empty")]
    EmptyText,

    #[error("Text exceeds maximum length of {max} characters (got {got})")]
    TextTooLong { max: usize, got: usize },

    #[error("Invalid language code: {0}")]
    InvalidLanguage(String),

    #[error("Invalid file path: {0}")]
    InvalidPath(String),

    #[error("Path traversal detected")]
    PathTraversal,

    // ── IO / File errors ──────────────────────────────────────────────────
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Failed to write file: {0}")]
    FileWrite(String),

    // ── Network / HTTP errors ─────────────────────────────────────────────
    #[error("Network error: {0}")]
    Network(String),

    #[error("HTTP error: {status} {message}")]
    Http { status: u16, message: String },

    #[error("Request timeout")]
    Timeout,

    // ── Serialization errors ──────────────────────────────────────────────
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // ── Configuration errors ──────────────────────────────────────────────
    #[error("Configuration error: {0}")]
    Config(String),

    // ── OCR / Capture errors ──────────────────────────────────────────────
    #[error("OCR error: {0}")]
    Ocr(String),

    #[error("Screen capture error: {0}")]
    Capture(String),

    // ── Window / UI errors ────────────────────────────────────────────────
    #[error("Window error: {0}")]
    Window(String),

    #[error("Overlay error: {0}")]
    Overlay(String),

    // ── Hook / Injection errors ───────────────────────────────────────────
    #[error("Hook error: {0}")]
    Hook(String),

    #[error("Hook injection failed: {0}")]
    HookInjection(String),

    // ── Document processing errors ────────────────────────────────────────
    #[error("PDF error: {0}")]
    Pdf(String),

    #[error("EPUB error: {0}")]
    Epub(String),

    #[error("Subtitle error: {0}")]
    Subtitle(String),

    #[error("Document error: {0}")]
    Document(String),

    // ── Security / Encryption errors ──────────────────────────────────────
    #[error("Security error: {0}")]
    Security(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    // ── Cache / Storage errors ────────────────────────────────────────────
    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Database error: {0}")]
    Database(String),

    // ── Concurrency errors ────────────────────────────────────────────────
    #[error("Lock poisoned: {0}")]
    LockPoisoned(String),

    #[error("Task join error: {0}")]
    TaskJoin(String),

    // ── Generic errors ────────────────────────────────────────────────────
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not supported on this platform")]
    PlatformNotSupported,

    #[error("Operation cancelled")]
    Cancelled,
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

fn join_errors(errors: &[String]) -> String {
    errors.join("; ")
}

fn fmt_retry(retry_after_ms: &Option<u64>) -> String {
    match retry_after_ms {
        Some(ms) => format!(" (retry after {ms}ms)"),
        None => String::new(),
    }
}

// ---------------------------------------------------------------------------
// Serialize as a plain string for frontend compatibility
// ---------------------------------------------------------------------------

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ---------------------------------------------------------------------------
// User-friendly Chinese messages
// ---------------------------------------------------------------------------

impl AppError {
    /// Return a user-friendly Chinese error message.
    pub fn user_message(&self) -> String {
        match self {
            // Translation
            Self::NoEngine => "没有可用的翻译引擎".to_string(),
            Self::AllEnginesFailed(errors) => {
                format!("所有翻译引擎均失败: {}", errors.join("; "))
            },
            Self::EngineError { engine, message } => {
                format!("{engine} 引擎错误: {message}")
            },
            Self::RateLimited {
                engine,
                retry_after_ms,
            } => match retry_after_ms {
                Some(ms) => format!("{engine} 引擎限流，请 {ms}ms 后重试"),
                None => format!("{engine} 引擎限流，请稍后重试"),
            },
            Self::StreamingNotSupported => "当前引擎不支持流式翻译".to_string(),

            // Validation
            Self::InvalidInput(msg) => format!("输入无效: {msg}"),
            Self::EmptyText => "文本为空".to_string(),
            Self::TextTooLong { max, got } => {
                format!(
                    "文本超出最大长度限制 (最大 {max} 字符，当前 {got} 字符)"
                )
            },
            Self::InvalidLanguage(code) => format!("无效的语言代码: {code}"),
            Self::InvalidPath(msg) => format!("无效的文件路径: {msg}"),
            Self::PathTraversal => "检测到路径穿越攻击".to_string(),

            // IO
            Self::Io(e) => format!("IO 错误: {e}"),
            Self::FileNotFound(path) => format!("文件未找到: {path}"),
            Self::FileWrite(msg) => format!("文件写入失败: {msg}"),

            // Network
            Self::Network(msg) => format!("网络错误: {msg}"),
            Self::Http { status, message } => format!("HTTP 错误 {status}: {message}"),
            Self::Timeout => "请求超时".to_string(),

            // Serialization
            Self::Json(e) => format!("JSON 解析错误: {e}"),

            // Config
            Self::Config(msg) => format!("配置错误: {msg}"),

            // OCR / Capture
            Self::Ocr(msg) => format!("OCR 错误: {msg}"),
            Self::Capture(msg) => format!("截图错误: {msg}"),

            // Window
            Self::Window(msg) => format!("窗口错误: {msg}"),
            Self::Overlay(msg) => format!("悬浮窗错误: {msg}"),

            // Hook
            Self::Hook(msg) => format!("Hook 错误: {msg}"),
            Self::HookInjection(msg) => format!("Hook 注入失败: {msg}"),

            // Document
            Self::Pdf(msg) => format!("PDF 错误: {msg}"),
            Self::Epub(msg) => format!("EPUB 错误: {msg}"),
            Self::Subtitle(msg) => format!("字幕错误: {msg}"),
            Self::Document(msg) => format!("文档错误: {msg}"),

            // Security
            Self::Security(msg) => format!("安全错误: {msg}"),
            Self::Encryption(msg) => format!("加密错误: {msg}"),

            // Cache / Storage
            Self::Cache(msg) => format!("缓存错误: {msg}"),
            Self::Database(msg) => format!("数据库错误: {msg}"),

            // Concurrency
            Self::LockPoisoned(msg) => format!("锁错误: {msg}"),
            Self::TaskJoin(msg) => format!("任务执行错误: {msg}"),

            // Generic
            Self::Internal(msg) => format!("内部错误: {msg}"),
            Self::PlatformNotSupported => "当前平台不支持此操作".to_string(),
            Self::Cancelled => "操作已取消".to_string(),
        }
    }

    /// Log the error with appropriate severity and sanitized message.
    pub fn log(&self) {
        let sanitized = crate::security::sanitize_log_message(&self.to_string());
        match self {
            // Warnings: expected/recoverable
            Self::RateLimited { .. } | Self::Timeout | Self::Cancelled => {
                tracing::warn!("[AppError] {}", sanitized);
            },
            // Errors: unexpected failures
            Self::Internal(_)
            | Self::Io(_)
            | Self::Network(_)
            | Self::Http { .. }
            | Self::LockPoisoned(_)
            | Self::TaskJoin(_) => {
                tracing::error!("[AppError] {}", sanitized);
            },
            // Info: validation, user input
            Self::InvalidInput(_)
            | Self::EmptyText
            | Self::TextTooLong { .. }
            | Self::InvalidLanguage(_)
            | Self::InvalidPath(_)
            | Self::PathTraversal => {
                tracing::info!("[AppError] {}", sanitized);
            },
            // Everything else
            _ => {
                tracing::error!("[AppError] {}", sanitized);
            },
        }
    }
}

// ---------------------------------------------------------------------------
// From implementations: automatic conversion from common error types
// ---------------------------------------------------------------------------

/// From `TranslationError` (existing structured error in `models::error`)
impl From<crate::models::error::TranslationError> for AppError {
    fn from(err: crate::models::error::TranslationError) -> Self {
        use crate::models::error::TranslationError as TE;
        match err {
            TE::NoEngine => Self::NoEngine,
            TE::AllEnginesFailed { errors } => Self::AllEnginesFailed(errors),
            TE::EngineError { engine, message } => Self::EngineError { engine, message },
            TE::RateLimited {
                engine,
                retry_after_ms,
            } => Self::RateLimited {
                engine,
                retry_after_ms,
            },
            TE::InvalidInput(msg) => Self::InvalidInput(msg),
            TE::ConfigError(msg) => Self::Config(msg),
            TE::NetworkError(msg) => Self::Network(msg),
            TE::CacheError(msg) => Self::Cache(msg),
            TE::PluginError { name, message } => Self::EngineError {
                engine: name,
                message,
            },
            TE::StreamingNotSupported => Self::StreamingNotSupported,
            TE::Internal(msg) => Self::Internal(msg),
        }
    }
}

/// From `anyhow::Error`
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

/// From `String` (for backward compatibility with existing `Result<_, String>`)
impl From<String> for AppError {
    fn from(msg: String) -> Self {
        Self::Internal(msg)
    }
}

/// From `&str`
impl From<&str> for AppError {
    fn from(msg: &str) -> Self {
        Self::Internal(msg.to_string())
    }
}

/// From `reqwest::Error`
impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::Timeout
        } else if err.is_connect() {
            Self::Network(format!("Connection failed: {err}"))
        } else {
            Self::Network(err.to_string())
        }
    }
}

/// From `tokio::task::JoinError`
impl From<tokio::task::JoinError> for AppError {
    fn from(err: tokio::task::JoinError) -> Self {
        Self::TaskJoin(err.to_string())
    }
}

/// From `std::sync::PoisonError` (Mutex lock poisoning)
impl<T> From<std::sync::PoisonError<T>> for AppError {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        Self::LockPoisoned(err.to_string())
    }
}

/// From `windows::core::Error` (Windows API errors)
#[cfg(target_os = "windows")]
impl From<windows::core::Error> for AppError {
    fn from(err: windows::core::Error) -> Self {
        Self::Internal(format!("Windows API error: {err}"))
    }
}

// ---------------------------------------------------------------------------
// Backward-compatible conversion: AppError -> String
// ---------------------------------------------------------------------------

impl From<AppError> for String {
    fn from(err: AppError) -> String {
        err.to_string()
    }
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

impl AppError {
    /// Create a network error from a status code and body.
    pub fn http_status(status: u16, body: &str) -> Self {
        Self::Http {
            status,
            message: body.to_string(),
        }
    }

    /// Create an engine error.
    pub fn engine(engine: &str, message: impl fmt::Display) -> Self {
        Self::EngineError {
            engine: engine.to_string(),
            message: message.to_string(),
        }
    }

    /// Wrap any error as an internal error with context.
    pub fn internal_with(err: impl fmt::Display, context: &str) -> Self {
        Self::Internal(format!("{context}: {err}"))
    }
}
