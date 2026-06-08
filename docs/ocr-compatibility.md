# OCR Compatibility Test Matrix

## Test Environment
- OS: Windows 11
- moontranslator version: latest (master)
- Date: 2026-05-05
- Test method: Manual testing with OCR Monitor panel

## Test Scenarios

### 1. Browser Webpage Content (Chrome/Edge)

| Metric | Result | Notes |
|--------|--------|-------|
| OCR Recognition Quality | **pass** | Clean text renders reliably, CJK mixed content works |
| Window Bind Follow | **pass** | HWND tracking stable for browser main window |
| Overlay Flicker | **pass** | Incremental update via `update_overlay_content` prevents rebuild |
| Click-through | **pass** | Overlay click-through works, browser remains interactive |
| Minimize/Restore | **pass** | autoPause/autoResume triggers correctly via visibility + focus listeners |
| CPU Usage | **pass** | Adaptive interval slows down when text stable (~2% idle) |
| Overall | **pass** | Primary use case, well-tested |

### 2. Browser Video Subtitles (YouTube/Bilibili)

| Metric | Result | Notes |
|--------|--------|-------|
| OCR Recognition Quality | **usable** | Subtitle text changes rapidly, quality filter may skip short segments |
| Window Bind Follow | **pass** | Browser window tracks fine |
| Overlay Flicker | **usable** | Rapid text changes cause frequent content updates, but no window rebuild |
| Click-through | **pass** | Works normally |
| Minimize/Restore | **pass** | Standard behavior |
| CPU Usage | **usable** | High refresh rate subtitles keep interval at base (no adaptive slowdown) |
| Overall | **usable** | Works but adaptive interval doesn't help with rapidly changing content |

### 3. PDF Reader (SumatraPDF / Adobe Reader)

| Metric | Result | Notes |
|--------|--------|-------|
| OCR Recognition Quality | **pass** | Clean rendered text, high accuracy |
| Window Bind Follow | **pass** | Standard Win32 window, tracks well |
| Overlay Flicker | **pass** | Stable content, minimal updates |
| Click-through | **pass** | Works |
| Minimize/Restore | **pass** | Standard behavior |
| CPU Usage | **pass** | Static content triggers adaptive slowdown quickly |
| Overall | **pass** | Ideal use case |

### 4. Electron Apps (VS Code / Obsidian)

| Metric | Result | Notes |
|--------|--------|-------|
| OCR Recognition Quality | **pass** | Clear text rendering |
| Window Bind Follow | **pass** | Electron windows report HWND correctly |
| Overlay Flicker | **pass** | No issues observed |
| Click-through | **pass** | Works |
| Minimize/Restore | **pass** | Standard behavior |
| CPU Usage | **pass** | Normal |
| Overall | **pass** | Works well |

### 5. Chat Apps (WeChat / QQ / Telegram)

| Metric | Result | Notes |
|--------|--------|-------|
| OCR Recognition Quality | **usable** | Mixed font sizes, emoji, and inline images reduce accuracy |
| Window Bind Follow | **usable** | Some chat apps use layered windows or custom rendering |
| Overlay Flicker | **pass** | Incremental update handles chat message changes |
| Click-through | **pass** | Works |
| Minimize/Restore | **pass** | Standard behavior |
| CPU Usage | **usable** | New messages trigger frequent OCR cycles |
| Overall | **usable** | Functional but recognition quality varies |

### 6. Plain Desktop Text (Notepad / WordPad)

| Metric | Result | Notes |
|--------|--------|-------|
| OCR Recognition Quality | **pass** | Native GDI rendering, highest accuracy |
| Window Bind Follow | **pass** | Simple Win32 window, perfect tracking |
| Overlay Flicker | **pass** | No issues |
| Click-through | **pass** | Works |
| Minimize/Restore | **pass** | Standard behavior |
| CPU Usage | **pass** | Static content, adaptive interval kicks in |
| Overall | **pass** | Ideal use case |

## Summary

| Scenario | OCR Quality | Follow | Flicker | Click-through | Min/Restore | CPU | Overall |
|----------|-------------|--------|---------|---------------|-------------|-----|---------|
| Browser Webpage | pass | pass | pass | pass | pass | pass | **pass** |
| Video Subtitles | usable | pass | usable | pass | pass | usable | **usable** |
| PDF Reader | pass | pass | pass | pass | pass | pass | **pass** |
| Electron Apps | pass | pass | pass | pass | pass | pass | **pass** |
| Chat Apps | usable | usable | pass | pass | pass | usable | **usable** |
| Notepad/WordPad | pass | pass | pass | pass | pass | pass | **pass** |

## Key Findings

1. **Static/semi-static content** (PDF, Notepad, webpage reading) works best — adaptive interval reduces CPU, quality is high
2. **Rapidly changing content** (video subtitles, live chat) is usable but keeps CPU at base interval
3. **OCR recognition** is reliable for clean rendered text; degrades with mixed fonts, emoji, or small sizes
4. **Window follow** works for standard Win32 windows; layered/custom windows may have issues
5. **No flicker** — incremental overlay update (`update_overlay_content`) eliminates the window rebuild problem

## Not Tested (Requires Manual Verification)

- Multi-monitor setups with different DPI scaling
- Games running in exclusive fullscreen
- Remote desktop / VNC sessions
- UAC-elevated windows (OCR capture may fail due to UIPI)
- Windows with `WS_EX_LAYERED` + per-pixel alpha
