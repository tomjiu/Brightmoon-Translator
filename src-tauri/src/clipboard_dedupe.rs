//! Shared clipboard text claim so main monitor + hook clipboard do not double-fire.

use std::sync::Mutex;
use std::time::{Duration, Instant};

static LAST: Mutex<Option<(String, Instant)>> = Mutex::new(None);

/// Minimum interval to re-emit the exact same text (covers dual listeners + paste echo).
const SAME_TEXT_COOLDOWN: Duration = Duration::from_millis(800);

/// Returns true if this process should act on `text` (first claimant within cooldown wins).
pub fn claim_clipboard_text(text: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_claim_wins() {
        let t = format!("dedupe-unique-{}", uuid::Uuid::new_v4());
        assert!(claim_clipboard_text(&t));
        assert!(!claim_clipboard_text(&t));
    }
}
