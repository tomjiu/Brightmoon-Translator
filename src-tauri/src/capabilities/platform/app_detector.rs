use async_trait::async_trait;

use super::super::adapters::{AppContext, TargetAppDetector};

/// Windows implementation of TargetAppDetector.
/// Uses Win32 APIs to detect the foreground application,
/// including process name, window title, PID, and embedded app type.
pub struct WindowsTargetAppDetector;

impl WindowsTargetAppDetector {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TargetAppDetector for WindowsTargetAppDetector {
    async fn detect(&self) -> Option<AppContext> {
        let info = super::windows::detect_foreground_app()?;

        let embedded_type = super::windows::classify_embedded_app(&info.app_name, &info.window_class);

        Some(AppContext {
            app_name: info.app_name,
            window_title: info.window_title,
            pid: info.pid,
            is_embedded: embedded_type.is_some(),
            embedded_type,
        })
    }
}
