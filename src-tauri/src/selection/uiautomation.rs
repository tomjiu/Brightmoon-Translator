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
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.is_err() {
            log::error!("[UIA] COM init failed: {:?}", hr);
            return None;
        }

        // Create UIAutomation instance
        let automation: IUIAutomation = match CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL) {
            Ok(a) => a,
            Err(e) => {
                log::error!("[UIA] CoCreateInstance failed: {:?}", e);
                return None;
            }
        };

        // Get the focused element
        let element = match automation.GetFocusedElement() {
            Ok(e) => e,
            Err(e) => {
                log::error!("[UIA] GetFocusedElement failed: {:?}", e);
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

        // Try patterns on the focused element first
        let result = try_text_pattern(&element)
            .or_else(|_| try_value_pattern_with_selection(&element))
            .or_else(|_| try_value_pattern_full(&element));

        let (text, bounds) = match result {
            Ok(r) => r,
            Err(_) => {
                // Focused element doesn't support text patterns — walk children
                log::debug!("[UIA] Focused element has no text patterns, walking children...");
                match find_text_in_children(&automation, &element, 0) {
                    Some(r) => r,
                    None => {
                        log::debug!("[UIA] No text found in focused element or children");
                        return None;
                    }
                }
            }
        };

        if text.trim().is_empty() {
            log::debug!("[UIA] Got text but it's empty/whitespace");
            return None;
        }

        log::info!("[UIA] Success: {} chars from '{}'", text.trim().len(), source_app);

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
/// Supports multi-range selections by concatenating all ranges.
unsafe fn try_text_pattern(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> Result<(String, Option<SelectionBounds>), Box<dyn std::error::Error>> {
    let text_pattern_obj = element.GetCurrentPattern(UIA_TextPatternId).map_err(|e| {
        log::debug!("[UIA] TextPattern not available: {:?}", e);
        e
    })?;
    let text_pattern: IUIAutomationTextPattern = text_pattern_obj.cast()?;

    // GetSelection returns IUIAutomationTextRangeArray
    let ranges = text_pattern.GetSelection()?;
    let count = ranges.Length()?;
    if count == 0 {
        return Err("No text selection (0 ranges)".into());
    }

    // Concatenate all selected ranges
    let mut all_text = String::new();
    let mut merged_bounds: Option<SelectionBounds> = None;

    for i in 0..count {
        if let Ok(range) = ranges.GetElement(i) {
            if let Ok(text) = range.GetText(-1) {
                let s = text.to_string();
                if !s.is_empty() {
                    if !all_text.is_empty() {
                        all_text.push(' ');
                    }
                    all_text.push_str(&s);
                }
            }

            // Merge bounds: expand to cover all ranges
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
                        let b = SelectionBounds {
                            x: r[0],
                            y: r[1],
                            width: r[2],
                            height: r[3],
                        };
                        merged_bounds = Some(match merged_bounds {
                            Some(existing) => merge_bounds(&existing, &b),
                            None => b,
                        });
                    }
                }
            }
        }
    }

    if all_text.is_empty() {
        return Err("All ranges empty".into());
    }

    Ok((all_text, merged_bounds))
}

/// Try ValuePattern, then attempt to extract selected portion via TextPattern.
/// This handles the case where a control exposes ValuePattern (full text) but
/// the user has only selected a portion.
unsafe fn try_value_pattern_with_selection(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> Result<(String, Option<SelectionBounds>), Box<dyn std::error::Error>> {
    // First get the full value
    let pattern_obj = element.GetCurrentPattern(UIA_ValuePatternId)?;
    use windows::Win32::UI::Accessibility::IUIAutomationValuePattern;
    let value_pattern: IUIAutomationValuePattern = pattern_obj.cast()?;
    let full_value = value_pattern.CurrentValue()?.to_string();

    // Then try to get the selected portion via TextPattern on the same element
    if let Ok(text_pattern_obj) = element.GetCurrentPattern(UIA_TextPatternId) {
        if let Ok(text_pattern) = text_pattern_obj.cast::<IUIAutomationTextPattern>() {
            if let Ok(ranges) = text_pattern.GetSelection() {
                if let Ok(count) = ranges.Length() {
                    if count > 0 {
                        if let Ok(range) = ranges.GetElement(0) {
                            if let Ok(selected_text) = range.GetText(-1) {
                                let sel = selected_text.to_string();
                                if !sel.is_empty() && full_value.contains(&sel) {
                                    log::debug!("[UIA] ValuePattern: extracted selection '{}' from full value", sel);
                                    let bounds = element.CurrentBoundingRectangle().ok().map(|rect| SelectionBounds {
                                        x: rect.left as f64,
                                        y: rect.top as f64,
                                        width: (rect.right - rect.left) as f64,
                                        height: (rect.bottom - rect.top) as f64,
                                    });
                                    return Ok((sel, bounds));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // No selection available — return full value
    let bounds = element.CurrentBoundingRectangle().ok().map(|rect| SelectionBounds {
        x: rect.left as f64,
        y: rect.top as f64,
        width: (rect.right - rect.left) as f64,
        height: (rect.bottom - rect.top) as f64,
    });

    Ok((full_value, bounds))
}

/// Fallback: try to read full text via ValuePattern (no selection extraction)
unsafe fn try_value_pattern_full(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> Result<(String, Option<SelectionBounds>), Box<dyn std::error::Error>> {
    let pattern_obj = element.GetCurrentPattern(UIA_ValuePatternId).map_err(|e| {
        log::debug!("[UIA] ValuePattern not available: {:?}", e);
        e
    })?;

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

/// Walk the UIA tree to find a child element that supports text patterns.
/// Limits depth to 5 and checks max 10 children per level.
unsafe fn find_text_in_children(
    automation: &IUIAutomation,
    parent: &windows::Win32::UI::Accessibility::IUIAutomationElement,
    depth: u32,
) -> Option<(String, Option<SelectionBounds>)> {
    if depth >= 5 {
        return None;
    }

    let true_cond = automation.CreateTrueCondition().ok()?;
    let tree_walker = match automation.CreateTreeWalker(&true_cond) {
        Ok(w) => w,
        Err(_) => return None,
    };

    let mut child = match tree_walker.GetFirstChildElement(parent) {
        Ok(c) => c,
        Err(_) => return None,
    };

    let mut checked = 0u32;
    loop {
        if checked >= 10 {
            break;
        }
        checked += 1;

        // Try text patterns on this child
        if let Ok((text, bounds)) = try_text_pattern(&child) {
            if !text.trim().is_empty() {
                return Some((text, bounds));
            }
        }
        if let Ok((text, bounds)) = try_value_pattern_with_selection(&child) {
            if !text.trim().is_empty() {
                return Some((text, bounds));
            }
        }
        if let Ok((text, bounds)) = try_value_pattern_full(&child) {
            if !text.trim().is_empty() {
                return Some((text, bounds));
            }
        }

        // Recurse into children
        if let Some(result) = find_text_in_children(automation, &child, depth + 1) {
            return Some(result);
        }

        // Next sibling
        match tree_walker.GetNextSiblingElement(&child) {
            Ok(next) => child = next,
            Err(_) => break,
        }
    }

    None
}

/// Merge two bounds rectangles into the smallest bounding rectangle that covers both.
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
