use super::{SelectionBounds, SelectionProvider, SelectionResult};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Accessibility::{
    IUIAutomation, IUIAutomationTextPattern, CUIAutomation,
    UIA_TextPatternId, UIA_ValuePatternId,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows::core::Interface;

// SAFEARRAY helpers for reading GetBoundingRectangles output
#[cfg(target_os = "windows")]
extern "system" {
    fn SafeArrayGetUBound(psa: *mut std::ffi::c_void, nDim: u32, plUbound: *mut i32) -> i32;
    fn SafeArrayGetElement(psa: *mut std::ffi::c_void, rgIndices: *const i32, pv: *mut std::ffi::c_void) -> i32;
}

/// Uses Windows UI Automation to read selected text from the focused control.
/// Falls back gracefully when the focused element doesn't support text patterns.
pub struct UiAutomationSelectionProvider;

#[async_trait::async_trait]
impl SelectionProvider for UiAutomationSelectionProvider {
    async fn get_selection(&self) -> Option<SelectionResult> {
        // UIA calls are blocking, run on a dedicated thread
        tokio::task::spawn_blocking(|| get_uia_selection()).await.ok()?
    }

    fn name(&self) -> &'static str {
        "uiautomation"
    }

    fn priority(&self) -> u32 {
        10 // high priority - try first
    }
}

fn get_uia_selection() -> Option<SelectionResult> {
    unsafe {
        // Initialize COM on this thread
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        // Create UIAutomation instance
        let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL).ok()?;

        // Get the focused element
        let element = automation.GetFocusedElement().ok()?;

        // Get window title
        let hwnd = GetForegroundWindow();
        let window_title = get_window_title(hwnd);

        // Get app name from element's class name or window title
        let source_app = {
            let class_name = element.CurrentClassName()
                .ok()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if class_name.is_empty() {
                detect_app_from_title(&window_title)
            } else {
                class_name
            }
        };

        // Try to get selected text via TextPattern, then ValuePattern
        let (text, bounds) = try_text_pattern(&element)
            .or_else(|_| try_value_pattern(&element))
            .ok()?;

        if text.trim().is_empty() {
            return None;
        }

        Some(SelectionResult {
            text: text.trim().to_string(),
            source_app,
            window_title,
            bounds,
            confidence: 0.95,
            provider: "uiautomation",
        })
    }
}

/// Try to read selected text via the TextPattern (rich text controls, browsers, etc.)
unsafe fn try_text_pattern(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> Result<(String, Option<SelectionBounds>), Box<dyn std::error::Error>> {
    let text_pattern_obj = element.GetCurrentPattern(UIA_TextPatternId)?;
    let text_pattern: IUIAutomationTextPattern = text_pattern_obj.cast()?;

    // GetSelection returns IUIAutomationTextRangeArray
    let ranges = text_pattern.GetSelection()?;
    let count = ranges.Length()?;
    if count == 0 {
        return Err("No text selection".into());
    }

    let range = ranges.GetElement(0)?;
    let text = range.GetText(-1)?;
    let text_str = text.to_string();

    // GetBoundingRectangles returns *mut SAFEARRAY of doubles (x,y,w,h per rect)
    let bounds = range.GetBoundingRectangles().ok().and_then(|rects_ptr| {
        if rects_ptr.is_null() {
            return None;
        }
        let mut upper: i32 = -1;
        SafeArrayGetUBound(rects_ptr as *mut std::ffi::c_void, 1, &mut upper);
        if upper < 3 {
            return None;
        }
        let mut r = [0.0f64; 4];
        for i in 0..4i32 {
            SafeArrayGetElement(
                rects_ptr as *mut std::ffi::c_void,
                &i as *const i32,
                &mut r[i as usize] as *mut f64 as *mut std::ffi::c_void,
            );
        }
        Some(SelectionBounds {
            x: r[0],
            y: r[1],
            width: r[2],
            height: r[3],
        })
    });

    Ok((text_str, bounds))
}

/// Fallback: try to read text via the ValuePattern (simple input fields)
unsafe fn try_value_pattern(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> Result<(String, Option<SelectionBounds>), Box<dyn std::error::Error>> {
    let pattern_obj = element.GetCurrentPattern(UIA_ValuePatternId)?;

    use windows::Win32::UI::Accessibility::IUIAutomationValuePattern;
    let value_pattern: IUIAutomationValuePattern = pattern_obj.cast()?;
    let value = value_pattern.CurrentValue()?;
    let text = value.to_string();

    let bounds = element
        .CurrentBoundingRectangle()
        .ok()
        .map(|rect| SelectionBounds {
            x: rect.left as f64,
            y: rect.top as f64,
            width: (rect.right - rect.left) as f64,
            height: (rect.bottom - rect.top) as f64,
        });

    Ok((text, bounds))
}

/// Get window title from HWND
unsafe fn get_window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(hwnd, &mut buf);
    if len > 0 {
        String::from_utf16_lossy(&buf[..len as usize])
    } else {
        String::new()
    }
}

/// Extract a rough app name from the window title
fn detect_app_from_title(title: &str) -> String {
    if let Some(pos) = title.rfind(" - ") {
        let app = &title[pos + 3..];
        if !app.is_empty() {
            return app.to_string();
        }
    }
    title.to_string()
}
