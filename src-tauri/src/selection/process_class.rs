//! Foreground process classification for selection strategy (Easydict-inspired).
//! Electron → clipboard first; terminal → never Ctrl+C; others → UIA then clipboard.

use std::path::Path;

/// How to order UIA vs clipboard for the current foreground app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionStrategy {
    /// UIA first, then Ctrl+C clipboard (default desktop apps)
    UiaThenClipboard,
    /// Clipboard first (Electron/Chromium — TextPattern flaky)
    ClipboardThenUia,
    /// UIA only — Ctrl+C would SIGINT or kill the session
    UiaOnly,
    /// Skip selection entirely (our own process or excluded)
    Skip,
}

/// Snapshot of the foreground process used for routing.
#[derive(Debug, Clone)]
pub struct ForegroundProcess {
    pub process_name: String,
    pub process_id: u32,
    pub is_electron: bool,
    /// Chrome/Edge/Firefox etc. — same clipboard-first strategy as Electron
    pub is_browser: bool,
    pub is_terminal: bool,
    pub is_self: bool,
}

impl ForegroundProcess {
    pub fn strategy(&self, exclude: &[String]) -> SelectionStrategy {
        if self.is_self {
            return SelectionStrategy::Skip;
        }
        let n = normalize_process_name(&self.process_name);
        if exclude.iter().any(|e| normalize_process_name(e) == n) {
            return SelectionStrategy::Skip;
        }
        if self.is_terminal {
            // Terminals: UIA only. A synthetic Ctrl+C would be delivered to the
            // foreground terminal window (Windows Terminal / cmd / opencode TUI)
            // and SIGINT the running session — killing `tauri dev`, npm, or the
            // user's shell. Windows Terminal exposes the selection via UIA
            // TextPattern; when UIA can't read it we give up rather than risk
            // the foreground session.
            return SelectionStrategy::UiaOnly;
        }
        if self.is_electron || self.is_browser {
            return SelectionStrategy::ClipboardThenUia;
        }
        SelectionStrategy::UiaThenClipboard
    }
}

/// Strip path, extension, trailing version-ish suffixes for matching.
pub fn normalize_process_name(raw: &str) -> String {
    let file = Path::new(raw.trim())
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(raw.trim());
    let lower = file.to_ascii_lowercase();
    // drop trailing " - insiders" style already handled by stem; collapse spaces
    lower.replace(' ', "")
}

/// Apps where UIA TextPattern is flaky → prefer Ctrl+C clipboard first.
const CLIPBOARD_FIRST_NAMES: &[&str] = &[
    // Electron / Chromium shells
    "code",
    "code-insiders",
    "cursor",
    "slack",
    "discord",
    "teams",
    "ms-teams",
    "notion",
    "obsidian",
    "postman",
    "figma",
    "spotify",
    "whatsapp",
    "signal",
    "telegram",
    "telegramdesktop",
    "atom",
    "typora",
    "gitkraken",
    "insomnia",
    "beekeeper-studio",
    "1password",
    // Browsers (page text rarely exposes TextPattern selection)
    "chrome",
    "msedge",
    "msedgewebview2",
    "firefox",
    "brave",
    "opera",
    "vivaldi",
    "chromium",
    "arc",
];

const TERMINAL_NAMES: &[&str] = &[
    "windowsterminal",
    "windowsterminal.exe",
    "cmd",
    "powershell",
    "pwsh",
    "conhost",
    "mintty",
    "alacritty",
    "wezterm",
    "wezterm-gui",
    "hyper",
    "terminus",
    "wsl",
    "wslhost",
    "ubuntu",
    "debian",
    "mobaxterm",
    "putty",
    "kitty",
    "xshell",
    "securecrt",
    "tabby",
    "conemu",
    "conemu64",
    "cmder",
    "fluentterminal",
    "termius",
    "mremoteng",
    "teraterm",
    "ttermpro",
    "openssh",
    "ssh",
];

fn is_clipboard_first_name(norm: &str) -> bool {
    CLIPBOARD_FIRST_NAMES.iter().any(|e| {
        let e = e.replace(' ', "");
        norm == e || norm.starts_with(&format!("{e}-")) || norm.starts_with(&format!("{e}."))
    }) || norm.contains("electron")
}

fn is_browser_name(norm: &str) -> bool {
    matches!(
        norm,
        "chrome"
            | "msedge"
            | "msedgewebview2"
            | "firefox"
            | "brave"
            | "opera"
            | "vivaldi"
            | "chromium"
            | "arc"
    ) || norm.starts_with("chrome")
        || norm.starts_with("msedge")
        || norm.starts_with("firefox")
}

fn is_terminal_name(norm: &str) -> bool {
    TERMINAL_NAMES.iter().any(|t| {
        let t = t.replace(' ', "").replace(".exe", "");
        norm == t || norm.starts_with(&format!("{t}-"))
    })
}

/// Read foreground HWND → process name / pid. Returns None if unavailable.
pub fn foreground_process() -> Option<ForegroundProcess> {
    #[cfg(windows)]
    {
        foreground_process_win()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
fn foreground_process_win() -> Option<ForegroundProcess> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    // SAFETY: GetForegroundWindow returns HWND (or null, checked below).
    // OpenProcess handle is closed via CloseHandle on every path; pid is a
    // stack &mut u32. QueryFullProcessImageNameW writes into a stack buffer.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid: u32 = 0;
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }

        let self_pid = std::process::id();
        let is_self = pid == self_pid;

        let mut process_name = String::new();
        if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut buf = [0u16; 512];
            let mut size = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(buf.as_mut_ptr()),
                &mut size,
            )
            .is_ok();
            if ok && size > 0 {
                let path = String::from_utf16_lossy(&buf[..size as usize]);
                process_name = Path::new(&path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&path)
                    .to_string();
            }
            let _ = CloseHandle(handle);
        }

        if process_name.is_empty() {
            process_name = format!("pid-{pid}");
        }

        let norm = normalize_process_name(&process_name);
        let is_browser = is_browser_name(&norm);
        Some(ForegroundProcess {
            is_electron: is_clipboard_first_name(&norm) && !is_browser,
            is_browser,
            is_terminal: is_terminal_name(&norm),
            is_self,
            process_name,
            process_id: pid,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_apps() {
        assert!(is_clipboard_first_name("code"));
        assert!(is_clipboard_first_name("chrome"));
        assert!(is_browser_name("msedge"));
        assert!(is_terminal_name("windowsterminal"));
        assert!(is_terminal_name("pwsh"));
        assert!(!is_terminal_name("notepad"));
    }

    #[test]
    fn strategy_order() {
        let term = ForegroundProcess {
            process_name: "WindowsTerminal".into(),
            process_id: 1,
            is_electron: false,
            is_browser: false,
            is_terminal: true,
            is_self: false,
        };
        assert_eq!(term.strategy(&[]), SelectionStrategy::UiaOnly);

        let elec = ForegroundProcess {
            process_name: "Code".into(),
            process_id: 2,
            is_electron: true,
            is_browser: false,
            is_terminal: false,
            is_self: false,
        };
        assert_eq!(elec.strategy(&[]), SelectionStrategy::ClipboardThenUia);

        let browser = ForegroundProcess {
            process_name: "chrome".into(),
            process_id: 3,
            is_electron: false,
            is_browser: true,
            is_terminal: false,
            is_self: false,
        };
        assert_eq!(browser.strategy(&[]), SelectionStrategy::ClipboardThenUia);

        assert_eq!(
            ForegroundProcess {
                is_self: true,
                ..elec.clone()
            }
            .strategy(&[]),
            SelectionStrategy::Skip
        );
        assert_eq!(elec.strategy(&["code".into()]), SelectionStrategy::Skip);
    }
}
