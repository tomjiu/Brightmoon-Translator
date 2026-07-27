pub mod app_detector;
#[cfg(target_os = "windows")]
pub mod windows;

pub use app_detector::WindowsTargetAppDetector;

#[cfg(target_os = "windows")]
pub use windows::{deliver_replacement_text, replace_text_via_clipboard, type_text_via_sendinput};

#[cfg(not(target_os = "windows"))]
pub fn replace_text_via_clipboard(_text: &str) -> Result<(), String> {
    Err("replace_text_via_clipboard is only supported on Windows".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn type_text_via_sendinput(
    _text: &str,
    _cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    Err("type_text_via_sendinput is only supported on Windows".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn deliver_replacement_text(
    text: &str,
    use_clipboard_output: bool,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    if use_clipboard_output {
        replace_text_via_clipboard(text)
    } else {
        type_text_via_sendinput(text, cancel)
    }
}
