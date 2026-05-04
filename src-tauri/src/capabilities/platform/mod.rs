pub mod app_detector;
#[cfg(target_os = "windows")]
pub mod windows;

pub use app_detector::WindowsTargetAppDetector;

#[cfg(target_os = "windows")]
pub use windows::replace_text_via_clipboard;
