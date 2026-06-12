#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::Write;
use tracing_subscriber::EnvFilter;

/// Set a custom panic hook that gracefully handles stderr pipe errors.
/// In Windows GUI applications, the stderr pipe may be closed unexpectedly,
/// causing "failed printing to stderr" panics (os error 232).
fn set_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        // Try to get the panic message
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        let location = panic_info
            .location()
            .map(|l| format!(" at {}:{}", l.file(), l.line()))
            .unwrap_or_default();

        // Try to log via tracing (which may also fail if stderr is closed)
        tracing::error!("Panic{}: {}", location, message);

        // Also try eprintln, but ignore errors (pipe may be closed)
        let _ = eprintln!("Panic{}: {}", location, message);
    }));
}

/// A `MakeWriter` implementation that sanitizes log output to remove
/// sensitive patterns (API keys, tokens) before writing to stderr.
struct SanitizingWriter;

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SanitizingWriter {
    type Writer = SanitizedStderr;

    fn make_writer(&self) -> Self::Writer {
        SanitizedStderr
    }
}

struct SanitizedStderr;

impl Write for SanitizedStderr {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let sanitized = moontranslator_lib::security::sanitize_log_message(&text);
        let mut stderr = std::io::stderr();
        // Ignore broken pipe errors (os error 232) - common in Windows GUI apps
        match stderr.write_all(sanitized.as_bytes()) {
            Ok(()) => Ok(buf.len()),
            Err(e) if e.raw_os_error() == Some(232) => Ok(buf.len()), // ERROR_NO_DATA: pipe is being closed
            Err(e) => Err(e),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match std::io::stderr().flush() {
            Ok(()) => Ok(()),
            Err(e) if e.raw_os_error() == Some(232) => Ok(()), // Ignore broken pipe
            Err(e) => Err(e),
        }
    }
}

fn main() {
    // Set custom panic hook before tracing is initialized
    set_panic_hook();

    let filter = if cfg!(debug_assertions) {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("warn")
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(SanitizingWriter)
        .init();

    moontranslator_lib::run()
}
