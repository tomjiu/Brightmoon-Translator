use super::{SelectionBounds, SelectionProvider, SelectionResult};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Accessibility::{
    IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern, CUIAutomation,
    UIA_TextPatternId,
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
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.is_err() {
            log::error!("[uiautomation] CoInitializeEx failed: {:?}", hr);
            return None;
        }

        // Create UIAutomation instance
        let automation: IUIAutomation = match CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL) {
            Ok(a) => a,
            Err(e) => {
                log::error!("[uiautomation] CoCreateInstance failed: {}", e);
                return None;
            }
        };

        // Get the focused element
        let element = match automation.GetFocusedElement() {
            Ok(e) => e,
            Err(e) => {
                log::warn!("[uiautomation] GetFocusedElement failed: {}", e);
                return None;
            }
        };

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

        // Try patterns in order: TextPattern -> ValuePattern with selection -> ValuePattern full -> children
        let (text, bounds) = match try_text_pattern(&element) {
            Ok(result) => {
                log::info!("[uiautomation] TextPattern success: {} chars", result.0.len());
                result
            }
            Err(e) => {
                log::debug!("[uiautomation] TextPattern failed: {}", e);
                match try_value_pattern_with_selection(&element, &automation) {
                    Ok(result) => {
                        log::info!("[uiautomation] ValuePattern+selection success: {} chars", result.0.len());
                        result
                    }
                    Err(e2) => {
                        log::debug!("[uiautomation] ValuePattern+selection failed: {}", e2);
                        match try_value_pattern_full(&element) {
                            Ok(result) => {
                                log::info!("[uiautomation] ValuePattern(full) success: {} chars", result.0.len());
                                result
                            }
                            Err(e3) => {
                                log::debug!("[uiautomation] ValuePattern(full) failed: {}", e3);
                                match find_text_in_children(&element, &automation, 0) {
                                    Some(result) => {
                                        log::info!("[uiautomation] Children walk success: {} chars", result.0.len());
                                        result
                                    }
                                    None => {
                                        log::debug!("[uiautomation] All patterns exhausted for focused element");
                                        return None;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };

        if text.trim().is_empty() {
            log::debug!("[uiautomation] Got text but it's empty after trim");
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
/// Concatenates all selected ranges and merges their bounds.
unsafe fn try_text_pattern(
    element: &IUIAutomationElement,
) -> Result<(String, Option<SelectionBounds>), Box<dyn std::error::Error>> {
    let text_pattern_obj = element.GetCurrentPattern(UIA_TextPatternId)?;
    let text_pattern: IUIAutomationTextPattern = text_pattern_obj.cast()?;

    // GetSelection returns IUIAutomationTextRangeArray
    let ranges = text_pattern.GetSelection()?;
    let count = ranges.Length()?;
    if count == 0 {
        return Err("No text selection".into());
    }

    // Concatenate all selected ranges and merge bounds
    let mut all_text = String::new();
    let mut merged_bounds: Option<SelectionBounds> = None;

    for i in 0..count {
        let range = ranges.GetElement(i)?;
        let text = range.GetText(-1)?;
        let text_str = text.to_string();
        if !text_str.is_empty() {
            all_text.push_str(&text_str);
        }

        // Merge bounding rectangles
        if let Ok(rects_ptr) = range.GetBoundingRectangles() {
            if !rects_ptr.is_null() {
                let mut upper: i32 = -1;
                SafeArrayGetUBound(rects_ptr as *mut std::ffi::c_void, 1, &mut upper);
                if upper >= 3 {
                    let mut r = [0.0f64; 4];
                    for j in 0..4i32 {
                        SafeArrayGetElement(
                            rects_ptr as *mut std::ffi::c_void,
                            &j as *const i32,
                            &mut r[j as usize] as *mut f64 as *mut std::ffi::c_void,
                        );
                    }
                    let rect = SelectionBounds {
                        x: r[0],
                        y: r[1],
                        width: r[2],
                        height: r[3],
                    };
                    merged_bounds = Some(match merged_bounds {
                        Some(existing) => merge_bounds(&existing, &rect),
                        None => rect,
                    });
                }
            }
        }
    }

    if all_text.is_empty() {
        return Err("All ranges empty".into());
    }

    Ok((all_text, merged_bounds))
}

/// Try ValuePattern to get full value, then cross-reference with TextPattern
/// to extract the selected portion.
unsafe fn try_value_pattern_with_selection(
    element: &IUIAutomationElement,
    _automation: &IUIAutomation,
) -> Result<(String, Option<SelectionBounds>), Box<dyn std::error::Error>> {
    use windows::Win32::UI::Accessibility::{IUIAutomationValuePattern, UIA_ValuePatternId};

    let pattern_obj = element.GetCurrentPattern(UIA_ValuePatternId)?;
    let value_pattern: IUIAutomationValuePattern = pattern_obj.cast()?;
    let value = value_pattern.CurrentValue()?;
    let full_text = value.to_string();

    if full_text.is_empty() {
        return Err("ValuePattern empty".into());
    }

    // Try to get TextPattern selection from the same element to find selected portion
    if let Ok(text_pattern_obj) = element.GetCurrentPattern(UIA_TextPatternId) {
        if let Ok(text_pattern) = text_pattern_obj.cast::<IUIAutomationTextPattern>() {
            if let Ok(ranges) = text_pattern.GetSelection() {
                if let Ok(count) = ranges.Length() {
                    if count > 0 {
                        // Build selected text by concatenating ranges
                        let mut selected = String::new();
                        for i in 0..count {
                            if let Ok(range) = ranges.GetElement(i) {
                                if let Ok(t) = range.GetText(-1) {
                                    selected.push_str(&t.to_string());
                                }
                            }
                        }
                        if !selected.is_empty() && full_text.contains(&selected) {
                            // Found the selected portion within the full value
                            let bounds = element
                                .CurrentBoundingRectangle()
                                .ok()
                                .map(|rect| SelectionBounds {
                                    x: rect.left as f64,
                                    y: rect.top as f64,
                                    width: (rect.right - rect.left) as f64,
                                    height: (rect.bottom - rect.top) as f64,
                                });
                            return Ok((selected, bounds));
                        }
                    }
                }
            }
        }
    }

    // TextPattern cross-reference didn't confirm a real selection.
    // Only full value is available — that's not a selection success.
    log::debug!("[uiautomation] ValuePattern: only full value available ({} chars), no confirmed selection — falling through", full_text.len());
    Err("ValuePattern: no confirmed selection, only full text available".into())
}

/// Pure ValuePattern fallback — returns the full value (no selection info).
unsafe fn try_value_pattern_full(
    element: &IUIAutomationElement,
) -> Result<(String, Option<SelectionBounds>), Box<dyn std::error::Error>> {
    use windows::Win32::UI::Accessibility::{IUIAutomationValuePattern, UIA_ValuePatternId};

    let pattern_obj = element.GetCurrentPattern(UIA_ValuePatternId)?;
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

/// Walk the UIA tree to find a child (or descendant) that supports TextPattern.
/// Max depth: 5, max children per level: 10.
unsafe fn find_text_in_children(
    element: &IUIAutomationElement,
    automation: &IUIAutomation,
    depth: u32,
) -> Option<(String, Option<SelectionBounds>)> {
    if depth >= 5 {
        return None;
    }

    let true_cond = match automation.CreateTrueCondition() {
        Ok(c) => c,
        Err(e) => {
            log::debug!("[uiautomation] CreateTrueCondition failed: {}", e);
            return None;
        }
    };

    let children = match element.FindAll(
        windows::Win32::UI::Accessibility::TreeScope_Children,
        &true_cond,
    ) {
        Ok(c) => c,
        Err(e) => {
            log::debug!("[uiautomation] FindAll children failed at depth {}: {}", depth, e);
            return None;
        }
    };

    let count = children.Length().unwrap_or(0);
    let limit = count.min(10); // max 10 children per level

    for i in 0..limit {
        let child = match children.GetElement(i) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Try TextPattern on this child
        if let Ok(result) = try_text_pattern(&child) {
            if !result.0.trim().is_empty() {
                return Some(result);
            }
        }

        // Try ValuePattern+selection on this child
        if let Ok(result) = try_value_pattern_with_selection(&child, automation) {
            if !result.0.trim().is_empty() {
                return Some(result);
            }
        }

        // Try ValuePattern full on this child
        if let Ok(result) = try_value_pattern_full(&child) {
            if !result.0.trim().is_empty() {
                return Some(result);
            }
        }

        // Recurse into grandchildren
        if let Some(result) = find_text_in_children(&child, automation, depth + 1) {
            return Some(result);
        }
    }

    None
}

/// Merge two bounding rectangles into the smallest rectangle that contains both.
fn merge_bounds(a: &SelectionBounds, b: &SelectionBounds) -> SelectionBounds {
    let left = a.x.min(b.x);
    let top = a.y.min(b.y);
    let right = (a.x + a.width).max(b.x + b.width);
    let bottom = (a.y + a.height).max(b.y + b.height);
    SelectionBounds {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
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
