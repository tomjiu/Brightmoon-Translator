#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing_subscriber::EnvFilter;

/// Set a custom panic hook that gracefully handles stderr pipe errors.
/// In Windows GUI applications, the stderr pipe may be closed unexpectedly,
/// causing "failed printing to stderr" panics (os error 232).
fn set_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
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

        let line = format!("Panic{}: {}", location, message);

        // Try to log via tracing (which may also fail if stderr is closed)
        tracing::error!("{}", line);

        // Also try eprintln, but ignore errors (pipe may be closed)
        let _ = eprintln!("{}", line);

        // Last-resort: persist the panic to a crash file so we have a
        // forensic trail even if the tracing subscriber is in a bad state.
        if let Some(dir) = resolve_log_dir() {
            let crash_path = dir.join("crashes.log");
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&crash_path) {
                let ts = chrono::Local::now().to_rfc3339();
                let _ = writeln!(f, "[{}] {}", ts, line);
                let _ = f.flush();
            }
        }
    }));
}

/// Resolve the log directory under the per-user config dir.
/// Returns `None` if the directory cannot be created (we fall back to
/// stderr-only logging in that case — never crash the app over logging).
///
/// Path layout:
///   Windows: %APPDATA%\moontranslator\logs\
///   Linux:   ~/.config/moontranslator/logs/
///   macOS:   ~/Library/Application Support/moontranslator/logs/
fn resolve_log_dir() -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    let dir = base.join("moontranslator").join("logs");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Build the per-day log file path: `app-YYYY-MM-DD.log`.
fn today_log_path(log_dir: &PathBuf) -> PathBuf {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    log_dir.join(format!("app-{}.log", date))
}

const LOG_ROTATE_BYTES: u64 = 10 * 1024 * 1024;

/// A `MakeWriter` implementation that:
///   1. Sanitizes log output to remove sensitive patterns (API keys, tokens)
///   2. Writes to stderr (ignoring broken-pipe errors common on Windows GUI)
///   3. Mirrors to a per-day log file under the user config dir, so crashes
///      leave evidence behind. Without this, all logs vanish with the
///      process because Tauri's GUI subsystem discards stderr.
struct SanitizingWriter {
    file: Option<Arc<Mutex<File>>>,
    path: Option<PathBuf>,
}

impl SanitizingWriter {
    fn new() -> Self {
        let (file, path) = match resolve_log_dir() {
            Some(dir) => {
                let p = today_log_path(&dir);
                // Pre-rotate: if today's file is already huge, archive it.
                if let Ok(meta) = std::fs::metadata(&p) {
                    if meta.len() > LOG_ROTATE_BYTES {
                        let mut old = p.clone();
                        old.set_extension("log.1");
                        let _ = std::fs::rename(&p, &old);
                    }
                }
                let f = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&p)
                    .ok();
                (f.map(|file| Arc::new(Mutex::new(file))), Some(p))
            },
            None => (None, None),
        };
        Self { file, path }
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SanitizingWriter {
    type Writer = SanitizedWriter;

    fn make_writer(&self) -> Self::Writer {
        SanitizedWriter {
            file: self.file.clone(),
            path: self.path.clone(),
        }
    }
}

struct SanitizedWriter {
    file: Option<Arc<Mutex<File>>>,
    path: Option<PathBuf>,
}

impl Write for SanitizedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let sanitized = moontranslator_lib::security::sanitize_log_message(&text);
        let bytes = sanitized.as_bytes();

        // Mirror to file first (best-effort) so we capture logs even when
        // stderr is broken. Rotation: when the file exceeds the threshold,
        // archive it to `.log.1` and reopen. We do this under the lock to
        // avoid racing with other threads.
        if let Some(ref arc) = self.file {
            if let Ok(mut f) = arc.lock() {
                let need_rotate = f
                    .metadata()
                    .map(|m| m.len() > LOG_ROTATE_BYTES)
                    .unwrap_or(false);
                if need_rotate {
                    if let Some(ref path) = self.path {
                        let _ = f.flush();
                        let mut old = path.clone();
                        old.set_extension("log.1");
                        if std::fs::rename(path, &old).is_ok() {
                            // Reopen a fresh handle at the original path.
                            if let Ok(new_f) = OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(path)
                            {
                                *f = new_f;
                            }
                        }
                    }
                }
                let _ = f.write_all(bytes);
                let _ = f.flush();
            }
        }

        let mut stderr = std::io::stderr();
        match stderr.write_all(bytes) {
            Ok(()) => Ok(buf.len()),
            Err(e) if e.raw_os_error() == Some(232) => Ok(buf.len()),
            Err(e) => Err(e),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(ref arc) = self.file {
            if let Ok(mut f) = arc.lock() {
                let _ = f.flush();
            }
        }
        match std::io::stderr().flush() {
            Ok(()) => Ok(()),
            Err(e) if e.raw_os_error() == Some(232) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

/// Recursively find the newest modification time under `dir` (skipping
/// unreadable entries). Returns `None` if `dir` doesn't exist.
fn newest_mtime(dir: &std::path::Path) -> Option<std::time::SystemTime> {
    fn walk(dir: &std::path::Path, best: &mut Option<std::time::SystemTime>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                walk(&entry.path(), best);
            } else if let Ok(metadata) = entry.metadata() {
                if let Ok(mtime) = metadata.modified() {
                    let newer = best
                        .map(|b: std::time::SystemTime| mtime > b)
                        .unwrap_or(true);
                    if newer {
                        *best = Some(mtime);
                    }
                }
            }
        }
    }
    let mut best: Option<std::time::SystemTime> = None;
    walk(dir, &mut best);
    best
}

/// Startup freshness check: warn loudly if the running binary predates newer
/// source files. This catches the "stale exe" failure mode where a fix lands in
/// source but is never compiled into the binary you launch — e.g. hovering
/// translate-card fixes that looked "dead" because the exe predated the edit.
///
/// Compares the exe build time against:
///   - newest `src-tauri/src/**/*.rs` mtime (debug + release)
///   - newest `dist/**` mtime (release only — debug may run the vite dev server)
fn check_binary_freshness() {
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn fmt_time(t: SystemTime) -> String {
        let dt: chrono::DateTime<chrono::Utc> = t.into();
        chrono::DateTime::<chrono::Local>::from(dt)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    }

    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return,
    };
    let exe_mtime = match std::fs::metadata(&exe).and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return,
    };

    // Find the project root by walking up from the exe until we see
    // `src-tauri/Cargo.toml`. Works for target/debug and target/release layouts;
    // silently skips when the source tree isn't present (installed/bundled exe).
    let mut root: Option<PathBuf> = None;
    let mut dir = exe.parent();
    for _ in 0..6 {
        let Some(d) = dir else { break };
        if d.join("src-tauri").join("Cargo.toml").is_file() {
            root = Some(d.to_path_buf());
            break;
        }
        dir = d.parent();
    }
    let Some(root) = root else {
        return;
    };

    let mut stale: Vec<String> = Vec::new();
    if let Some(rust_newest) = newest_mtime(&root.join("src-tauri").join("src")) {
        if exe_mtime < rust_newest {
            stale.push(format!(
                "Rust source changed at {} (after exe) — run `cargo build` / `pnpm tauri dev`",
                fmt_time(rust_newest)
            ));
        }
    }
    if !cfg!(debug_assertions) {
        if let Some(dist_newest) = newest_mtime(&root.join("dist")) {
            if exe_mtime < dist_newest {
                stale.push(format!(
                    "FE bundle dist/ changed at {} (after exe) — run `pnpm run build` then rebuild",
                    fmt_time(dist_newest)
                ));
            }
        }
    }

    if stale.is_empty() {
        tracing::info!(
            "[startup-check] binary is current (exe built {})",
            fmt_time(exe_mtime)
        );
    } else {
        tracing::warn!("[startup-check] STALE BINARY — you are running an outdated build (exe built {})", fmt_time(exe_mtime));
        for s in stale {
            tracing::warn!("[startup-check]   - {}", s);
        }
        tracing::warn!("[startup-check]   Fix: rebuild then relaunch. Changes are NOT live in this process.");
    }
}

fn main() {
    // Set custom panic hook before tracing is initialized
    set_panic_hook();

    // Tier4-6: One-shot OCR worker subprocess dispatch.
    // Parent spawns `current_exe() --ocr-worker [--lang <lang>]`, pipes PNG
    // bytes to stdin, reads JSON result from stdout. The child runs WinRT
    // OCR and exits — OS reclaims the ONNX model memory.
    //
    // Dispatch before tracing init: the child must not write the startup
    // banner to the parent's stdout (would corrupt the JSON wire format).
    // Only stderr is safe in the child.
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "--ocr-worker" {
        let lang = args.iter().position(|a| a == "--lang").and_then(|i| {
            args.get(i + 1).map(|s| s.clone())
        });
        let exit_code = moontranslator_lib::ocr_worker::run_worker(lang);
        std::process::exit(exit_code);
    }

    let filter = if cfg!(debug_assertions) {
        EnvFilter::new("debug")
    } else {
        // P0: allow selection/overlay/mouse_hook info logs in release so the
        // 划词/hover/OCR card pipeline is observable without a dev build.
        // Without this, the entire success path is silent in release builds.
        EnvFilter::new(
            "warn,moontranslator_lib::selection=info,moontranslator_lib::overlay=info,moontranslator_lib::commands::window=info",
        )
    };

    let writer = SanitizingWriter::new();
    // Note: we can't tracing::info! here because the subscriber is not yet
    // initialized. Emit a startup banner directly to the file so we can
    // confirm the file path is being used.
    if let Some(ref arc) = writer.file {
        if let Ok(mut f) = arc.lock() {
            let ts = chrono::Local::now().to_rfc3339();
            let _ = writeln!(
                f,
                "\n=== Moon Translator starting {} (pid={}) ===",
                ts,
                std::process::id()
            );
            if let Some(ref p) = writer.path {
                let _ = writeln!(f, "[main] log file: {}", p.display());
            }
            let _ = f.flush();
        }
    } else {
        eprintln!("[main] log file unavailable, stderr-only");
    }

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(writer)
        .init();

    // Fail loudly at startup if we're running a build that predates the source.
    check_binary_freshness();

    moontranslator_lib::run()
}
