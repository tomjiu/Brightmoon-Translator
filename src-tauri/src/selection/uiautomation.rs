use super::{SelectionBounds, SelectionProvider, SelectionResult};
use std::sync::LazyLock;
use tokio::sync::Semaphore;
use windows::core::Interface;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern, UIA_TextPatternId,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

// SAFEARRAY helpers for reading GetBoundingRectangles output
#[cfg(target_os = "windows")]
extern "system" {
    fn SafeArrayGetUBound(psa: *mut std::ffi::c_void, nDim: u32, plUbound: *mut i32) -> i32;
    fn SafeArrayGetElement(
        psa: *mut std::ffi::c_void,
        rgIndices: *const i32,
        pv: *mut std::ffi::c_void,
    ) -> i32;
}

/// Read the first (merged) bounding rect out of a text range's SAFEARRAY.
/// Shared by the selection provider and hover pick so both place cards near
/// the *word*, not the whole control element.
#[cfg(target_os = "windows")]
pub(crate) fn text_range_bounds(
    range: &windows::Win32::UI::Accessibility::IUIAutomationTextRange,
) -> Option<SelectionBounds> {
    // SAFETY: GetBoundingRectangles returns a SAFEARRAY of doubles; parsing
    // reads only the first 4 elements (x, y, w, h) after checking UB >= 3.
    unsafe {
        let rects_ptr = range.GetBoundingRectangles().ok()?;
        if rects_ptr.is_null() {
            return None;
        }
        let mut upper: i32 = -1;
        SafeArrayGetUBound(rects_ptr.cast::<std::ffi::c_void>(), 1, &raw mut upper);
        if upper < 3 {
            return None;
        }
        let mut r = [0.0f64; 4];
        for j in 0..4i32 {
            SafeArrayGetElement(
                rects_ptr.cast::<std::ffi::c_void>(),
                &raw const j,
                (&raw mut r[j as usize]).cast::<std::ffi::c_void>(),
            );
        }
        Some(SelectionBounds {
            x: r[0],
            y: r[1],
            width: r[2],
            height: r[3],
        })
    }
}

/// Uses Windows UI Automation to read selected text from the focused control.
/// Falls back gracefully when the focused element doesn't support text patterns.
pub struct UiAutomationSelectionProvider;

/// Easydict `_automationSemaphore` (`SemaphoreSlim` 1,1) + UiaSemaphoreTimeoutMs=200.
/// Serializes UIA calls so concurrent selection requests (hotkey + `auto_select` + hover)
/// don't contend on the focused element / COM apartment. If busy past the timeout we skip
/// rather than piling up — matches Easydict's "Wait then give up" behavior.
/// P1: 200ms was too short and dropped words under concurrent hotkey + `auto_select` — now 500ms.
static UIA_SEMAPHORE: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(1));

#[async_trait::async_trait]
impl SelectionProvider for UiAutomationSelectionProvider {
    async fn get_selection(&self) -> Option<SelectionResult> {
        // Easydict: acquire the UIA semaphore (500ms budget) before doing any UIA work,
        // so two in-flight selection requests can't race on GetFocusedElement / COM.
        // P1: 200ms was too short — concurrent hotkey+auto_select dropped words.
        let _permit = match tokio::time::timeout(
            std::time::Duration::from_millis(500),
            UIA_SEMAPHORE.acquire(),
        )
        .await
        {
            Ok(Ok(p)) => p,
            Ok(Err(_)) => {
                tracing::warn!("[uiautomation] semaphore closed — skip");
                return None;
            },
            Err(_) => {
                tracing::warn!(
                    "[uiautomation] semaphore busy after 500ms — skip (UIA serialized)"
                );
                return None;
            },
        };

        // Easydict: UIA execution timeout 800ms — never hang selection path
        match tokio::time::timeout(
            std::time::Duration::from_millis(800),
            tokio::task::spawn_blocking(get_uia_selection),
        )
        .await
        {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                tracing::warn!("[uiautomation] join error: {e}");
                None
            },
            Err(_) => {
                tracing::warn!("[uiautomation] timed out after 800ms");
                None
            },
        }
    }

    fn name(&self) -> &'static str {
        "uiautomation"
    }

    fn priority(&self) -> u32 {
        10 // high priority - try first
    }
}

/// Get selected text via UI Automation.
/// SAFETY: COM and UI Automation API calls. All COM objects are reference-counted.
fn get_uia_selection() -> Option<SelectionResult> {
    unsafe {
        // Initialize COM on this thread.
        // S2-12: RPC_E_CHANGED_MODE (0x80010106) means COM was already
        // initialized on this thread with a different threading model —
        // common when UIA is called from a thread that already hosts an
        // STA component. This is NOT fatal: the thread already has COM,
        // so we proceed and let UIA use the existing apartment.
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        // S_FALSE (1) = already initialized same mode — also OK.
        const RPC_E_CHANGED_MODE: windows::core::HRESULT = windows::core::HRESULT(0x80010106u32 as i32);
        if hr.is_err() && hr != RPC_E_CHANGED_MODE {
            tracing::error!("[uiautomation] CoInitializeEx failed: {:?}", hr);
            return None;
        }

        // Create UIAutomation instance
        let automation: IUIAutomation = match CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL) {
            Ok(a) => a,
            Err(e) => {
                tracing::error!("[uiautomation] CoCreateInstance failed: {}", e);
                return None;
            },
        };

        // Get the focused element
        let element = match automation.GetFocusedElement() {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("[uiautomation] GetFocusedElement failed: {}", e);
                return None;
            },
        };

        // Get window title
        let hwnd = GetForegroundWindow();
        let window_title = get_window_title(hwnd);

        // Get app name from element's class name or window title
        let source_app = {
            let class_name = element
                .CurrentClassName()
                .ok()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if class_name.is_empty() {
                detect_app_from_title(&window_title)
            } else {
                class_name
            }
        };

        // Easydict-style: ONLY real selection ranges — never full ValuePattern text.
        // Full-value "selection" was the main source of wrong-word / whole-document bugs.
        let (text, bounds) = match try_text_pattern(&element) {
            Ok(result) => {
                tracing::info!(
                    "[uiautomation] TextPattern selection: {} chars",
                    result.0.len()
                );
                result
            },
            Err(e) => {
                tracing::debug!("[uiautomation] TextPattern failed: {}", e);
                match try_value_pattern_with_selection(&element, &automation) {
                    Ok(result) => {
                        tracing::info!(
                            "[uiautomation] Value+TextPattern selection: {} chars",
                            result.0.len()
                        );
                        result
                    },
                    Err(e2) => {
                        tracing::debug!(
                            "[uiautomation] no confirmed selection (Value+Text failed: {})",
                            e2
                        );
                        // Children: TextPattern selection only (no full value)
                        if let Some(result) = find_text_selection_in_children(&element, &automation, 0) {
                            tracing::info!(
                                "[uiautomation] child TextPattern selection: {} chars",
                                result.0.len()
                            );
                            result
                        } else {
                            tracing::debug!(
                                "[uiautomation] no selection — fall through to clipboard"
                            );
                            return None;
                        }
                    },
                }
            },
        };

        if text.trim().is_empty() {
            tracing::debug!("[uiautomation] Got text but it's empty after trim");
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

/// Try to read selected text via the `TextPattern` (rich text controls, browsers, etc.)
/// Concatenates all selected ranges and merges their bounds.
/// Try to read selected text via `TextPattern`.
/// SAFETY: UI Automation COM interface calls.
///
/// S1-7: this is intentionally a SEPARATE function from
/// `hover_pick::try_text_pattern_at_point`. Both touch UIA `TextPattern` but
/// serve different purposes and use different UIA APIs:
///   - this fn reads the user's *current selection* via `GetSelection()`
///     and concatenates all selected ranges (used by selection translation)
///   - `hover_pick` fn reads the word/sentence *under the cursor* via
///     `RangeFromPoint(pt)` + `ExpandToEnclosingUnit` (used by hover dict)
///
/// The only shared logic is `range.GetText(max_chars).to_string()` — a
/// one-liner not worth a shared module. `GetBoundingRectangles` (the
/// non-trivial SAFEARRAY parsing below) is only called here, not in
/// `hover_pick`, so there is no real duplication to extract.
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
        // S2-11: cap GetText at 4096 chars per range. Previously -1 requested
        // the entire document text, which for large controls (Word, browser
        // pages) returned megabytes and blocked the 800ms selection timeout.
        // 4096 matches the hover_pick.rs cap and is well above any realistic
        // single-selection length.
        let text = range.GetText(4096)?;
        let text_str = text.to_string();
        if !text_str.is_empty() {
            all_text.push_str(&text_str);
        }

        // Merge bounding rectangles
        if let Some(rect) = text_range_bounds(&range) {
            merged_bounds = Some(match merged_bounds {
                Some(existing) => merge_bounds(&existing, &rect),
                None => rect,
            });
        }
    }

    if all_text.is_empty() {
        return Err("All ranges empty".into());
    }

    Ok((all_text, merged_bounds))
}

/// Try `ValuePattern` to get full value, then cross-reference with `TextPattern`
/// to extract the selected portion.
/// Try `ValuePattern` with `TextPattern` cross-reference.
/// SAFETY: UI Automation COM interface calls.
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
                            let bounds = element.CurrentBoundingRectangle().ok().map(|rect| {
                                SelectionBounds {
                                    x: f64::from(rect.left),
                                    y: f64::from(rect.top),
                                    width: f64::from(rect.right - rect.left),
                                    height: f64::from(rect.bottom - rect.top),
                                }
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
    tracing::debug!("[uiautomation] ValuePattern: only full value available ({} chars), no confirmed selection — falling through", full_text.len());
    Err("ValuePattern: no confirmed selection, only full text available".into())
}

/// Full `ValuePattern` — not used for selection (would return whole document).
/// SAFETY: Read-only UI Automation COM calls on a borrowed element.
/// COM pointers from `GetCurrentPattern` are reference-counted.
#[allow(dead_code)]
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
            x: f64::from(rect.left),
            y: f64::from(rect.top),
            width: f64::from(rect.right - rect.left),
            height: f64::from(rect.bottom - rect.top),
        });

    Ok((text, bounds))
}

/// Walk children for `TextPattern` **selection** only (Easydict — no full Value).
/// Max depth 3, max 8 children (bounded for 800ms timeout).
/// SAFETY: UI Automation COM calls on borrowed elements; depth/width are
/// bounded so the walk always terminates. COM pointers are reference-counted.
unsafe fn find_text_selection_in_children(
    element: &IUIAutomationElement,
    automation: &IUIAutomation,
    depth: u32,
) -> Option<(String, Option<SelectionBounds>)> {
    if depth >= 3 {
        return None;
    }

    let true_cond = automation.CreateTrueCondition().ok()?;
    let children = element
        .FindAll(
            windows::Win32::UI::Accessibility::TreeScope_Children,
            &true_cond,
        )
        .ok()?;

    let count = children.Length().unwrap_or(0);
    let limit = count.min(8);

    for i in 0..limit {
        let child = match children.GetElement(i) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Ok(result) = try_text_pattern(&child) {
            if !result.0.trim().is_empty() {
                return Some(result);
            }
        }
        if let Ok(result) = try_value_pattern_with_selection(&child, automation) {
            if !result.0.trim().is_empty() {
                return Some(result);
            }
        }
        if let Some(result) = find_text_selection_in_children(&child, automation, depth + 1) {
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
/// Get window title from HWND.
/// SAFETY: `GetWindowTextW` is a standard Win32 API.
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
