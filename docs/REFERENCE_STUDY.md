# Reference deep study (2026-07-26)

Follow-up to [FULL_PROJECT_AUDIT.md](./FULL_PROJECT_AUDIT.md) §6 — previously **unread** high-value areas in `tmp/reference/oss/`.

**Policy:** Steal pipelines and coordinates; do not paste foreign UI stacks. Luna Hook engines still frozen.

---

## 1. Coverage after this pass

| Project | Before | After this pass |
|---------|--------|-----------------|
| pot-desktop | OCR selector path deep | + multi-window, Translate/Recognize FE, server control API, engine plugin shape, tray |
| STranslate | Screenshot + continuous hide | + OcrLayout, ImageTranslate overlay, ClipboardMonitor, CtrlSameC, replace, ExternalCall |
| LunaTranslator | Product gaps only | Unchanged (inject deep-dive still frozen) |
| immersive-translate | FEATURES gaps | Unchanged (dist-only clone) |
| read-frog | Plan ideas | + host/translate batch, prompts, providers, content entry |
| **AiNiee** | Missing | **2026-07-27** zip extract + A1–A3 ports (symbol repair, response check, numbered parse) — §6 |
| youdao-dict | Policy only | Unchanged |
| **Easydict / QTranslate / MTT / YomiNinja** | — | **2026-07-28** 克隆 + 划词深读 → [REFERENCE_SELECTION_UX.md](./REFERENCE_SELECTION_UX.md) |
| **snow-shot** | — | **2026-07-30** hot-load pool / ONNX hot_start / set_exclude_from_capture / 多屏并行 / ResizeWindowService — §9 in [DEVELOPMENT_PLAN.md](./DEVELOPMENT_PLAN.md) |
| **capcap** | — | **2026-07-30** PinWindowManager / stackedOrigin / 滚动拼接 Vision 配准 + 粘性元素排除 / hotkey 冲突检测 — §10 |
| **BabelDOC** | — | **2026-07-30** IL 中间层 / scale-down+box-expansion reflow / passthrough 兜底 / 三信号公式识别 / DocLayout-YOLO — §11 |
| **PDFMathTranslate** | — | **2026-07-30** 逐字 Tm 绝对定位 / `{vN}` 占位符 / `obj_patch` q/Q 隔离 / SQLite+WAL 缓存 / v2_bridge 子进程模式 — §12 |
| **kivio** | — | **2026-07-30** NSPanel 重分类 / 冻结帧 / 几何抽象层 / 选择三态 / take-once 复位 / 一次性子进程 OCR worker — §13 |
| **old-immersive-translate** | dist-only | **2026-07-30** 克隆归档开源版 → `<font>` 包装 / 双语克隆 / piece 切分 / fooCount 代次防污染 / IndexedDB+SHA-1 / LLM 批量分隔符 — §14 |

**Full synthesis and prioritized roadmap:** [DEVELOPMENT_PLAN.md](./DEVELOPMENT_PLAN.md) — absorption matrix + Tier 1 (browser bilingual) / Tier 1.5 (PDF layout) / Tier 2 (OCR M2 multi-pin) / Tier 2.5 (capture quality) / 12-week sequence + verification gates + risks.

---

## 2. STranslate — findings → Moon

### Steal now (aligns with audit P0/P1)

| # | Steal | Moon target |
|---|--------|-------------|
| **S1** | Event-driven clipboard (`AddClipboardFormatListener` + short settle + dedupe) | **Done** (main + hook + `clipboard_dedupe`; synthetic suppress 2026-07-26) |
| **S2** | Selection fetch: key-up → Ctrl+C → clipboard sequence wait | **Done** (hotkey Released; modifier KeyUp; `GetClipboardSequenceNumber`; restore + mark) |
| **S3** | Replace: cancel-if-running; optional type-vs-paste | **Done** (`in_flight` cancel-only; `useClipboardOutput` + type SendInput) |
| **S4** | Capture returns **bitmap + PhysicalBounds** atomic | Already partially done; keep region in result, no cursor reverse-engineer |
| **S5** | Hide **set** of result windows before snip | `OcrScreenshotTranslator` hide main + stale frames together |

### Steal after OCR geometry stable

| # | Steal | Moon target |
|---|--------|-------------|
| **S6** | `OcrLayoutAnalyzer` Auto/Smart/NoMerge + table/list guards | New `ocrLayoutAnalyzer.ts` → feed `OcrRegionFrame` |
| **S7** | Vector overlay: theme cover, font fit, bg-then-text order | `OcrRegionFrame.tsx` presentation |

### Later

| # | Steal | Moon target |
|---|--------|-------------|
| **S8** | ExternalCall path→action + serial mutex | `api_server.rs` **after** auth (S1) |

---

## 3. pot-desktop — findings → Moon

### Steal (UX / orchestration, not FE eval plugins)

| # | Steal | Moon target |
|---|--------|-------------|
| **P1** | Control HTTP: `/selection_translate`, `/ocr_recognize`, open config | Extend `api_server.rs` side-effect routes (auth first) |
| **P2** | Tray as full control surface (input/OCR/clipboard/auto-copy) | `lib.rs` tray → optional `tray.rs` |
| **P3** | Sentinel payload + `new_text` / modes for one popup | Popup translate or MainTranslator hydrate |
| **P4** | Mouse-monitor placement + DPI clamp | `window.rs` show popup |
| **P5** | Close-on-blur + pin for floating translate | New hook / popup shell |
| **P6** | Multi-engine result cards + stale request id | `MainTranslator` + `translateStore.results[]` |
| **P7** | Recognize workspace after crop | Optional `RecognizeWorkspace` page |
| **P8** | Persist popup geometry | Extend save_window_position |

### Do not copy

- FE `eval` plugins  
- tiny_http (use axum)  
- DeepL free fingerprint scrape as primary  

---

## 4. read-frog — findings → Moon

### Steal (when extension / batch unfrozen)

| # | Steal | Moon target |
|---|--------|-------------|
| **R1** | Feature-scoped provider IDs (page vs selection vs input) | Extension + desktop config |
| **R2** | Pure `translateTextCore` + background queues | Extension SW; desktop optional service |
| **R3** | LLM-only batch with rare separator + count-mismatch fallback | Extension batch; desktop docs already batch |
| **R4** | Page context capped + in cache key | Extension hover/page |
| **R5** | Prompt templates + mode appendices (batch/sentinel) | AiSettings / custom_prompt |
| **R6** | Session cancel scopes for bulk page work | Extension page translate |
| **R7** | Stream LLM for selection; batch for page | Selection vs page paths |

---

## 5. Priority adoption order (cross-reference)

1. **S1 + S2 + S3** — clipboard + replace reliability (audit H4/H5)  
2. **S4 + S5** — OCR capture/hide set polish  
3. **P1 + P2** — control API + tray (cheap external UX)  
4. **P3–P6** — floating multi-engine translate popup  
5. **S6–S7** — OCR layout/overlay quality  
6. **R*** — extension batch/context when extension priority rises  
7. **S8 / P1 auth** — only after API token  

---

## 6. AiNiee (2026-07-27) — studied + ported slices

Source: `tmp/reference/oss/AiNiee-extract/AiNiee-main/` (main.zip via ghfast; direct git clone flaky).

| # | Steal | Moon target | Status |
|---|--------|-------------|--------|
| **A1** | `TextSymbolRepair` dialogue quotes / CJK punct | `post_process.rs` `repair_text_symbols` + `symbol_repair` config; `process_with_source` in translation finalize | Done |
| **A2** | `ResponseChecker` line/empty/identical/newline | `response_check.rs`; warn after `translate_batch_core` | Done |
| **A3** | Numbered `1.` batch parse | `parse_numbered_response` in `response_check.rs`; wired via `LlmEngine::translate_batch_segments` + batch core | Done |
| **A4** | Game extractors (Mtool/T++/Renpy…) | Not needed for desktop OCR/clipboard product | Skip |
| **A5** | PyQt UI / full file writers matrix | Already have docx/pdf/epub/subtitle paths | Skip |

Do not copy: Qt UI, full plugin marketplace, game-only IO plugins.

## 7. Still not studied (honest)

| Target | Reason |
|--------|--------|
| Luna `NativeImpl/LunaHook`, textio, transoptimi | Frozen / experimental inject |
| immersive **source** | Clone is dist-only |
| pot Anki/collection, full recognize engines list | Lower priority |
| youdao resultui/skins | UX glance only if toolbar density needed |
| STranslate full plugin marketplace packages | Marketplace frozen |
| AiNiee NameExtractor / PromptBuilder depth | Optional later if LLM batch product expands |

---

## 8. File index (absolute)

### STranslate
- `tmp/reference/oss/STranslate/src/STranslate/Core/OcrLayoutAnalyzer.cs`
- `.../ImageTranslateTextOverlayLayout.cs`, `ImageTranslateCompactWindowPlacement.cs`
- `.../Helpers/ClipboardMonitor.cs`, `ClipboardHelper.cs`, `CtrlSameCHelper.cs`
- `.../Core/ExternalCallService.cs`

### pot
- `tmp/reference/oss/pot-desktop/src-tauri/src/window.rs`, `server.rs`, `tray.rs`
- `.../src/window/Translate/`, `Recognize/`
- `.../src/services/translate/`

### read-frog
- `tmp/reference/oss/read-frog/src/utils/host/translate/`
- `.../utils/prompts/`, `utils/providers/`, `utils/request/batch-queue.ts`
- `.../entrypoints/background/translation-queues.ts`
- `.../entrypoints/host.content/`, `selection.content/`

### AiNiee
- `tmp/reference/oss/AiNiee-extract/AiNiee-main/ModuleFolders/Domain/TextSymbolRepair/TextSymbolRepair.py`
- `.../Domain/ResponseChecker/{ResponseChecker,BaseChecks,AdvancedChecks}.py`
- Moon ports: `src-tauri/src/post_process.rs`, `src-tauri/src/response_check.rs`
- FE typography/icons: `src/index.css` (`.ui-*`), `src/components/{Icon,PageHeader}.tsx`

### Selection / hover / OCR pickup (2026-07-28)
- Full write-up: [REFERENCE_SELECTION_UX.md](./REFERENCE_SELECTION_UX.md)
- `tmp/reference/oss/easydict_win32/dotnet/src/Easydict.WinUI/Services/{MouseHook,TextSelection,PopButton}Service.cs`
- `tmp/reference/oss/QTranslate/ui-swing/.../{QuickTranslateDialog,QuickDictionaryDialog}.kt`
- `tmp/reference/oss/MouseTooltipTranslator/src/event/mouseover.js`, `src/ocr/ocrView.js`
- `tmp/reference/oss/YomiNinja/yomininja-e/electron-src/{ocr_recognition,overlay}/`
