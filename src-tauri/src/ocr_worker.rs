//! Tier4-6: One-shot subprocess OCR worker.
//!
//! Pattern (kivio `rapidocr.rs:192-230`): self-invoke via `current_exe()` with
//! `--ocr-worker` flag, OS reclaims ONNX address space when the child exits,
//! `worker_lock: Mutex<()>` serializes spawns.
//!
//! ## Why
//! - In-process `WinRT` OCR holds the ONNX model in our address space for the
//!   process lifetime. Long-running sessions accumulate heap fragmentation
//!   inside the ONNX runtime that is never returned to the OS.
//! - For occasional OCR users (one snip every few minutes), loading the model
//!   per-call is acceptable and the OS frees everything when the child exits.
//! - For heavy continuous OCR, the existing in-process path is faster —
//!   subprocess mode is opt-in via `winrt_ocr_use_subprocess`.
//!
//! ## Protocol
//! 1. Parent acquires `OCR_WORKER_LOCK` (serialize — prevents N children
//!    competing for the same `WinRT` OCR engine registry keys).
//! 2. Parent spawns `current_exe() --ocr-worker --lang <lang>`.
//! 3. Parent writes PNG bytes to child's stdin, then closes stdin.
//! 4. Child reads stdin, runs `run_winrt_ocr(bytes, lang)`, writes JSON to
//!    stdout, exits 0 on success / non-zero on error.
//! 5. Parent reads stdout, parses JSON, returns result.
//!
//! ## Failure modes
//! - Subprocess spawn failure → fall back to in-process `run_winrt_ocr`.
//! - Subprocess timeout (default 30s) → kill child, return error.
//! - Malformed stdout → return error, do not fall back (would mask bugs).

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Default subprocess timeout. `WinRT` OCR cold-start is 200–800 ms; 30 s is
/// generous enough for any reasonable image while preventing zombie children.
const DEFAULT_WORKER_TIMEOUT: Duration = Duration::from_secs(30);

/// Serializes subprocess OCR spawns. Without this, N concurrent OCR calls
/// would spawn N children, each loading the ONNX model — memory spikes and
/// `WinRT` OCR engine registry contention slows everything down.
static OCR_WORKER_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn worker_lock() -> &'static Mutex<()> {
    OCR_WORKER_LOCK.get_or_init(|| Mutex::new(()))
}

/// JSON wire format between parent and child. Kept minimal so the child can
/// emit it with a single `serde_json::to_string` + `println!`.
#[derive(Debug, Serialize, Deserialize)]
struct OcrWorkerResult {
    /// `true` when OCR succeeded and `text` is the recognized string.
    /// `false` when OCR succeeded but returned no text (empty result).
    /// Absent when OCR failed — see `error`.
    #[serde(skip_serializing_if = "Option::is_none")]
    ok: Option<bool>,
    /// Recognized text (only present when `ok == Some(true)`).
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    /// Error message (only present when `ok == None`).
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Run `WinRT` OCR via a one-shot subprocess.
///
/// Acquires `OCR_WORKER_LOCK`, spawns `current_exe() --ocr-worker --lang <lang>`,
/// pipes `png_bytes` to stdin, reads JSON result from stdout.
///
/// Returns `Ok(None)` when OCR succeeded but the image had no text.
/// Returns `Err(message)` when the subprocess failed, timed out, or produced
/// malformed output. **Callers should fall back to in-process `run_winrt_ocr`
/// only on spawn failure, not on OCR error** (the error is real).
pub fn run_winrt_ocr_via_subprocess(
    png_bytes: &[u8],
    lang: Option<&str>,
) -> Result<Option<String>, String> {
    // Serialize: prevents N children competing for WinRT OCR engine.
    // Hold the lock for the entire subprocess lifetime — release on drop.
    let _guard = worker_lock()
        .lock()
        .map_err(|e| format!("OCR worker lock poisoned: {e}"))?;

    let exe = std::env::current_exe()
        .map_err(|e| format!("current_exe failed: {e}"))?;

    let mut cmd = Command::new(&exe);
    cmd.arg("--ocr-worker");
    if let Some(lang) = lang {
        cmd.arg("--lang").arg(lang);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Detach from parent's console to avoid stealing focus on Windows.
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW (0x08000000): no console window for the child.
        cmd.creation_flags(0x0800_0000);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("OCR worker spawn failed: {e}"))?;

    // Write PNG to stdin. If the child exits early (e.g., bad lang), the
    // write will fail — capture the error but still try to read stdout.
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(png_bytes);
        // Drop stdin to signal EOF.
    }

    // Wait with timeout. `wait_with_output` blocks indefinitely and consumes
    // the child; use a thread + channel pattern to enforce the timeout and
    // move the child into the waiter thread.
    let child_id = child.id();
    let (tx, rx) = std::sync::mpsc::channel();
    let join = std::thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(DEFAULT_WORKER_TIMEOUT) {
        Ok(Ok(output)) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!(
                    "OCR worker exited with status {}: {}",
                    output.status,
                    stderr.trim()
                ));
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let parsed: OcrWorkerResult = serde_json::from_str(stdout.trim())
                .map_err(|e| format!("OCR worker JSON parse failed: {e} — stdout={stdout:?}"))?;
            match (parsed.ok, parsed.text, parsed.error) {
                (Some(true), Some(text), _) => Ok(Some(text)),
                (Some(false), _, _) => Ok(None),
                (_, _, Some(e)) => Err(e),
                _ => Err("OCR worker returned malformed result (all fields None)".into()),
            }
        }
        Ok(Err(e)) => {
            Err(format!("OCR worker wait_with_output failed: {e}"))
        }
        Err(_) => {
            // Timeout — kill the child by pid to avoid zombies. The waiter
            // thread is still blocked in `wait_with_output`; killing the
            // process unblocks it and the thread will exit cleanly.
            kill_process_by_pid(child_id);
            // Join the thread so it doesn't leak.
            let _ = join.join();
            Err(format!(
                "OCR worker timeout after {}s",
                DEFAULT_WORKER_TIMEOUT.as_secs()
            ))
        }
    }
}

/// Kill a process by PID. Used to enforce the subprocess OCR timeout.
///
/// SAFETY: `OpenProcess` + `TerminateProcess` are safe to call from any
/// thread. The handle is closed immediately after termination.
#[cfg(target_os = "windows")]
fn kill_process_by_pid(pid: u32) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
    // SAFETY: We pass a valid PID and request only PROCESS_TERMINATE access.
    // TerminateProcess is safe — it does not execute any code in the target
    // process. The handle is closed immediately.
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, pid) {
            let _ = TerminateProcess(handle, 1);
            let _ = CloseHandle(handle);
        }
    }
}

/// Kill a process by PID (non-Windows fallback).
#[cfg(not(target_os = "windows"))]
fn kill_process_by_pid(pid: u32) {
    use std::process::Command;
    let _ = Command::new("kill").arg("-9").arg(pid.to_string()).output();
}

/// Child-side entry point. Called from `main.rs` when `--ocr-worker` is the
/// first CLI arg.
///
/// Reads PNG bytes from stdin, runs `run_winrt_ocr`, writes JSON to stdout,
/// exits. Returns the process exit code (0 = success, 1 = OCR error, 2 = IO
/// error, 3 = argument error).
#[allow(clippy::needless_pass_by_value)]
pub fn run_worker(lang: Option<String>) -> i32 {
    let mut stdin = std::io::stdin();
    let mut bytes = Vec::new();
    if let Err(e) = stdin.read_to_end(&mut bytes) {
        eprintln!("OCR worker: stdin read failed: {e}");
        return 2;
    }
    if bytes.is_empty() {
        eprintln!("OCR worker: stdin was empty");
        return 2;
    }

    let result = crate::ocr_engine::run_winrt_ocr(&bytes, lang.as_deref());
    let wire = match result {
        Ok(Some(text)) => OcrWorkerResult {
            ok: Some(true),
            text: Some(text),
            error: None,
        },
        Ok(None) => OcrWorkerResult {
            ok: Some(false),
            text: None,
            error: None,
        },
        Err(e) => OcrWorkerResult {
            ok: None,
            text: None,
            error: Some(e),
        },
    };

    match serde_json::to_string(&wire) {
        Ok(json) => {
            // Write to stdout and flush — parent is blocked on read.
            let _ = std::io::stdout().write_all(json.as_bytes());
            let _ = std::io::stdout().write_all(b"\n");
            let _ = std::io::stdout().flush();
            i32::from(!(wire.ok == Some(true) || wire.ok == Some(false)))
        }
        Err(e) => {
            eprintln!("OCR worker: JSON serialize failed: {e}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocr_worker_result_serializes_empty() {
        let w = OcrWorkerResult {
            ok: Some(false),
            text: None,
            error: None,
        };
        let json = serde_json::to_string(&w).unwrap();
        assert!(json.contains("\"ok\":false"));
        assert!(!json.contains("text"));
    }

    #[test]
    fn ocr_worker_result_serializes_text() {
        let w = OcrWorkerResult {
            ok: Some(true),
            text: Some("hello".into()),
            error: None,
        };
        let json = serde_json::to_string(&w).unwrap();
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"text\":\"hello\""));
    }

    #[test]
    fn ocr_worker_result_serializes_error() {
        let w = OcrWorkerResult {
            ok: None,
            text: None,
            error: Some("boom".into()),
        };
        let json = serde_json::to_string(&w).unwrap();
        assert!(!json.contains("ok"));
        assert!(json.contains("\"error\":\"boom\""));
    }

    #[test]
    fn ocr_worker_result_roundtrip() {
        let original = OcrWorkerResult {
            ok: Some(true),
            text: Some("hello world".into()),
            error: None,
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: OcrWorkerResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.ok, Some(true));
        assert_eq!(parsed.text.as_deref(), Some("hello world"));
        assert!(parsed.error.is_none());
    }

    #[test]
    fn worker_lock_is_reentrant_safe() {
        // Verify the lock can be acquired and released without deadlock.
        // (Not actually reentrant — Mutex is not — but verify basic acquire.)
        {
            let _g1 = worker_lock().lock().unwrap();
        }
        {
            let _g2 = worker_lock().lock().unwrap();
        }
        // If we got here, no deadlock.
    }
}
