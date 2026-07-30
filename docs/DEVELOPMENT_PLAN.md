# Further Development Plan (2026-07-30)

Synthesis of deep study across **16 reference projects** (pot-desktop, STranslate, LunaTranslator, immersive-translate, read-frog, AiNiee, youdao-dict, Easydict, QTranslate, MouseTooltipTranslator, YomiNinja, **snow-shot**, **capcap**, **BabelDOC**, **PDFMathTranslate**, **kivio**, **old-immersive-translate**). Focus: **mature designs not yet fully absorbed** into Moon Translator.

Follows: [REFERENCE_STUDY.md](./REFERENCE_STUDY.md) §1–8 (prior pass) + this round §9–14 (six new deep studies). Live compass: [CURRENT_FOCUS.md](./CURRENT_FOCUS.md).

**Policy unchanged:** first-party only · steal pipelines & coordinates, not foreign UI stacks · Luna Hook engines still frozen · OCR session lifecycle frozen.

---

## 0. Executive summary — where we under-absorbed

The user's intuition is correct. Five high-value patterns from mature references are **not yet absorbed** and are blocking product quality:

| Gap | Reference | Current Moon state | Impact |
|-----|-----------|--------------------|--------|
| **Browser bilingual display** (paragraph-level original+translation) | old-immersive-translate | Direct `textContent` rewrite, no original preservation, no CSS isolation | Core "immersive translate" UX missing |
| **PDF layout-preserving translation** | BabelDOC + PDFMathTranslate | EPUB only; PDF path weak | PDF is P0 in audit, currently unshippable |
| **Hot-load page pool** (eliminate webview cold-start) | snow-shot | OCR result windows created on-demand, first-frame white flash | OCR UX regression |
| **Multi-pin stacked layout** (M2 milestone) | capcap | Single-frame only (M0 gate) | M2/M3+ blocked |
| **Lens freeze-frame + WS_EX_NOACTIVATE** | kivio | Hide/show flicker on capture; Windows non-activate flag missing | Capture flicker, focus stealing |

Six additional medium-impact patterns are partially absorbed (filter rules, batch separators, IndexedDB cache, formula placeholders, ONNX hot_start, geometry abstraction).

---

## 1. Reference absorption matrix

### 1.1 Fully absorbed (no action)

| Pattern | Source | Moon location |
|---------|--------|---------------|
| Event-driven clipboard + dedupe | STranslate | `clipboard_dedupe`, synthetic suppress |
| Selection fetch (key-up → Ctrl+C → sequence wait) | STranslate | hotkey Released + `GetClipboardSequenceNumber` |
| Replace (cancel-if-running, type-vs-paste) | STranslate | `in_flight` cancel-only + `useClipboardOutput` |
| Text symbol repair / response check / numbered parse | AiNiee | `post_process.rs`, `response_check.rs` |
| Multi-engine selection overlay | pot + Easydict | `display_text` shared, hotkey path + auto_watch |
| Multi-monitor clamp | kivio (partial) | `clamp_rect_to_cursor_monitor` |
| Dictionary hotkey (QTranslate D) | QTranslate | `hotkeys.dictionaryLookup` |
| Compact overlay + theme hook | kivio | `overlay/html_builder.rs`, `set_overlay_theme` |
| Hook multi-module IAT + dedup | Luna | `hook_text.cpp` EnumProcessModules + `hook_text_is_noise` |

### 1.2 Partially absorbed (needs completion)

| Pattern | Source | Moon current | Gap |
|---------|--------|--------------|-----|
| OCR layout analysis (Auto/Smart/NoMerge) | STranslate `OcrLayoutAnalyzer` | None | New `ocrLayoutAnalyzer.ts` needed (S6) |
| Multi-engine result cards + stale request id | pot | `translateStore.results[]` exists, stale id missing | P6 incomplete |
| Translation cache (cross-tab, persistent) | old-immersive-translate | `sessionStorage` per-tab | IndexedDB + SHA-1 key needed |
| LLM batch separator protocol | old-immersive-translate `translationService.js` | Direct segments array | `[[idx]]` placeholder protocol needed |
| Page translate mutation observer | old-immersive-translate | TEXT_NODE only, no 2s throttle | Element nodes + throttle needed |
| Tray as full control surface | pot | Tray menu exists | P2 incomplete |
| Control HTTP API | pot | `api_server.rs` exists, auth added | Side-effect routes (P1) |

### 1.3 Not absorbed (new work required)

| Pattern | Source | Priority |
|---------|--------|----------|
| Paragraph-level bilingual display (clone + font wrap) | old-immersive-translate `enhance.js` | **P0** |
| PDF Intermediate Layer + reflow algorithm | BabelDOC `typesetting.py` + `il_version_1.py` | **P0** |
| Per-character Tm absolute positioning + `{vN}` formula placeholder | PDFMathTranslate `converter.py` | **P0** |
| Hot-load page pool | snow-shot `hot_load_page_service.rs` | **P0** |
| Multi-pin PinWindowManager + stackedOrigin | capcap `PinLauncher.swift` | **P1** (M2) |
| Freeze-frame capture | kivio `lens_commands.rs:2246` | **P1** |
| Windows WS_EX_NOACTIVATE equivalent of NSPanel | kivio (macOS only) | **P1** |
| DocLayout-YOLO ONNX integration | BabelDOC `doclayout.py` + PDFMathTranslate `doclayout.py` | **P1** |
| Font subset + glyph id encoding | BabelDOC `fontmap.py` + PDFMathTranslate `raw_string` | **P1** |
| OCR geometric post-processing (DBNet → Markdown) | kivio `rapidocr.rs:537` | **P2** |
| One-shot subprocess OCR worker (OS reclaims model memory) | kivio `rapidocr.rs:192` | **P2** |
| Scroll stitching (Vision registration + sticky element exclusion) | capcap `ScrollCapturer.swift` | **P2** (M3+) |
| fooCount generation guard for async translation | old-immersive-translate `pageTranslator.js:269` | **P2** |
| Special-rules site adaptation framework | old-immersive-translate `specialRules.js` | **P3** |
| Main container heuristic (find largest `<p>`, climb to 40% page text) | old-immersive-translate `enhance.js:488` | **P3** |
| Closed shadow DOM for tooltips/progress UI | old-immersive-translate `showOriginal.js:130` | **P3** |
| ResizeWindowService (aspect-ratio-constrained drag) | snow-shot `resize_window_service.rs` | **P3** |
| HDR capture + magnifier color inversion | snow-shot `monitor_info.rs:719` | **P3** (accessibility) |

---

## 2. Development plan — prioritized roadmap

### Tier 0 — Stabilize desktop selection (current, unchanged)

Continue [CURRENT_FOCUS.md](./CURRENT_FOCUS.md) Tier 0. Selection smoke open for owner acceptance. No new reference absorption here.

### Tier 1 — Browser extension bilingual display (NEW P0)

**Goal:** Ship paragraph-level bilingual display, closing the largest UX gap vs immersive-translate.

| Slice | Source | Effort |
|-------|--------|--------|
| **B1** `<font>` wrapper + inline style + `nodesToRestore` array | old-immersive-translate `pageTranslator.js:619-656` | S |
| **B2** Dual-language clone (cloneNode + insertBefore + `formatCopiedNode` with `display:none` + `notranslate`) | old-immersive-translate `enhance.js:385-481` | M |
| **B3** Filter completion (`notranslate` / `translate=no` / `contenteditable` / `data-translationmark=copiedNode` ancestor) | old-immersive-translate `enhance.js:134-176` | S |
| **B4** `dualStyle` modes: underline / highlight / weakening / mask | old-immersive-translate `enhance.js:616-625` | S |
| **B5** `fooCount` generation guard (translatePage/restorePage ++, async result check) | old-immersive-translate `pageTranslator.js:269,785` | S |
| **B6** Piece切分 (inline/block classification + 1000-char auto-split) | old-immersive-translate `pageTranslator.js:394-535` | M |
| **B7** IndexedDB cache (`service@source.target` store, SHA-1 key, in-memory L1) | old-immersive-translate `translationCache.js` | M |
| **B8** LLM batch separator protocol (`[[idx]]` + system prompt rule + tolerant regex parse) | old-immersive-translate `translationService.js:628-798` (adapted) | M |

**Sequence:** B1+B3+B5 first (correctness) → B2+B4 (bilingual display) → B6+B7+B8 (performance + LLM batch).

**Do not:** port `specialRules.js` site list yet (P3); port YouTube/PDF sub-features.

### Tier 1.5 — PDF layout-preserving translation (NEW P0, parallel with Tier 1)

**Goal:** Ship PDF bilingual translation with layout preservation. Currently EPUB-only.

| Slice | Source | Effort |
|-------|--------|--------|
| **P1** Define Rust IL data structures (Document/Page/Paragraph/Character/Formula/TranslatedUnicode), serde-serializable | BabelDOC `il_version_1.py` | M |
| **P2** PDF→IL frontend via `pdfium-render` + custom operator interpreter (preserve `passthrough_per_char_instruction` for unknown ops) | BabelDOC `il_creater_active.py` + PDFMathTranslate `pdfinterp.py` | L |
| **P3** `_find_optimal_scale_and_layout` reflow (scale-down 0.05/0.1 step + box-expansion down-then-right + CJK line_skip=1.5) | BabelDOC `typesetting.py:941` | M |
| **P4** Per-character `Tf/Tm/TJ` writeback with absolute positioning | PDFMathTranslate `converter.py:385-386,409-511` | M |
| **P5** Formula placeholder `{vN}` + three-signal detection (font regex + Unicode class + `has_glyph` check) | PDFMathTranslate `converter.py:205-275` + BabelDOC `formular_helper.py` | M |
| **P6** DocLayout-YOLO ONNX via `ort` crate (input 1024×1024 + resize/pad, fallback_line for missed pages) | BabelDOC `doclayout.py:40` + PDFMathTranslate `doclayout.py` | M |
| **P7** Font subset (PyMuPDF subprocess with timeout, or `subset` crate) + glyph id encoding | PDFMathTranslate `high_level.py:246` + BabelDOC `fontmap.py:67` | M |
| **P8** `obj_patch` + `q ops_base Q cm ops_new` coordinate isolation | PDFMathTranslate `pdfinterp.py:254-278` | S |
| **P9** Mono + dual PDF dual-output (interleaved page insertion) | PDFMathTranslate `high_level.py:393-405` | S |
| **P10** SQLite + WAL translation cache (engine+params+text triple key, recursive sort JSON) | PDFMathTranslate `cache.py` | S |

**Sequence:** P1+P3+P4+P8 first (round-trip "PDF → IL → PDF" with zero loss, no translation) → P5+P6 (formula + layout detection) → P2 (full frontend) → P7+P9+P10 (polish).

**Short-term bridge:** if Rust rewrites take too long, spawn PDFMathTranslate v1 as Python subprocess (cf. `kernel/v2_bridge.py`) — same pattern as BabelDOC's own v2 bridge. Acceptable for v1, not for v2.

**Do not:** port BabelDOC's `AutomaticTermExtractor` (30% weight, low value for desktop); port full plugin marketplace.

### Tier 2 — OCR screenshot app-ification (M2 multi-pin)

**Goal:** Ship M2 (multi-pin static) per [OCR_STRATEGY.md](./OCR_STRATEGY.md). Unblocks M3+ (multi live frame).

| Slice | Source | Effort |
|-------|--------|--------|
| **O1** `PinWindowManager` retain pool (Tauri `Map<id, WebviewWindow>` + explicit `destroy()` + `map.delete(id)`) | capcap `PinLauncher.swift:285-296` | S |
| **O2** `stackedOrigin` 28pt阶梯 + clamp to `visibleFrame` | capcap `PinLauncher.swift:256-269` | S |
| **O3** `pinSource` + "close and clear source" action (right-click menu) | capcap `PinLauncher.swift:6-10,45-59` | S |
| **O4** Pin window autonomy — each pin webview manages own local state, manager only holds id list | capcap design pattern | S |
| **O5** Hot-load page pool (pre-warm N invisible webviews, pop on demand, async refill) | snow-shot `hot_load_page_service.rs` | M |
| **O6** `set_exclude_from_capture` (Windows) before screenshot, reset after | snow-shot `screenshot.rs:813-826` | S |
| **O7** OCR `hot_start` + `model_write_to_memory` (preload 3 ONNX files to `Vec<u8>`, lazy session init) | snow-shot `ocr_service.rs:66-114` | S |
| **O8** Hotkey独立 slot + conflict detection (register/unregister per feature, pre-check on save) | capcap `HotkeyManager.swift:912-1098` | M |
| **O9** DWM `DWMWA_TRANSITIONS_FORCEDISABLED=1` on screenshot draw window creation | snow-shot `screenshot/src/lib.rs:631` | XS |

**Sequence:** O1+O2+O3+O4 (M2 multi-pin core) → O5+O6+O7 (UX polish, eliminates white flash + first-OCR lag) → O8+O9 (reliability).

**Do not:** port capcap scroll stitching yet (M3+); port `OverlayPanelPool` (Tauri webview pool already covers it via O5).

### Tier 2.5 — OCR capture quality (parallel with Tier 2)

**Goal:** Eliminate capture flicker + focus stealing. Currently `OCR_INVARIANTS.md` I1–I7 still manual smoke.

| Slice | Source | Effort |
|-------|--------|--------|
| **C1** Freeze-frame mechanism (capture full monitor on select-enter, crop from frame for region shots) | kivio `lens_commands.rs:2246-2291` | M |
| **C2** Geometry abstraction layer (`monitor_for_region` + `windows_monitor_region`, multi-monitor + scale_factor + negative origin, with unit tests) | kivio `capture_geometry.rs` | S |
| **C3** Windows `WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW` explicit flag on overlay windows (equivalent to macOS NSPanel non-activating) | kivio macOS pattern, Windows gap | S |
| **C4** `ShowWindow(HWND, SW_SHOWNOACTIVATE)` instead of `set_focus()` for overlay | Windows API | XS |
| **C5** Front App hand-off (snapshot `GetForegroundWindow` before overlay show, restore after close) | kivio `windows.rs:730-848` | S |
| **C6** Selection capture three-state (Text/Empty/Unavailable — Empty skips Ctrl+C fallback, avoids system beep) | kivio `shortcuts.rs:388-456` | S |
| **C7** `take-once` reset payload + mode baked into URL hash (avoids mount/event double-trigger on cold mount) | kivio `lens_commands.rs:503-582` | S |

**Sequence:** C1+C2 (foundation) → C3+C4+C5 (Windows non-activate) → C6+C7 (selection robustness).

### Tier 3 — Engine facade + Hook verdict (unchanged, after Tier 1+2)

Per [MODULE_MAP.md](./MODULE_MAP.md) B+C. No new reference absorption; complete existing E1–E5 slices.

### Tier 4 — Polish (P2/P3 from references, scheduled not now)

| Slice | Source | Tier |
|-------|--------|------|
| OCR geometric post-processing (DBNet → Markdown, dynamic line-height aggregation, CJK space insertion) | kivio `rapidocr.rs:537-712` | P2 |
| One-shot subprocess OCR worker (OS reclaims model memory via `current_exe()` self-invocation) | kivio `rapidocr.rs:192-230` | P2 |
| `ResizeWindowService` (aspect-ratio-constrained drag for pin windows) | snow-shot `resize_window_service.rs` | P2 |
| Multi-monitor parallel capture (rayon `par_iter` + `overlay_image_ptr`合成) | snow-shot `monitor_info.rs:345-522` | P2 |
| `specialRules` site adaptation framework (5–10 high-frequency sites: Twitter/Reddit/GitHub/YouTube/Zhihu/WeChat) | old-immersive-translate `specialRules.js` | P3 |
| Main container heuristic (find largest `<p>`, climb to 40% page text, `blacklist=["comment"]`) | old-immersive-translate `enhance.js:488-571` | P3 |
| Closed shadow DOM for tooltips/progress UI (`all: initial` reset) | old-immersive-translate `showOriginal.js:130-146` | P3 |
| Scroll stitching (image registration + sticky element exclusion + frame-settle detection) | capcap `ScrollCapturer.swift:370-603` | P3 (M3+) |
| HDR capture + magnifier color inversion (accessibility) | snow-shot `monitor_info.rs:719-787` | P3 |
| `PyodideWorker` terminate pattern (if AI Chat sandbox is added) | kivio `pyodideWorker.ts` | P3 |
| Floating animation platform branch (macOS `NSAnimationContext`, Windows immediate snap + setTimeout) | kivio `lens_commands.rs:2477-2579` | P3 |

---

## 3. Reference project deep-study index (this round)

### §9. snow-shot — Tauri 同构截图 OCR 工具

**Source:** `tmp/reference/oss/snow-shot/`

| # | Pattern | File:line | Moon target | Priority |
|---|---------|-----------|-------------|----------|
| **SS1** | Hot-load page pool (`HotLoadPageService`, DashMap + 1×1 invisible webviews, pop+async refill, default 2 max 3) | `src-tauri/src-crates/app-services/src/hot_load_page_service.rs` | Tier 2 O5 | P0 |
| **SS2** | OCR `hot_start` + `model_write_to_memory` (preload 3 ONNX to Vec<u8>, lazy session, inter+intra threads = physical cores, GraphOptimizationLevel::Level3) | `src-tauri/src-crates/app-services/src/ocr_service.rs:66-114` | Tier 2 O7 | P0 |
| **SS3** | `set_exclude_from_capture` (Windows, hide self from capture during screenshot) | `src-tauri/src-crates/tauri-commands/screenshot/src/lib.rs:813-826` | Tier 2 O6 | P0 |
| **SS4** | DWM `DWMWA_TRANSITIONS_FORCEDISABLED=1` (disable window show animation on draw window) | `src-tauri/src-crates/tauri-commands/screenshot/src/lib.rs:631` | Tier 2 O9 | P0 |
| **SS5** | Multi-monitor parallel capture (rayon `par_iter` + `overlay_image_ptr` row-copy, single-monitor short-circuit) | `src-tauri/src-crates/app-utils/src/monitor_info.rs:345-522` | Tier 4 | P2 |
| **SS6** | `ResizeWindowService` (30fps mouse sampling, aspect-ratio constraint, 4-direction fixed-point) | `src-tauri/src-crates/app-services/src/resize_window_service.rs` | Tier 4 | P2 |
| **SS7** | Windows HDR capture via Windows Graphics Capture API (fallback to xcap) | `src-tauri/src-crates/app-utils/src/monitor_info.rs` | Tier 4 | P3 |
| **SS8** | Magnifier color inversion (5×5 matrix inverse for accessibility users) | `src-tauri/src-crates/app-utils/src/monitor_info.rs:719-787` | Tier 4 | P3 |
| **SS9** | `DeviceEventHandlerService` (global mouse/keyboard 30fps sampling) | `src-tauri/src-crates/app-services/src/device_event_handler_service.rs` | Tier 4 | P2 |

**Key code (hot-load pool, portable as-is):**
```rust
pub struct HotLoadPageService {
    page_limit: RwLock<usize>,
    page_list: DashMap<String, HotLoadPage>,
    app_handle: RwLock<Option<tauri::AppHandle>>,
    page_id: RwLock<usize>,
}
// create_idle_window_core: 1×1 invisible + skip_taskbar + transparent + focused(false)
// pop_page: find status==true, remove from pool, return window
// refill: async spawn create_idle_windows to maintain page_limit
```

**Do not copy:** rsbuild config, biome.json, commitlint, commercial/non-commercial license dual.

### §10. capcap — 截图贴图/滚动拼接

**Source:** `tmp/reference/oss/capcap/`

| # | Pattern | File:line | Moon target | Priority |
|---|---------|-----------|-------------|----------|
| **CC1** | `PinWindowManager` singleton retain pool (pure retain, no business state; `isReleasedWhenClosed=false` equivalent) | `capcap/Capture/PinLauncher.swift:285-296` | Tier 2 O1+O4 | P1 |
| **CC2** | `stackedOrigin` 28pt阶梯 + `maxDistinctStackOffsets=8` + wrap `*10` micro-offset + `clampedOrigin` to `visibleFrame` | `capcap/Capture/PinLauncher.swift:256-279` | Tier 2 O2 | P1 |
| **CC3** | `pinSource` enum + X-key clear source (clipboard/Finder cleanup on pin dismiss) | `capcap/Capture/PinLauncher.swift:6-10,45-59` | Tier 2 O3 | P1 |
| **CC4** | Pin dismiss three-step (orderOut + contentView=nil + manager.remove) | `capcap/Capture/PinLauncher.swift:39-43` | Tier 2 O1 | P1 |
| **CC5** | Hotkey独立 slot + `hotkeyConflictMessage` pre-check (Carbon `RegisterEventHotKey` per feature) | `capcap/Trigger/HotkeyManager.swift:912-1098` | Tier 2 O8 | P1 |
| **CC6** | `OverlayPanelPool` per-screen pool (`.screenSaver` level + `CanJoinAllSpaces + fullScreenAuxiliary`) | `capcap/Capture/OverlayWindowController.swift:196-202,507-559` | (covered by SS1) | P2 |
| **CC7** | Pre-snapshot frozen desktop (background snapshot before overlay, freezes transient UI) | `capcap/Capture/OverlayWindowController.swift:85,441-479` | Tier 2.5 C1 | P1 |
| **CC8** | Vision translational image registration (`VNTranslationalImageRegistrationRequest`) for scroll stitching | `capcap/Capture/ScrollCapturer.swift:370-449` | Tier 4 (M3+) | P2 |
| **CC9** | Sticky element exclusion (scrollbar detection + sticky header detection, once-per-session cache) | `capcap/Capture/ScrollCapturer.swift:43-58,458-603` | Tier 4 (M3+) | P2 |
| **CC10** | Frame-settle detection (consecutive frames byte-identical via CGImage dataProvider) | `capcap/Capture/ScrollCapturer.swift:141-191` | Tier 4 (M3+) | P2 |
| **CC11** | Incremental preview + final memcpy stitch (separate preview stream from final compose) | `capcap/Capture/ScrollCapturer.swift:242-357` | Tier 4 (M3+) | P2 |
| **CC12** | `PinNavigatorView` mini-map navigator (shown when zoom >1.2x) | `capcap/Capture/PinLauncher.swift:2564-2749` | Tier 4 | P3 |
| **CC13** | `PinInteractivePreviewRenderer` low-res preview during interactive zoom | `capcap/Capture/PinLauncher.swift:882-941` | Tier 4 | P3 |
| **CC14** | History file persistence + xattr cloud URL (NSCache text 512/8MB, retention policy, copy-promotion) | `capcap/Capture/HistoryManager.swift` | Tier 4 | P3 |
| **CC15** | Editor in-place overlay on selection rect (not separate window) | `capcap/Editor/EditWindowController.swift:1377-1418` | Tier 4 | P3 |

**Key code (stackedOrigin, portable verbatim):**
```swift
private func stackedOrigin(baseOrigin: CGPoint, index: Int, size: CGSize, on screen: NSScreen) -> CGPoint {
    let maxDistinctStackOffsets = 8
    let offsetIndex = index % maxDistinctStackOffsets
    let xOffset: CGFloat = 28 * CGFloat(offsetIndex % 4)
    let yOffset: CGFloat = -28 * CGFloat(offsetIndex / 4)
    var origin = CGPoint(x: baseOrigin.x + xOffset, y: baseOrigin.y + yOffset)
    // wrap-around micro-offset
    let wrapIndex = index / maxDistinctStackOffsets
    origin.x += CGFloat(wrapIndex) * 10
    origin.y -= CGFloat(wrapIndex) * 10
    // clamp to visibleFrame
    let sf = screen.visibleFrame
    origin.x = max(sf.minX, min(origin.x, sf.maxX - size.width))
    origin.y = max(sf.minY + size.height, min(origin.y, sf.maxY))
    return origin
}
```

**Correction:** capcap has **no edge magnetic snap**. `window-snap.png` refers to window-detection snap (click to capture window rect), not Aero Snap. If Moon needs edge snap, design separately.

**Do not copy:** Carbon hotkey API (use `tauri-plugin-global-shortcut`); Swift NSWindow API (use Tauri WebviewWindow); AWSV4Signer upload stack.

### §11. BabelDOC — PDF 双语翻译布局保留

**Source:** `tmp/reference/oss/BabelDOC/`

| # | Pattern | File:line | Moon target | Priority |
|---|---------|-----------|-------------|----------|
| **BD1** | Intermediate Layer (IL) dataclass hierarchy (Document > Page > Paragraph > Composition > Character/Formula/TranslatedUnicode, with `@dataclass(slots=True)`, XML schema serializable) | `babeldoc/format/pdf/document_il/il_version_1.py` | Tier 1.5 P1 | P0 |
| **BD2** | `passthrough_per_char_instruction` (preserve unparseable PDF ops verbatim, zero-loss fallback) | `il_version_1.py:58` | Tier 1.5 P2 | P0 |
| **BD3** | `visual_bbox` independent from `box` (font metrics often inflate geometric box; typesetting uses visual_bbox y/y2) | `il_version_1.py:613`, `typesetting.py:442-445` | Tier 1.5 P2 | P0 |
| **BD4** | `_find_optimal_scale_and_layout` reflow (scale 0.05/0.1 step, box-expansion down-then-right, `scale=1.0` retry on expand, CJK line_skip=1.5) | `babeldoc/format/pdf/document_il/midend/typesetting.py:941-1062` | Tier 1.5 P3 | P0 |
| **BD5** | Document-level mode scale (`statistics.multimode(all_scales)`, paragraphs above mode are pressed down) | `typesetting.py:865,919-935` | Tier 1.5 P3 | P1 |
| **BD6** | `TypesettingUnit` wrapper (char/formula/unicode trichotomy, `relocate(x,y,scale)`, `is_cjk_char`/`can_break_line`/`is_hung_punctuation` language attrs) | `typesetting.py:90` | Tier 1.5 P3 | P0 |
| **BD7** | Three-signal formula detection (char-level `is_formulas_start_char` + font-level `is_formulas_font` regex + layout-level `formula` class from YOLO) | `babeldoc/format/pdf/document_il/utils/formular_helper.py` | Tier 1.5 P5 | P1 |
| **BD8** | Formula placeholder `<formula_N>` for LLM translation | `babeldoc/format/pdf/document_il/midend/il_translator.py:502-519` | Tier 1.5 P5 | P0 |
| **BD9** | `relocation_transform` for formula (don't pollute CTM, separate matrix for translation/scale) | `typesetting.py:709-725,769-785` | Tier 1.5 P5 | P1 |
| **BD10** | DocLayout-YOLO ONNX integration (`OnnxModel.predict`, 1024×1024 input + resize/pad, CoreML on macOS, CPU-only elsewhere) | `babeldoc/docvision/doclayout.py:40` | Tier 1.5 P6 | P1 |
| **BD11** | `fallback_line` layout (when YOLO misses, cluster chars into lines as fallback) | `babeldoc/format/pdf/document_il/midend/layout_parser.py:178` | Tier 1.5 P6 | P1 |
| **BD12** | `FontMapper` (per-lang font family: china-ss/japan-s/korea-s, `has_glyph` with `lru_cache(10240)`, bold/italic/serif-aware `map()`) | `babeldoc/format/pdf/document_il/utils/fontmap.py:35,67-72,154` | Tier 1.5 P7 | P1 |
| **BD13** | `CharacterRenderUnit` output (`q <gs> BT /F <size> Tf 1 0 0 1 x y Tm <hex> Tj ET Q`) | `babeldoc/format/pdf/document_il/backend/pdf_creater.py:71-135` | Tier 1.5 P4 | P0 |
| **BD14** | `subset_fonts_in_subprocess` with timeout (multiprocessing + join+terminate, avoids fonttools hang on corrupt fonts) | `backend/pdf_creater.py:1220` | Tier 1.5 P7 | P1 |
| **BD15** | Priority thread pool for translation (`priority = 1048576 - paragraph_token_count`, short paragraphs first) | `il_translator.py:412,477` | Tier 1.5 (also LLM batch) | P2 |
| **BD16** | `translate_tracking.json` paragraph-level cache (incremental retranslation support) | `il_translator.py:418-426` | Tier 1.5 P10 | P2 |
| **BD17** | `rtree` spatial index for paragraph vertical conflict detection (gap 0.5pt small / 3pt large) | `typesetting.py:1137` | Tier 1.5 P3 | P2 |
| **BD18** | CJK ↔ Latin inline spacing (`space_width * 0.5` at boundaries) | `typesetting.py:1373-1398` | Tier 1.5 P3 | P2 |
| **BD19** | Hung punctuation + line-start forbidden punctuation rules | `typesetting.py:1407` | Tier 1.5 P3 | P2 |

**Do not copy:** `AutomaticTermExtractor` (30% weight, low desktop value); `pdfminer.six` vendored fork (use `pdfium-render` instead); PyMuPDF dependency (use Rust `ab_glyph` + `rustybuzz`).

### §12. PDFMathTranslate — 坐标级文本替换

**Source:** `tmp/reference/oss/PDFMathTranslate/`

| # | Pattern | File:line | Moon target | Priority |
|---|---------|-----------|-------------|----------|
| **PT1** | `gen_op_txt` per-character absolute positioning (`Tf` + `Tm` + `TJ` trinity, per-char `adv` advance) | `pdf2zh/converter.py:385-386,409-511` | Tier 1.5 P4 | P0 |
| **PT2** | `raw_string` glyph id encoding (Noto: `has_glyph(ord(c))` → `%04x`; CID: `%04x`; single-byte: `%02x`) | `pdf2zh/converter.py:368-374` | Tier 1.5 P7 | P0 |
| **PT3** | Formula `{vN}` placeholder (insert before translation, regex match-back after) | `pdf2zh/converter.py:275,410-411` | Tier 1.5 P5 | P0 |
| **PT4** | Formula font regex `(CM[^R]|MS.M|XY|MT|BL|RM|EU|LA|RS|LINE|LCIRCLE|TeX-|rsfs|txsy|wasy|stmary|.*Mono|.*Code|.*Ital|.*Sym|.*Math)` | `pdf2zh/converter.py:205-209` | Tier 1.5 P5 | P0 |
| **PT5** | Subscript detection by font-size ratio (`child.size < pstk[-1].size * 0.79`) | `pdf2zh/converter.py:244` | Tier 1.5 P5 | P1 |
| **PT6** | `LANG_LINEHEIGHT_MAP` + auto-shrink (zh-cn=1.4, ja=1.1, ko=1.2, en=1.2; -0.05 per overflow until 1.0) | `pdf2zh/converter.py:377-381,513-516` | Tier 1.5 P3 | P1 |
| **PT7** | `obj_patch` page-level writeback (`q {ops_base} Q 1 0 0 1 {x0} {y0} cm {ops_new}`, q/Q coordinate isolation) | `pdf2zh/pdfinterp.py:254-278` | Tier 1.5 P8 | P0 |
| **PT8** | `obj_patch` for XForm XObject (matrix inverse + recursive `do_Do`) | `pdf2zh/pdfinterp.py:196-252,233-243` | Tier 1.5 P2 | P1 |
| **PT9** | `fontmap`/`fontid` bidirectional (fontid → PDFFont, PDFFont → fontid for writeback) | `pdf2zh/pdfinterp.py:90-100` | Tier 1.5 P2 | P0 |
| **PT10** | `doc_zh` + `doc_en` dual doc + `insert_file` + `move_page` for interleaved bilingual | `pdf2zh/high_level.py:393-405` | Tier 1.5 P9 | P1 |
| **PT11** | `subset_fonts(fallback=True)` PyMuPDF builtin | `pdf2zh/high_level.py:246-248` | Tier 1.5 P7 | P1 |
| **PT12** | Font insert + Resources/Font injection per xref (including XObject res) | `pdf2zh/high_level.py:202-229` | Tier 1.5 P7 | P1 |
| **PT13** | `LANG_NAME_MAP` (zh-cn→SourceHanSerifCN, ja→SourceHanSerifJP, ko→SourceHanSerifKR, others→GoNotoKurrent) | `pdf2zh/high_level.py:410-435` | Tier 1.5 P7 | P1 |
| **PT14** | SQLite + WAL translation cache (triple key `engine+params+text`, recursive sort JSON, `ON CONFLICT REPLACE`, `busy_timeout=1000`) | `pdf2zh/cache.py:12-42,103-105` | Tier 1.5 P10 | P2 |
| **PT15** | `v2_bridge.py` Python subprocess pattern (CLI args + `PDF2ZH_` env prefix) | `pdf2zh/kernel/v2_bridge.py:105-158` | Tier 1.5 short-term bridge | P2 |
| **PT16** | ONNX layout detection (render page → YOLO → write to `box` matrix, `cls==0` = formula/figure/table/abandon) | `pdf2zh/high_level.py:130-159` | Tier 1.5 P6 | P1 |

**Key code (per-character writeback, the core algorithm):**
```python
def gen_op_txt(font, size, x, y, rtxt):
    return f"/{font} {size:f} Tf 1 0 0 1 {x:f} {y:f} Tm [<{rtxt}>] TJ "

# Per-character loop in C-phase
while ptr < len(new):
    ch = new[ptr]
    if fcur_ == self.noto_name:
        adv = self.noto.char_lengths(ch, size)[0]
    else:
        adv = self.fontmap[fcur_].char_width(ord(ch)) * size
    if fcur_ != fcur or vy_regex or x + adv > x1 + 0.1 * size:
        # flush buffer with one Tf/Tm/TJ op
        ops_vals.append({"type": OpType.TEXT, "font": fcur, "size": size,
                         "x": tx, "dy": 0, "rtxt": raw_string(fcur, cstk), "lidx": lidx})
        cstk = ""
    cstk += ch
    x += adv
    ptr += 1
```

**Rust port feasibility:**
- ✅ Portable: `vflag` regex + Unicode class, `receive_layout` A-phase, `gen_op_txt`/`gen_op_line`, `raw_string` via `ab_glyph`/`rustybuzz`, C-phase layout loop
- 🔴 Replace: `pdfminer.six` → `lopdf` + custom operator interpreter; `pymupdf` → `pdfium-render` + `ab_glyph`; `onnxruntime` → `ort` crate; `peewee` → `rusqlite`
- 🔴 Hardest: PDF operator interpreter (write self, `lopdf` is too low-level); `PDFFont.char_width(cid)` (handle Type1/TrueType/CID width tables)

**Recommended path:** Short-term Python subprocess bridge (PT15 pattern) → mid-term Rust rewrite of `converter.py` algorithm layer → long-term self-contained Rust PDF operator interpreter.

### §13. kivio — Tauri Lens 覆盖层

**Source:** `tmp/reference/oss/kivio/`

| # | Pattern | File:line | Moon target | Priority |
|---|---------|-----------|-------------|----------|
| **KV1** | NSPanel runtime reclassification (`object_setClass` to `KivioOverlayPanel`, `_setPreventsActivation:YES` private tag补全, `NONACTIVATING_PANEL` styleMask, `CanJoinAllSpaces+FullScreenAuxiliary`) | `src-tauri/src/windows.rs:660-728` | (macOS only; Windows see KV3) | P1 (mac) |
| **KV2** | `orderFrontRegardless` display (not `makeKeyAndOrderFront`, no app activation) + `makeKeyWindow` + `makeFirstResponder(WKWebView)` for keyboard focus | `src-tauri/src/windows.rs:362-418` | (macOS only) | P1 (mac) |
| **KV3** | Windows `WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW` equivalent (kivio has gap, Moon should补齐) + `ShowWindow(SW_SHOWNOACTIVATE)` | (new for Moon) | Tier 2.5 C3+C4 | P0 (Windows) |
| **KV4** | Freeze-frame mechanism (capture full monitor on select-enter, crop from frame; macOS SCK excludes self PID, Windows avoids video-black-under-transparent-webview) | `src-tauri/src/lens_commands.rs:2246-2291` | Tier 2.5 C1 | P0 |
| **KV5** | `freeze_frame_crop_rect` (scale_factor clamp + boundary clamp, with unit tests) | `src-tauri/src/lens_commands.rs:2354-2391,2941-2986` | Tier 2.5 C1 | P0 |
| **KV6** | `capture_geometry.rs` abstraction (`monitor_for_region` by max overlap, `windows_monitor_region` logical→physical, handles negative origin + scale_factor, full unit tests) | `src-tauri/src/capture_geometry.rs` | Tier 2.5 C2 | P0 |
| **KV7** | Front-app hand-off (`remember_frontmost_app` before overlay show, `restore_previous_frontmost_app` after close, only activate if PID changed) | `src-tauri/src/windows.rs:730-848` | Tier 2.5 C5 | P1 |
| **KV8** | Selection capture three-state (`AxSelection::Text/Empty/Unavailable`, Empty skips Ctrl+C fallback avoids system beep) | `src-tauri/src/shortcuts.rs:388-456` | Tier 2.5 C6 | P1 |
| **KV9** | Clipboard `changeCount` double-check (covers "selected text == existing clipboard" edge case) | `src-tauri/src/shortcuts.rs:419-455` | Tier 2.5 C6 | P1 |
| **KV10** | `wait_for_copy_shortcut_modifiers_to_clear(450ms)` (avoid Cmd+Shift+C combo when user hasn't released Lens hotkey) | `src-tauri/src/shortcuts.rs` | Tier 2.5 C6 | P1 |
| **KV11** | `take-once` reset payload + mode baked into URL hash `#lens?mode=translate` (cold mount first frame reads correct mode, no event double-trigger) | `src-tauri/src/lens_commands.rs:503-582` | Tier 2.5 C7 | P0 |
| **KV12** | `run_overlay_on_main` single funnel for objc closures + `objc_exception::try` (catches NSException, `catch_unwind` can't) | `src-tauri/src/windows.rs:462-556` | (macOS only) | P1 (mac) |
| **KV13** | Lens history (`HISTORY_MAX=20`, `HISTORY_THUMB_SIZE=96` JPEG 0.7 in localStorage, persistent copy in `{app_data_dir}/lens-history/`, `resolve_explain_image_path` two-level resolution) | `src/lens/history.ts` + `src-tauri/src/lens_commands.rs:2861-2939` | Tier 4 | P2 |
| **KV14** | RapidOCR one-shot subprocess worker (`current_exe()` self-invoke with `--kivio-rapidocr-worker`, OS reclaims ONNX address space, `worker_lock: Mutex<()>` serialize) | `src-tauri/src/rapidocr.rs:192-230` | Tier 4 | P2 |
| **KV15** | `join_text_regions` OCR geometric post-processing (dynamic line-height aggregation, soft-wrap vs paragraph break, list-item Markdown, CJK↔ASCII space insertion) | `src-tauri/src/rapidocr.rs:537-712` | Tier 4 | P2 |
| **KV16** | Annotation composition (OffscreenCanvas + draw order mosaic-first/arrow-rect-on-top, lineWidth = `max(3, naturalWidth/400)`) | `src/lens/annotation.ts:9-114` | Tier 4 | P2 |
| **KV17** | `setWindowRgn` Windows region crop (full-screen → card rect + `FLOATING_PADDING=24` outer expand) | `src-tauri/src/lens_commands.rs:338` | Tier 2.5 (Windows) | P1 |
| **KV18** | Floating animation platform branch (macOS `NSAnimationContext`+`animator setFrame:display:NO` native refresh; Windows immediate snap + setTimeout) | `src-tauri/src/lens_commands.rs:2477-2579` | Tier 4 | P3 |
| **KV19** | Lens → Chat pipeline (`PendingChatExternalSend` + `chat-external-send-ready` event, distinguishes single message vs history preset) | `src-tauri/src/lens_commands.rs:1028-1168` | Tier 4 (if Chat added) | P3 |
| **KV20** | Pyodide Worker terminate pattern (only way to reclaim WASM linear memory without closing window) | `src/chat/pyodideWorker.ts` | Tier 4 (if sandbox added) | P3 |
| **KV21** | SCK `prewarm()` (amortize first-screenshot WindowServer query 30-80ms) | `src-tauri/src/lens_commands.rs:426-429` | Tier 2.5 (macOS) | P2 (mac) |
| **KV22** | `setHidesOnDeactivate:false` + `setHasShadow:false` (avoid transparent rect window native shadow leaking outside rounded card) | `src-tauri/src/windows.rs:721,727` | (macOS only) | P1 (mac) |
| **KV23** | Destroy path platform branch (Windows: `hide()+destroy()`; macOS: `hide()+destroy_overlay_window` to swap class back first, avoids ObjC abort) | `src-tauri/src/lens_commands.rs:2182-2217` | Tier 2.5 | P0 |

**Key code (Windows WS_EX_NOACTIVATE equivalent, Moon should add):**
```rust
// Moon Translator Windows overlay window should set:
// WS_EX_NOACTIVATE: click doesn't activate the app
// WS_EX_TRANSPARENT: mouse-through (for select state)
// WS_EX_TOOLWINDOW: no taskbar entry
// WS_EX_TOPMOST: equivalent to NS_STATUS_WINDOW_LEVEL
// Display via ShowWindow(HWND, SW_SHOWNOACTIVATE) not SetForegroundWindow
// Keyboard focus: AttachThreadInput trick or SetFocus to child window
```

**Key code (freeze-frame crop, portable as-is):**
```rust
fn freeze_frame_crop_rect(x, y, width, height, scale_factor, image_width, image_height) -> Option<ImageCropRect> {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 { scale_factor } else { 1.0 };
    let x = (x as f64 * scale).round() as i32;
    let y = (y as f64 * scale).round() as i32;
    let w = (width as f64 * scale).round() as i32;
    let h = (height as f64 * scale).round() as i32;
    // clamp to image bounds, reject empty/invalid
}
```

### §14. old-immersive-translate — 浏览器扩展双语显示

**Source:** `tmp/reference/oss/old-immersive-translate/` (cloned 2026-07-30, archived open-source version)

| # | Pattern | File:line | Moon target | Priority |
|---|---------|-----------|-------------|----------|
| **IT1** | `<font>` wrapper + inline `vertical-align:inherit` (deprecated tag, almost no site CSS hits it; original node preserved in `nodesToRestore` array) | `src/contentScript/pageTranslator.js:619-656` | Tier 1 B1 | P0 |
| **IT2** | Dual-language clone (`cloneNode(true)` + `insertBefore` + `formatCopiedNode` sets `display:none` + `class=notranslate` + `data-translationmark=copiedNode`) | `src/contentScript/enhance.js:385-481,604-626` | Tier 1 B2 | P0 |
| **IT3** | Filter completion (`notranslate` class, `translate=no` attr, `isContentEditable`, ancestor `data-translationmark=copiedNode`) | `src/contentScript/pageTranslator.js:414-418`, `enhance.js:134-176` | Tier 1 B3 | P0 |
| **IT4** | `dualStyle` modes (underline `border-bottom:2px solid #72ECE9` / highlight `background:#EAD0B3` / weakening `opacity:0.4` / mask `filter:blur(5px)`+`:hover` restore) | `src/contentScript/enhance.js:616-625` | Tier 1 B4 | P0 |
| **IT5** | `fooCount` generation guard (increment on translatePage/restorePage, async result checks `currentFooCount === fooCount`, discard if mismatch) | `src/contentScript/pageTranslator.js:269,755,785,844,886` | Tier 1 B5 | P0 |
| **IT6** | `getPiecesToTranslate` piece切分 (inline tags `#text,A,B,SPAN,STRONG,EM,...` continuous, block boundary breaks piece, 1000-char auto-split, shadowRoot穿透) | `src/contentScript/pageTranslator.js:225,394-535` | Tier 1 B6 | P1 |
| **IT7** | IndexedDB cache (`service@source.target` store, SHA-1 key, in-memory L1, `cacheList` store for one-click `deleteAll`, incognito `dontSaveInPersistentCache`) | `src/background/translationCache.js:115,257-265` | Tier 1 B7 | P1 |
| **IT8** | LLM batch separator protocol (`<a i=0>...</a><a i=1>...</a>` wrapped in `<pre>`, tolerant regex parse handles text-outside-tag) | `src/background/translationService.js:628-637,706-798` | Tier 1 B8 (adapt to `[[idx]]` for LLM) | P1 |
| **IT9** | 800-char request split + concurrent dedupe (`translationsInProgress` Map keyed by `[sl,tl,requestString].join(",")`, same request shares Promise) | `src/background/translationService.js:424-501` | Tier 1 B6+B8 | P1 |
| **IT10** | MutationObserver incremental (childList+subtree, 2s `translateNewNodes` throttle, `removedNodes` tracking, `visibilitychange` disable when hidden) | `src/contentScript/pageTranslator.js:281-358` | Tier 1 (after B1-B8) | P2 |
| **IT11** | Viewport-driven dynamic translate (`setTimeout(translateDynamically, 600)` poll, only `bottomIsInScreen \|\| topIsInScreen` pieces) | `src/contentScript/pageTranslator.js:717-814` | Tier 1 (after B1-B8) | P2 |
| **IT12** | `specialRules` site adaptation (50+ sites, `selectors`/`containerSelectors`/`noTranslateSelectors`/`blockElements`/`detectLanguage`/`brToParagraph`/`iframeContainer`) | `src/lib/specialRules.js:1-396` | Tier 4 | P3 |
| **IT13** | `getContainers` main container heuristic (find largest `<p>`, climb parent until ≥40% page text, `blacklist=["comment"]`) | `src/contentScript/enhance.js:488-571` | Tier 4 | P3 |
| **IT14** | Attribute translation (`placeholder`/`alt`/`title`, skip `value` to avoid breaking forms) | `src/contentScript/pageTranslator.js:537-617` | Tier 4 | P2 |
| **IT15** | Closed shadow DOM + `all: initial` for tooltip/progress UI (full CSS reset, site styles cannot interfere) | `src/contentScript/showOriginal.js:130-146`, `css/showOriginal.css:1-27` | Tier 4 | P3 |
| **IT16** | Custom dictionary placeholder (`@%index#$` substitution before translation, restore after; degrades to single-segment retry on Google reorder break) | `src/contentScript/pageTranslator.js:29-108` | Tier 4 | P3 |
| **IT17** | PDF translation flex container hack (wrap inline divs in `display:flex` for translatewebpages.org PDF-to-HTML) | `src/contentScript/enhance.js:374-383` | Tier 4 | P3 |
| **IT18** | `swapTranslationService` (manual google↔yandex switch and re-run on failure) | `src/contentScript/pageTranslator.js:922` | Tier 4 | P3 |
| **IT19** | Multi-frame sync (subframe via `getMainFramePageLanguageState`) | `src/contentScript/pageTranslator.js:985-1057` | Tier 4 | P3 |
| **IT20** | YouTube subtitle bilingual via `#content-text` selector | `src/lib/specialRules.js:145-151` | Tier 4 | P3 |

**Key code (font wrapper, portable as-is):**
```js
function encapsulateTextNode(node, translatedText, dualStyle) {
    const fontNode = document.createElement("font");
    let style = 'vertical-align: inherit;';
    if (dualStyle === 'underline') style += ' border-bottom: 2px solid #72ECE9;';
    else if (dualStyle === 'highlight') style += ' background-color: #EAD0B3; padding: 3px 0;';
    else if (dualStyle === 'weakening') style += ' opacity: 0.4;';
    fontNode.setAttribute("style", style);
    fontNode.textContent = translatedText;
    node.replaceWith(fontNode);
    // Original node preserved in nodesToRestore for restorePage
}
```

**Key code (dual-language clone, portable as-is):**
```js
function formatCopiedNode(copyNode, originalDisplay, ctx, pageSpecialConfig) {
    copyNode.setAttribute("data-translationmark", "copiedNode");
    copyNode.style.display = "none";          // hidden until translation done
    copyNode.classList.add("notranslate");    // prevent re-translation
    // Site-specific <br> spacing for reddit/stackoverflow/google/discord
}

// After translation completes:
function showCopyiedNodes() {
    document.querySelectorAll('[data-translationmark="copiedNode"]').forEach(n => {
        n.style.display = '';  // restore display
    });
}
```

**Key code (LLM batch separator — adapt for Moon):**
```js
// Moon should use [[idx]] instead of <a i=N> (LLM-friendly, no HTML)
function buildBatchRequest(segments) {
    return segments.map((s, i) => `[[${i}]]${s}[[/${i}]]`).join('\n');
}
// System prompt: "Preserve all [[N]]...[[/N]] markers verbatim in output"
// Tolerant parse: handle LLM merging/splitting markers
function parseBatchResponse(response, expectedCount) {
    const re = /\[\[(\d+)\]\]([\s\S]*?)\[\[\/\1\]\]/g;
    const result = new Array(expectedCount);
    let m;
    while ((m = re.exec(response)) !== null) {
        result[parseInt(m[1])] = m[2].trim();
    }
    // Fallback: split by [[N]] markers if close-tag missing
    // ...
    return result;
}
```

**Do not copy:** specialRules site list verbatim (legal + maintenance); Yandex/Bing API integration; closed-source current immersive-translate features (AI translation, video subtitle bilingual).

---

## 4. Implementation sequence (12-week view, parallelizable)

```
Week 1-2  │ Tier 1 B1+B3+B5 (font wrap + filter + fooCount)        │ Owner: FE
          │ Tier 2.5 C1+C2 (freeze-frame + geometry abstraction)   │ Owner: BE
          │ Tier 2 O9 (DWM disable, 1 line)                        │ Owner: BE
─────────────────────────────────────────────────────────────────
Week 3-4  │ Tier 1 B2+B4 (dual clone + dualStyle)                  │ Owner: FE
          │ Tier 2 O1+O2+O3+O4 (PinWindowManager + stackedOrigin)  │ Owner: BE
          │ Tier 2.5 C3+C4+C5 (Windows WS_EX_NOACTIVATE + handoff) │ Owner: BE
─────────────────────────────────────────────────────────────────
Week 5-6  │ Tier 1 B6+B7+B8 (piece切分 + IndexedDB + LLM separator) │ Owner: FE
          │ Tier 2 O5+O6+O7 (hot-load pool + exclude_capture + hot_start) │ Owner: BE
          │ Tier 2.5 C6+C7 (selection three-state + take-once)     │ Owner: BE
─────────────────────────────────────────────────────────────────
Week 7-8  │ Tier 1.5 P1+P3+P4+P8 (IL + reflow + per-char writeback + q/Q) │ Owner: BE
          │ Tier 2 O8 (hotkey conflict detection)                  │ Owner: BE
─────────────────────────────────────────────────────────────────
Week 9-10 │ Tier 1.5 P5+P6 (formula {vN} + DocLayout-YOLO ONNX)     │ Owner: BE
          │ Tier 1 (browser) smoke + iteration                     │ Owner: FE
─────────────────────────────────────────────────────────────────
Week 11-12│ Tier 1.5 P2+P7+P9+P10 (full frontend + font subset + dual PDF + SQLite cache) │ Owner: BE
          │ Tier 2 (OCR) smoke + iteration                         │ Owner: BE
```

**Parallelization rule:** Tier 1 (browser FE) and Tier 1.5 (PDF BE) are independent — different owners, different files, no merge conflicts. Tier 2 (OCR BE) and Tier 2.5 (capture BE) share `src-tauri/src/` but different modules (OCR vs overlay/window).

**Hard dependencies:**
- Tier 1 B2 (dual clone) requires B1 (font wrap) — clone uses same wrapper
- Tier 1.5 P3 (reflow) requires P1 (IL) — reflow operates on IL
- Tier 1.5 P4 (writeback) requires P1 (IL) + P3 (reflow computes positions)
- Tier 2 O5 (hot-load pool) requires O1 (PinWindowManager) — pool feeds manager
- Tier 2.5 C1 (freeze-frame) requires C2 (geometry) — crop uses geometry

---

## 5. Verification gates

Each tier has a smoke gate before moving on:

| Tier | Gate | Criteria |
|------|------|----------|
| Tier 1 (browser) | Open 5 test pages (Twitter/Reddit/GitHub/Zhihu/medium.com), enable bilingual mode, verify original+translation both visible, no CSS pollution, switch language 3x rapidly (fooCount guard) | Manual |
| Tier 1.5 (PDF) | Round-trip test: PDF → IL → PDF (no translation) byte-equivalent visually; then translate 3 sample PDFs (academic paper with formulas, novel, technical doc), verify layout preserved + formula intact + bilingual readable | Manual + visual diff |
| Tier 2 (OCR M2) | Pin 5 screenshots simultaneously, verify stacked layout + drag each + close each (no zombie webview) + hotkey独立 + conflict detection blocks duplicate | Manual |
| Tier 2.5 (capture) | OCR screenshot on 3 scenarios: (a) normal desktop, (b) video playing (no black flash), (c) multi-monitor cross-screen — verify no flicker, no focus steal, no self-capture | Manual |

---

## 6. Risks and mitigations

| Risk | Mitigation |
|------|------------|
| LLM batch separator `[[idx]]` parse failure (LLM ignores/mangles markers) | Tolerant regex parse + single-segment fallback retry + log warning (cf. IT8 `handleCustomWords` degrade pattern) |
| PDF operator interpreter Rust rewrite too long | Short-term: Python subprocess bridge (PT15 pattern); mid-term: incremental Rust port starting with most common operators |
| DocLayout-YOLO ONNX model size (~50MB) bloats installer | Download on first use, cache in `app_data_dir/`; or bundle as `resources/` with `tauri.conf.json` |
| Font subset in Rust (`subset` crate) immaturity | Use PyMuPDF subprocess with timeout (BD14 pattern) as bridge; Rust rewrite later |
| Hot-load page pool memory overhead (~30-60MB per idle webview) | Default 2, max 3 (snow-shot's clamp); expose setting; users with <8GB RAM warned |
| Multi-pin webview memory growth on long sessions | Each pin webview self-manages state (CC4); add "close all pins" tray action |
| Windows `WS_EX_NOACTIVATE` keyboard focus issues (click doesn't focus input) | Use `AttachThreadInput` trick or `SetFocus` to child HWND after `SW_SHOWNOACTIVATE` (cf. KV3 notes) |
| `immersive-translate` current version is closed-source | Old open-source version (2023-01 archived) is sufficient for paragraph bilingual; do not attempt to reverse-engineer current version |

---

## 7. Out of scope (explicit non-goals this round)

- **Luna Hook deep inject** — frozen per [CURRENT_FOCUS.md](./CURRENT_FOCUS.md) Tier 2; only after OCR+selection+PDF green
- **Plugin marketplace** — removed per product policy; first-party only
- **AI Chat panel** — kivio's chat module is interesting but not a Moon goal; Pyodide sandbox (KV20) deferred
- **Cloudflare multi-end** — frozen
- **AppState rewrite** — frozen; peel services incrementally per [MODULE_MAP.md](./MODULE_MAP.md) D
- **Full specialRules site list** — port framework + 5-10 sites only (P3); 50+ site list is maintenance burden
- **HDR capture / magnifier inversion** — accessibility niche (P3); only if user requests
- **Scroll stitching** — M3+ milestone, not M2; requires multi-live-frame design first

---

## 8. Related documents

- [REFERENCE_STUDY.md](./REFERENCE_STUDY.md) — §1-8 prior pass (pot/STranslate/read-frog/AiNiee/Easydict/QTranslate/MTT/YomiNinja)
- [REFERENCE_SELECTION_UX.md](./REFERENCE_SELECTION_UX.md) — selection UX deep study
- [REFERENCE_OCR_CAPTURE.md](./REFERENCE_OCR_CAPTURE.md) — OCR capture reference
- [CURRENT_FOCUS.md](./CURRENT_FOCUS.md) — live compass, Tier 0/1/1.5/2/3
- [MODULE_MAP.md](./MODULE_MAP.md) — A/B/C/D/E module map (OCR/Engines/Hook/Repo/Next)
- [OCR_STRATEGY.md](./OCR_STRATEGY.md) — M0/M1/M2/M3+ screenshot-app multi-frame plan
- [OCR_INVARIANTS.md](./OCR_INVARIANTS.md) — I1-I7 invariants
- [HEALTH_AUDIT.md](./HEALTH_AUDIT.md) — P0/P1/P2 repo-wide debt
- [FULL_PROJECT_AUDIT.md](./FULL_PROJECT_AUDIT.md) — original audit basis

---

**Last updated:** 2026-07-30
**Policy owner:** user
**Next review:** after Tier 1 B1-B5 + Tier 2.5 C1-C2 land
