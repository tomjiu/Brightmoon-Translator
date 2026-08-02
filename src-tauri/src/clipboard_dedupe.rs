//! Shared clipboard text claim so main monitor + hook clipboard do not double-fire.
//! Also suppresses monitors during synthetic Ctrl+C / paste used by selection + replace.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Process-wide lock serializing all Win32 OpenClipboard sections.
///
/// The clipboard is a single OS-wide resource: a write (replace paste) while a
/// monitor thread holds the clipboard open makes the second OpenClipboard fail
/// or read a half-updated buffer (M4-03). All clipboard-touching call sites
/// (replace, hook clipboard listener, selection Ctrl+C) must take this lock for
/// the duration of their OpenClipboard..CloseClipboard window.
pub fn clipboard_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

static LAST: Mutex<Option<(String, Instant)>> = Mutex::new(None);
static SYNTHETIC_DEPTH: AtomicU64 = AtomicU64::new(0);
static SYNTHETIC_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Minimum interval to re-emit the exact same text (covers dual listeners + paste echo).
const SAME_TEXT_COOLDOWN: Duration = Duration::from_millis(800);

/// Begin a synthetic clipboard mutation (selection Ctrl+C, replace paste). Nested-safe.
pub fn begin_synthetic_clipboard() {
    SYNTHETIC_DEPTH.fetch_add(1, Ordering::SeqCst);
    SYNTHETIC_ACTIVE.store(true, Ordering::SeqCst);
}

/// End a synthetic clipboard mutation.
pub fn end_synthetic_clipboard() {
    let prev = SYNTHETIC_DEPTH.fetch_sub(1, Ordering::SeqCst);
    if prev <= 1 {
        SYNTHETIC_DEPTH.store(0, Ordering::SeqCst);
        SYNTHETIC_ACTIVE.store(false, Ordering::SeqCst);
    }
}

/// True while selection/replace is mutating the clipboard.
pub fn is_synthetic_clipboard() -> bool {
    SYNTHETIC_ACTIVE.load(Ordering::SeqCst) || SYNTHETIC_DEPTH.load(Ordering::SeqCst) > 0
}

/// Returns true if this process should act on `text` (first claimant within cooldown wins).
/// Synthetic clipboard windows always lose so monitors do not translate selection copies.
pub fn claim_clipboard_text(text: &str) -> bool {
    if is_synthetic_clipboard() {
        return false;
    }

    let trimmed = text.trim();
    if trimmed.len() < 2 {
        return false;
    }

    let mut guard = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    if let Some((ref last, at)) = *guard {
        if last == trimmed && now.duration_since(at) < SAME_TEXT_COOLDOWN {
            return false;
        }
    }
    *guard = Some((trimmed.to_string(), now));
    true
}

/// Record text as already claimed (e.g. after selection read) so monitors skip it.
pub fn mark_clipboard_text(text: &str) {
    let trimmed = text.trim();
    if trimmed.len() < 2 {
        return;
    }
    let mut guard = LAST.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some((trimmed.to_string(), Instant::now()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_claim_wins() {
        let t = format!("dedupe-unique-{}", uuid::Uuid::new_v4());
        assert!(claim_clipboard_text(&t));
        assert!(!claim_clipboard_text(&t));
    }

    #[test]
    fn synthetic_blocks_claim() {
        let t = format!("synth-unique-{}", uuid::Uuid::new_v4());
        begin_synthetic_clipboard();
        assert!(!claim_clipboard_text(&t));
        end_synthetic_clipboard();
        assert!(claim_clipboard_text(&t));
    }
}
