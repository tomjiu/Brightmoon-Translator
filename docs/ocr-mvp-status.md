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

| Command | Result | Date |
|---------|--------|------|
| `tsc --noEmit` | PASS | 2026-05-06 |
| `npm test` | PASS (1 test) | 2026-05-06 |
| `cargo check` | PASS | 2026-05-06 |

**当前 commit**: `6c420c6` (fix(ocr): use regionRef.current in scheduleNext to avoid stale region)

**脏文件（非 OCR 相关）**: README.md, extension/README.md, extension/build.js, extension/manifest.json, src-tauri/src/models/translation.rs, docs/project-triage.md

### 结论

**代码分析未发现阻塞 bug。真实 GUI 阻塞 bug 未知。**

当前环境为无头/远程桌面，屏幕输出全黑，GetForegroundWindow 返回 null。无法做真实点击验证。以下所有"代码分析"项均**未经验真**，不能算验收通过。

### P0 风险清单（需人工验证）

| # | 风险 | 严重度 | 验证方法 |
|---|------|--------|----------|
| R1 | WebviewWindow fullscreen 权限被拒 | 阻塞 | 点击"开始截图翻译"后是否出现全屏窗口 |
| R2 | 主窗口 hide 后无法 restore | 阻塞 | OCR 完成/Esc 后主窗口是否恢复可见 |
| R3 | WinRT OCR 实际未被调用（fallback 到 tesseract.js） | 高 | 控制台是否出现 `[OCR] Engine: Windows.Media.Ocr` |
| R4 | 全屏截图窗口显示黑屏/白屏 | 阻塞 | 截图窗口是否显示真实桌面画面 |
| R5 | DPI 125%/150% 下选区坐标偏移 | 高 | 高 DPI 下拖选文字，OCR 结果是否对应选区内容 |
| R6 | crop_screenshot_snapshot 越界 panic | 高 | 选区超出截图边界时是否崩溃 |

### GUI Acceptance (P0-1) — 未验证

**所有项均为代码分析推断，无真实 GUI 验证证据。需要人工补验。**

| Step | 预期 | 代码分析推断 | 真实验证 | 证据 |
|------|------|-------------|---------|------|
| 打开 App → OCR 页 | 显示截图翻译 MVP 卡片 | App.tsx 渲染 OcrScreenshotTranslator | 未验证 | — |
| 点击"开始截图翻译" | 主窗口隐藏 | appWindow.hide() 被调用 | 未验证 | — |
| 截图窗口出现 | 全屏显示冻结截图 | prepare_screenshot_snapshot + WebviewWindow fullscreen | 未验证 | — |
| 截图窗口显示冻结画面 | 不是黑屏/白屏 | loadScreenshotSnapshot 加载已保存 PNG | 未验证 | — |
| 拖选区域 | 出现选区框 | mouseDown/mouseMove 处理 | 未验证 | — |
| 松开鼠标 | selector emit 事件后关闭 | emitTo("main", "ocr-screenshot-selected") + close() | 未验证 | — |
| 主窗口收到事件 | 开始 OCR | listen("ocr-screenshot-selected") → runOcr | 未验证 | — |
| OCR 完成 | 原文显示 | ocrImagePreferNative 返回文本 | 未验证 | — |
| 翻译完成 | 译文显示 | invoke("translate") → response.results | 未验证 | — |
| 主窗口恢复 | 可见 | getCurrentWindow().show() 在 runOcr 的 try/catch 中 | 未验证 | — |
| Esc 取消 | 主窗口恢复 | emitTo("main", "ocr-screenshot-cancelled") → show | 未验证 | — |

### Refresh Same Region (P0-2) — 未验证

| Step | 代码分析推断 | 真实验证 | 证据 |
|------|-------------|---------|------|
| 点击"刷新同一区域" | runOcr(result.region, true) 调用 captureScreenshotRegion | 未验证 | — |
| 重新截图 | captureScreenshotRegion 截取同一物理坐标 | 未验证 | — |
| 重新 OCR | ocrImagePreferNative 重新调用 | 未验证 | — |
| 重新翻译 | invoke("translate") 重新调用 | 未验证 | — |
| 结果更新 | setResult 覆盖旧结果 | 未验证 | — |

### Windows Native OCR (P0-3) — 未验证

| Item | 代码分析推断 | 真实验证 | 证据 |
|------|-------------|---------|------|
| system_ocr 命令已注册 | lib.rs 已注册 | 未验证 | — |
| WinRT API 调用链完整 | StorageFile → BitmapDecoder → SoftwareBitmap → OcrEngine → Recognize | 未验证 | — |
| 错误不吞掉 | 每步都有 .map_err | 未验证 | — |
| 引擎日志区分 | ocrImagePreferNative 已加 [OCR] Engine: 日志 | 未验证 | — |
| WinRT 实际是否被调用 | 未知 | 未验证 | 需看控制台 `[OCR] Engine:` 行 |

### Multi-Monitor / DPI (P1-5) — 未验证

| 场景 | 代码分析推断 | 真实验证 | 证据 |
|------|-------------|---------|------|
| 单显示器 100% | 应正常工作 | 未验证 | — |
| 单显示器 125%/150% | selector 用 naturalWidth/imageRect.width 做 DPI 换算 | 未验证 | — |
| 双显示器 | 只截主屏，选区坐标限于主屏 | 未验证 | — |

### Result Window UX (P1-6) — 未验证

| Item | 代码分析推断 | 真实验证 | 证据 |
|------|-------------|---------|------|
| OCR 原文复制 | copyText(result.sourceText) | 未验证 | — |
| 翻译复制 | copyText(primaryText(result.translations)) | 未验证 | — |
| 清空结果 | setResult(null), setStatus("idle") | 未验证 | — |
| 置顶按钮 | setAlwaysOnTop 切换 | 未验证 | — |
| 长文本显示 | whitespace-pre-wrap, overflow-auto | 未验证 | — |
| 空 OCR 错误提示 | "OCR 没有识别到文本" error | 未验证 | — |

## 人工验证 Checklist

在真实桌面环境（非远程桌面、非无头）执行以下步骤，每项填写证据：

```
环境：Windows ___, 分辨率 ___, DPI ___%
App 版本：git commit 6c420c6

[ ] 1. 打开 App → OCR 页 → 看到"截图翻译"卡片
    证据：截图

[ ] 2. 点击"开始截图翻译"
    预期：主窗口消失
    证据：截图

[ ] 3. 全屏截图窗口出现
    预期：显示真实桌面冻结画面（非黑屏/白屏）
    证据：截图

[ ] 4. 拖选一段文字区域
    预期：出现蓝色选区框
    证据：截图

[ ] 5. 松开鼠标
    预期：截图窗口关闭，主窗口恢复，显示 OCR 结果
    OCR 原文：___
    翻译结果：___
    证据：截图 + 控制台日志

[ ] 6. 检查控制台日志
    预期：出现 `[OCR] Engine: Windows.Media.Ocr (WinRT)` 或 `[OCR] Engine: tesseract.js`
    实际：___
    证据：控制台截图

[ ] 7. 点击"刷新同一区域"
    预期：重新 OCR + 翻译，结果更新
    OCR 原文：___
    翻译结果：___
    证据：截图

[ ] 8. 点击"复制"按钮（原文和翻译各试一次）
    预期：剪贴板有对应文本
    证据：粘贴结果

[ ] 9. 点击"置顶"按钮
    预期：窗口置顶/取消置顶切换
    证据：截图

[ ] 10. 点击"清空"
    预期：结果消失，回到初始状态
    证据：截图

[ ] 11. 再次"开始截图翻译"，按 Esc
    预期：截图窗口关闭，主窗口恢复
    证据：截图

[ ] 12. DPI 验证（如果 DPI > 100%）
    设置 DPI 为 125% 或 150%
    拖选文字区域，检查 OCR 结果是否对应选区内容
    证据：截图 + OCR 原文
```

## P2: Next Steps

1. **独立浮动结果窗口**：暂不需要。当前结果嵌入主页面，刷新/复制方便。如果用户需要同时看原文和翻译，再考虑独立窗口。
2. **区域绑定窗口**：暂不需要。MVP 场景是"截一次翻一次"，不需要持续跟随。
3. **多显示器正式支持**：需要。当前只截主屏，双屏用户无法截取第二屏。方案：传入显示器坐标给 `prepare_screenshot_snapshot`。
4. **OCR 预处理**：暂不需要。WinRT OCR 对中文/英文识别质量足够。如果识别率不够再加灰度化/二值化。
5. **替换 tesseract fallback**：暂不需要。tesseract.js 作为 fallback 可以覆盖 WinRT 不可用的场景。
