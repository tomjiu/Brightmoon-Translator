# OCR MVP Status

## Default OCR Path

The default OCR entry is now a screenshot-translation MVP, not the old continuous monitor.

Current call chain:

1. Main OCR page calls `prepare_screenshot_snapshot`.
2. Rust captures the primary screen and saves a temp PNG snapshot.
3. A fullscreen `ocr-screenshot` webview opens and displays that frozen snapshot.
4. User drags a region; the selector emits image-pixel coordinates back to `main`.
5. Main page calls `crop_screenshot_snapshot`.
6. OCR runs through `system_ocr` first on Windows, then falls back to frontend `tesseract.js`.
7. Translation uses the existing Rust `translate` command.
8. Result stays visible in the OCR page with source text, translated text, copy, pin, clear, and refresh.

## What Changed

- Added a true frozen-screenshot selection flow inspired by Pot Desktop's screenshot window model.
- Added Rust snapshot/crop commands so large screenshot data does not need to be passed through window URLs.
- Added Windows native OCR command registration and frontend fallback to `tesseract.js`.
- Added "refresh same region" by recapturing the primary screen and cropping the previous image-pixel region.
- Kept the old `OcrMonitor` continuous mode under an experimental details section.
- Fixed the old monitor follow-loop bug where the next cycle reused a stale region instead of `regionRef.current`.
- Added explicit `[OCR]` engine logging in `ocrImagePreferNative` to distinguish WinRT vs tesseract.js.

## Known Limits

- Current MVP captures the primary screen only. Multi-monitor support still needs explicit monitor selection.
- Refresh reuses image-pixel coordinates from the primary screen. It is suitable for a stable window/region, not moved windows.
- The result is persistent inside the main OCR page, not yet a separate floating result window.
- Windows OCR language is currently `auto`; explicit language package mapping still needs refinement.

## Verification Results

### Build & Test (automated)

| Command | Result |
|---------|--------|
| `tsc --noEmit` | PASS |
| `npm test` | PASS (1 test) |
| `npm run build` | PASS |
| `cargo check` | PASS |
| `cargo test` | PASS (8 tests) |

### GUI Acceptance (P0-1)

**环境限制：当前为无头/远程桌面环境，屏幕输出全黑，无法做真实点击验证。以下为代码分析结果，需要人工补验。**

| Step | 预期 | 代码分析 | 需人工验证 |
|------|------|----------|-----------|
| 打开 App → OCR 页 | 显示截图翻译 MVP 卡片 | ✅ App.tsx 渲染 OcrScreenshotTranslator | ❌ |
| 点击"开始截图翻译" | 主窗口隐藏 | ✅ appWindow.hide() 被调用 | ❌ |
| 截图窗口出现 | 全屏显示冻结截图 | ✅ prepare_screenshot_snapshot + WebviewWindow fullscreen | ❌ |
| 截图窗口显示冻结画面 | 不是黑屏/白屏 | ✅ loadScreenshotSnapshot 加载已保存 PNG | ❌ |
| 拖选区域 | 出现选区框 | ✅ mouseDown/mouseMove 处理 | ❌ |
| 松开鼠标 | selector emit 事件后关闭 | ✅ emitTo("main", "ocr-screenshot-selected") + close() | ❌ |
| 主窗口收到事件 | 开始 OCR | ✅ listen("ocr-screenshot-selected") → runOcr | ❌ |
| OCR 完成 | 原文显示 | ✅ ocrImagePreferNative 返回文本 | ❌ |
| 翻译完成 | 译文显示 | ✅ invoke("translate") → response.results | ❌ |
| 主窗口恢复 | 可见 | ✅ getCurrentWindow().show() 在 runOcr 的 try/catch 中 | ❌ |
| Esc 取消 | 主窗口恢复 | ✅ emitTo("main", "ocr-screenshot-cancelled") → show | ❌ |

### Refresh Same Region (P0-2)

| Step | 代码分析 | 需人工验证 |
|------|----------|-----------|
| 点击"刷新同一区域" | ✅ runOcr(result.region, true) 调用 captureScreenshotRegion | ❌ |
| 重新截图 | ✅ captureScreenshotRegion 截取同一物理坐标 | ❌ |
| 重新 OCR | ✅ ocrImagePreferNative 重新调用 | ❌ |
| 重新翻译 | ✅ invoke("translate") 重新调用 | ❌ |
| 结果更新 | ✅ setResult 覆盖旧结果 | ❌ |

### Windows Native OCR (P0-3)

| Item | Status |
|------|--------|
| system_ocr 命令已注册 | ✅ lib.rs 已注册 |
| WinRT API 调用链完整 | ✅ StorageFile → BitmapDecoder → SoftwareBitmap → OcrEngine → Recognize |
| 错误不吞掉 | ✅ 每步都有 .map_err |
| 引擎日志区分 | ✅ ocrImagePreferNative 已加 [OCR] Engine: 日志 |
| WinRT 实际是否被调用 | ❌ 需人工验证（看控制台 [OCR] Engine: 行） |

### Multi-Monitor / DPI (P1-5)

| 场景 | 代码分析 | 需人工验证 |
|------|----------|-----------|
| 单显示器 100% | ✅ 应正常工作 | ❌ |
| 单显示器 125%/150% | ✅ selector 用 naturalWidth/imageRect.width 做 DPI 换算 | ❌ |
| 双显示器 | ⚠️ 只截主屏，选区坐标限于主屏 | ❌ |

### Result Window UX (P1-6)

| Item | 代码分析 | 需人工验证 |
|------|----------|-----------|
| OCR 原文复制 | ✅ copyText(result.sourceText) | ❌ |
| 翻译复制 | ✅ copyText(primaryText(result.translations)) | ❌ |
| 清空结果 | ✅ setResult(null), setStatus("idle") | ❌ |
| 置顶按钮 | ✅ setAlwaysOnTop 切换 | ❌ |
| 长文本显示 | ✅ whitespace-pre-wrap, overflow-auto | ❌ |
| 空 OCR 错误提示 | ✅ "OCR 没有识别到文本" error | ❌ |

## P2: Next Steps

1. **独立浮动结果窗口**：暂不需要。当前结果嵌入主页面，刷新/复制方便。如果用户需要同时看原文和翻译，再考虑独立窗口。
2. **区域绑定窗口**：暂不需要。MVP 场景是"截一次翻一次"，不需要持续跟随。
3. **多显示器正式支持**：需要。当前只截主屏，双屏用户无法截取第二屏。方案：传入显示器坐标给 `prepare_screenshot_snapshot`。
4. **OCR 预处理**：暂不需要。WinRT OCR 对中文/英文识别质量足够。如果识别率不够再加灰度化/二值化。
5. **替换 tesseract fallback**：暂不需要。tesseract.js 作为 fallback 可以覆盖 WinRT 不可用的场景。
