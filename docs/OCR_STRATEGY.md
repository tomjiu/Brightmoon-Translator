# OCR product strategy (Moon)

## Do **not** wholesale “absorb” another OCR/snip app

| Source | Steal | Don’t copy |
|--------|-------|------------|
| **pot-desktop** | Screenshot window lifecycle: full-monitor capture → file/cache → crop by selection → close selector; optional external snip tool via cache file | Whole UI; older Tauri patterns |
| **STranslate** | OCR **provider plugins** (Baidu/Youdao/…); clipboard + replace as separate modes | WPF stack |
| **LunaTranslator** | Continuous game-region OCR habits; hide-self before capture | Hook/DLL as OCR core |
| **Flameshot / ShareX / PowerToys** | Snip UX (keys, multi-monitor), **not** translate product | Become a pure snipping tool |
| **ocrTranslator / snip2text** | “Save region position” for movie subtitles | Python one-offs |

**Rule:** Moon owns the product shell (region frame + translate + learning later). References inform **pipelines and coordinates**, not a second codebase.

## Target architecture (smart OCR = layers)

```
┌─────────────────────────────────────────────────────────┐
│  Actions (future-friendly, thin)                        │
│  copy src/dst · save PNG · pin · hand-edit overlay      │
└──────────────────────────▲──────────────────────────────┘
┌──────────────────────────┴──────────────────────────────┐
│  Present                                                │
│  region frame · line boxes · toolbar · follow HWND      │
│  (today’s pain: flicker, offset, misalignment)          │
└──────────────────────────▲──────────────────────────────┘
┌──────────────────────────┴──────────────────────────────┐
│  Recognize                                              │
│  WinRT / Youdao / tesseract · boxes · similarity gate   │
└──────────────────────────▲──────────────────────────────┘
┌──────────────────────────┴──────────────────────────────┐
│  Capture                                                │
│  virtual desktop snapshot · crop · hide chrome · DPI    │
└─────────────────────────────────────────────────────────┘
```

Smart features map cleanly if Capture/Present stay pure:

| Idea | Layer | When |
|------|-------|------|
| **Pinned region watch** (固定区域，滚动/内容变了再译) | Capture tick + image fp + text I7 | **Core product** — continuous button; default OFF |
| Follow window | Present + HWND bind (I6) | Optional with pin; stabilize |
| Copy 原文/译文 | Actions on stable text | After geometry green |
| Save region image | Capture crop → file | After single capture path |
| Freeform / multi-rect | Capture selection model | **Later** (rect first) |
| Handwriting / ink | Present overlay + export | **Later** |
| Auto text region detect | Recognize | Optional / P2 |

### Pinned region watch — **产品目标（记住，后做）**

> **冻结开发新能力。** 先把单次 OCR（闪/偏/错位）修稳，再增强监视。  
> 已有 continuous / fingerprint 只作骨架，不在此阶段扩功能。

**用户故事：** 框选页面固定一块 → 开监视 → 页面滚动或该区域内容变了 → 自动 OCR + 翻译；不变则不烧引擎。

```
框选一次 → 区域固定（屏幕坐标，可选 Follow 绑 HWND）
  → ▶ continuous（默认 OFF）
  → 每隔 ocrIntervalMs：
       hide 框 → 截矩形 → show 框     // I1，防自吃 overlay
       图像指纹相同 → 跳过 OCR/翻译   // imageFingerprint（已有）
       图像变了 → OCR → 文本≥0.92 相似 → 跳过翻译  // I7
       文本真变了 → 翻译并更新叠字
```

**预留实现位（修好当前问题后再填）：**

| 槽位 | 文件 / 符号 | 现状 | 以后做 |
|------|-------------|------|--------|
| 连续开关 UI | `OcrRegionFrame` ▶/⏸ | 有 | 文案「监视」；间隔可调 |
| Tick 循环 | `OcrScreenshotTranslator` continuous `useEffect` | 有 | 降闪、可取消 |
| 图变化门闩 | `imageFingerprint` + `lastImageFpRef` | 有 | 更稳哈希 / 可选像素 diff |
| 文变化门闩 | `normalizeOcrText` + 0.92 | 有 | 可配置阈值 |
| Follow 钉窗 | `ocrWindowBinding` + I6 | 有 | 与监视组合验收 |
| 无闪采样 | （未做） | — | 透明采样或离屏截，少 hide |
| 存区 / 复制 | Actions | 复制原文/译文/图 + **保存 PNG** | 文案/toast 可再抛光 |
| 异形选区 | Capture | 仅矩形 | 后置 |

**验收（监视阶段，单次 OCR 绿了之后）：** 固定网页段落 → 开连续 → 滚动使该段变/不变 → 仅变时更新译文；跟窗移动不丢绑。

## Near-term vertical slices

| Slice | Status (code) |
|-------|----------------|
| Geometry constants I2/I3 | Done (`ocrRegionGeometry` + Rust comments) |
| Single first OCR / less flash | Done (preCaptured, ready+ping, stable callbacks) |
| Line alignment I5 | Done (payload dims, contentSize boot, parallel probe+OCR) |
| Memory crop for selection | Done (`crop_screenshot_snapshot` prefers SNAPSHOT_CACHE) |
| pot-style full-screen preview | Done: disk PNG + `convertFileSrc` (no full-screen base64 IPC); show after `img.onLoad` |
| Refresh/continuous = region GDI only | Done (full prepare only for start selection; GDI fail → snapshot fallback) |
| Drag/resize regionRef | Done (xy-only drag; debounced resize OCR) |
| Follow no self-bind | Done (title filter TS+Rust; retry sample point) |
| Continuous fingerprint | Skeleton (product polish **later**) |
| Hide only during grab (not OCR/API) | Done |
| auto OCR sequential WinRT→Youdao | Done (no parallel double-bill) |
| Frame ready wait ≥2.5s | Done |
| sampling settle by affinity vs hide | Done (Rust returns bool) |
| create frame before close selector | Done |
| multi-line OCR → translate_embedded batch | Done (single line still `translate`) |
| crop uses lazy-decoded RGBA cache | Done |
| region frame reuse (reposition, no destroy) | Done |
| pixel fingerprint (24×24 luma Rust) | Done (JS base64 sample as fallback) |
| session-reset on re-snip | Done |
| min-width jump uses OCR_MIN_FRAME_WIDTH_CSS | Done |
| session-reset ACK (no fixed sleep race) | Done |
| translate_embedded optional channel | Done (OCR passes ocr; UI default) |
| GDI fail: retry then soft snapshot | Done |
| ready: register all listeners then emit | Done |
| Selector multi-mon pin from snapshot.info | Done |
| Line align token pack (not raw char slice) | Done (`ocrLineAlign.ts`) |
| WinRT empty → Ok empty (I4) | Done |
| OCR throw → ocr-region-error | Done |
| WinRT/Youdao empty → Ok empty | Done |
| DPR re-read on resize | Done |
| Resize ignore min-width false widen | Done |
| mergeOcrPreferBoxes safer mismatch | Done |
| Session cancel mid OCR/translate | Done |
| Hide settle 32ms (DWM) | Done |
| Save region PNG (dialog) | Done (`write_file_base64`) |
| Translate request join OCR lines with \\n | Done |
| Stronger image fingerprint sampling | Done |
| Continuous uses GDI-first (fast tick) | Done |
| Continuous skips while follow moving | Done |
| Continuous shorter hide settle (16ms) | Done |
| Copy/save action hint on toolbar | Done |
| Align drop blank LLM lines | Done |
| Watch button title 区域监视 | Done |
| contentSize observe without data dep | Done |
| Quiet OCR console.log noise | Done |
| Lang override not clobbered by config load | Done |
| Soft loading event (refresh/continuous) | Done |
| Parallel crop + create frame on select | Done |
| Pending re-OCR via queueMicrotask | Done |
| dialog:allow-save for OCR PNG | Done (capabilities) |
| Background img sized like line map (I5) | Done |
| startScreenshot always force snapshot | Done |

**Manual smoke:** `OCR_SMOKE.md` — when you choose; no progress reports required mid-dev.

| Capture exclude via WDA_EXCLUDEFROMCAPTURE | Done (`set_ocr_region_frame_sampling`) — near zero-blink |
| Soft loading only on manual refresh | Done |
| Per-line translate when ≤5 lines (batch 3) | Done |
| Clear sampling on close / new shot | Done |
| mergeOcr redistribute text on box lines | Done |
| Continuous adaptive interval on skips | Done |
| getCaptureRegion uses win.scaleFactor() | Done |
| Loading veil keeps old overlays | Done |
| Always clear sampling (cancel-safe) | Done |
| containImageCssSize shared helper | Done |
| Esc closes region frame | Done |
| Full-text fallback when no line boxes | Done |
| Stuck selector safety 60s | Done |
| Clear safety timer on select/cancel | Done |
| Watch on/off hint | Done |
| Follow bind via click-through (no hide) | Done |
| I1/I6 docs match sampling/click-through | Done |
| Region frame focus for Esc | Done |
| Clear click-through on close | Done |
| Shared I7 threshold constant | Done |
| Sync continuous UI when main pauses watch | Done |
| Pin/follow toolbar hints | Done |
| Follow bind fail → error toast in frame | Done |
| ocrRegion i18n keys + OcrSettings watch copy | Done |
| ocrConstants shared knobs | Done |
| Follow multi-point hwnd probe | Done |
| Double-click content refreshes OCR | Done |
| ocrRegion pin/follow i18n titles | Done |
| Ctrl+C copies translation in frame | Done |
| Double-click ignores select-text | Done |
| Resize handle aria/title | Done |
| Same-lang → soft hint not red error | Done |
| Error panel semi-transparent over content | Done |
| Watch enable → immediate sample | Done |
| Refresh clears error + shows loading | Done |
| Follow fail → soft hint | Done |
| I7: similar text still updates boxes | Done |
| Translate fail keepError on frame | Done |
| Follow poll serialize (no stale rect) | Done |
| Resize busy → pendingRegion coalesce | Done |
| Selector pointer capture for mouseup | Done |
| Selector drag via refs (no setState race) | Done |
| Frame accepts I7 geometry-only updates | Done |
| Watch enable no double OCR | Done |
| Close clears last translation cache | Done |
| Empty OCR does not lock fingerprint | Done |
| Follow click-through always restored | Done |
| Refresh toolbar hint | Done |

**Still open (larger product work):** freeform selection; always-on per-line API for many lines; ink.

**Then:** freeform/ink after rectangle path proven.

Freeform/ink **after** rectangle + watch path is boringly correct.

## Relation to invariants

`OCR_INVARIANTS.md` I1–I7 remain non-negotiable. Strategy features must not violate hide-before-capture or toolbar height sync.
