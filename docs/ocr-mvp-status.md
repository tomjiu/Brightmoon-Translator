# OCR Screen Translation Status

## Current Architecture

The OCR path uses a "region frame + result overlay" model:

1. User clicks "开始截图翻译" on the OCR page.
2. Main window hides; a fullscreen frozen-screenshot selector opens.
3. User drags a region on the screenshot.
4. Selector closes; main window reappears.
5. A **transparent region frame window** (`ocr-region-frame`) is created at the selection's screen position.
6. The region frame is **draggable** (grab anywhere except buttons) and **resizable** (bottom-right handle).
7. OCR + translate runs on the region; results appear in the **existing overlay** (`update_overlay`) positioned next to the frame.
8. Region frame has controls: **Refresh OCR**, **Start/Pause continuous refresh (2s)**, **Close**.
9. Dragging the frame repositions the overlay. Resizing the frame triggers re-OCR.
10. Closing the frame closes the overlay and restores the main window.

## Call Chain

```
startScreenshotTranslate()
  → hide main window
  → prepare_screenshot_snapshot() [Rust: capture full screen → temp PNG]
  → new WebviewWindow("ocr-screenshot") [fullscreen selector]
  → user drags region → emits ocr-screenshot-selected
  → OcrScreenshotTranslator receives event
  → create_ocr_region_frame(x, y, w, h) [Rust: transparent borderless window]
  → captureScreenshotRegion(region) [Rust: re-capture screen → crop]
  → ocrImagePreferNative(image) [WinRT → tesseract.js fallback]
  → invoke("translate", { text }) [Rust translation service]
  → invoke("update_overlay", { x, y, w, h, text, source }) [show result overlay]
```

## Key Files

| File | Role |
|------|------|
| `src/components/OcrScreenshotSelector.tsx` | Fullscreen screenshot selection window |
| `src/components/OcrRegionFrame.tsx` | Draggable/resizable OCR region frame window |
| `src/components/OcrScreenshotTranslator.tsx` | Main orchestrator (in main window) |
| `src/services/ocr.ts` | OCR service (capture, crop, WinRT, tesseract.js) |
| `src/services/ocrOverlayPosition.ts` | Overlay position calculation |
| `src-tauri/src/commands/capture.rs` | Screenshot capture, crop, system_ocr commands |
| `src-tauri/src/commands/window.rs` | create_ocr_region_frame, close_ocr_region_frame, update_overlay |

## Feature Status

| Feature | Status | Notes |
|---------|--------|-------|
| 全屏截图选区 | 代码已实现 | OcrScreenshotSelector |
| 区域框可拖动 | 代码已实现 | OcrRegionFrame custom drag |
| 区域框可调整大小 | 代码已实现 | Bottom-right resize handle |
| 区域框控制按钮 | 代码已实现 | Refresh / Play+Pause / Close |
| 持续刷新 (2s) | 代码已实现 | setInterval in OcrScreenshotTranslator |
| 结果 overlay 原位显示 | 代码已实现 | update_overlay positioned next to frame |
| Windows 原生 OCR | 代码已实现 | WinRT Windows.Media.Ocr |
| tesseract.js fallback | 代码已实现 | ocrImagePreferNative |
| DPI 100% | 代码已实现 | Selector uses naturalWidth/imageRect.width |
| DPI 125%/150% | 代码已实现，未验证 | Same DPI logic as selector |
| 多显示器 | 未支持 | 只截主屏，不崩溃但不支持第二屏 |
| GUI 验证 | 未完成 | 当前环境无头/远程桌面 |

## P0 Risks (need real GUI verification)

| # | Risk | Severity | How to verify |
|---|------|----------|---------------|
| R1 | Region frame window not visible (transparent issue) | Block | After selection, is the blue border frame visible on screen? |
| R2 | Region frame drag doesn't work | High | Can you grab and move the frame? |
| R3 | Region frame resize doesn't work | High | Does the bottom-right handle resize the frame? |
| R4 | Overlay positioned wrong | High | Is the result overlay next to the region frame? |
| R5 | Continuous refresh doesn't stop | Medium | Click pause — does the 2s cycle stop? |
| R6 | WinRT OCR not actually invoked | High | Console shows `[OCR] Engine: Windows.Media.Ocr`? |
| R7 | Main window not restored on close | Block | Close region frame — does main window reappear? |

## Automated Verification

| Command | Result | Date |
|---------|--------|------|
| `tsc --noEmit` | PASS | 2026-05-06 |
| `npm test` | PASS (3 tests) | 2026-05-06 |
| `npm run build` | PASS | 2026-05-06 |
| `cargo check` | PASS | 2026-05-06 |
| `cargo test` | PASS | 2026-05-06 |

## Manual Verification Checklist

```
环境：Windows ___, 分辨率 ___, DPI ___%
App 版本：git commit _____

[ ] 1. 打开 App → OCR 页 → 看到"屏幕 OCR 翻译"卡片
[ ] 2. 点击"开始截图翻译" → 主窗口消失
[ ] 3. 全屏截图窗口出现，显示真实桌面画面
[ ] 4. 拖选文字区域 → 出现蓝色选区框
[ ] 5. 松开鼠标 → 截图窗口关闭
[ ] 6. 屏幕原位出现蓝色边框的区域框（可拖动、有控制按钮）
[ ] 7. 区域框旁边出现翻译结果浮窗
[ ] 8. 拖动区域框 → 浮窗跟随移动
[ ] 9. 调整区域框大小 → 自动重新 OCR + 翻译
[ ] 10. 点击区域框刷新按钮 → 重新 OCR + 翻译
[ ] 11. 点击播放按钮 → 持续刷新模式，每 2 秒更新
[ ] 12. 点击暂停按钮 → 停止持续刷新
[ ] 13. 点击关闭按钮 → 区域框和浮窗关闭，主窗口恢复
[ ] 14. 控制台出现 [OCR] Engine: Windows.Media.Ocr 或 tesseract.js
[ ] 15. DPI 125%/150% 下拖选文字，OCR 结果对应选区内容
```

## Known Limits

- Only captures primary screen. Multi-monitor not supported (won't crash, but can't select on second screen).
- Region frame resize only has bottom-right handle (no top-left, no edge handles).
- Overlay uses the existing generic overlay system; no dedicated OCR result overlay with copy/refresh/close per-result.
- Windows OCR language is `auto`; no explicit language pack mapping.
- Continuous refresh interval is hardcoded 2 seconds.
