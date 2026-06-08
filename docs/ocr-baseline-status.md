# OCR Baseline Status

Date: 2026-05-05
Based on: Code analysis + bug fixes (commit 7c1393b)

## Call Chain Summary

```
User selects region (OcrMonitor.tsx)
  → startMonitoring(region, interval) (useOcrMonitor.ts)
    → captureAndOcr(region) loop:
      1. captureScreen(x,y,w,h) → invoke("capture_screen") → Rust screenshots crate → base64 PNG
      2. ocrImage(base64) → tesseract.js Worker.recognize() → text
      3. checkQuality(text, lastText, recentTexts) → quality filter
      4. setSourceText + translate() → TranslationService (Rust)
      5. overlayRef.update(region, translatedText) → invoke("update_overlay"/"update_overlay_content")
    → scheduleNext(region) with adaptive delay
```

**OCR Engine**: tesseract.js (browser-side, chi_sim+eng)
**Screen Capture**: Rust `screenshots` crate
**Translation**: Rust `TranslationService` (multi-engine)
**Overlay**: Rust WebviewWindow with data:text/html URL

## Feature Status

| Feature | Code Status | Verified | Notes |
|---------|-------------|----------|-------|
| Region selection | ✅ Fixed | ❌ | Was using clientX/clientY, now uses screenX/screenY |
| Screen capture | ✅ Implemented | ❌ | Rust `capture_screen` command, screenshots crate |
| OCR recognition | ✅ Implemented | ❌ | tesseract.js chi_sim+eng, no preprocessing |
| Translation | ✅ Implemented | ❌ | Via translateStore, uses TranslationService |
| Overlay creation | ✅ Implemented | ❌ | `update_overlay` creates if not exists |
| Overlay content update | ✅ Implemented | ❌ | `update_overlay_content` via eval() |
| Overlay position update | ✅ Implemented | ❌ | `update_overlay_position` via set_position |
| Pause/Resume | ✅ Implemented | ❌ | userPausedRef + timer clear/restore |
| Auto-pause (visibility) | ✅ Implemented | ❌ | document.visibilitychange listener |
| Auto-pause (focus) | ✅ Implemented | ❌ | Tauri onFocusChanged listener |
| Window binding | ✅ Fixed | ❌ | Was firing onWindowRestored every tick, now transition-only |
| Window follow | ✅ Implemented | ❌ | 500ms polling, offset-based tracking |
| Minimize detection | ✅ Fixed | ❌ | Now tracks wasMinimized state |
| Click-through | ✅ Implemented | ❌ | `set_overlay_click_through` command |
| Pin toggle | ✅ Fixed | ❌ | Was always setting pinned=true, now syncs with Rust |
| Adaptive interval | ✅ Implemented | ❌ | base×2 after 5 no-change, base×4 after 10 |
| Quality filtering | ✅ Implemented | ❌ | empty/too_short/noisy/jitter/similar checks |
| Diagnostics panel | ✅ Implemented | ❌ | Shows captureMs, ocrMs, translateMs, qualityScore |

## Bugs Fixed (this session)

### Bug 1: Region coordinates (CRITICAL)
- **Problem**: `clientX/clientY` (viewport-relative) passed to `capture_screen` which expects screen coordinates
- **Impact**: Capture targets wrong area when Tauri window not at screen origin (0,0)
- **Fix**: Use `screenX/screenY` for region, add `cssX/cssY` for CSS display
- **Files**: `OcrMonitor.tsx`

### Bug 2: Auto-resume spam
- **Problem**: Follow loop called `onWindowRestored()` every 500ms tick (not just on minimize→restore)
- **Impact**: Duplicate capture cycles, wasted CPU, potential state corruption
- **Fix**: Added `wasMinimized` flag, only fire on transition
- **Files**: `ocrWindowBinding.ts`

### Bug 3: Pin state desync
- **Problem**: `togglePin` always set `pinned: true` regardless of Rust toggle result
- **Impact**: After 2 clicks, UI shows pinned but overlay is unpinned
- **Fix**: Use `invoke<boolean>("pin_overlay")` return value
- **Files**: `useOcrMonitor.ts`

## Known Issues (not fixed, not blocking)

| Issue | Severity | Description |
|-------|----------|-------------|
| No OCR preprocessing | Low | No scaling/binarization/denoising before tesseract |
| No confidence score | Low | tesseract.js data.confidence not exposed, quality is heuristic only |
| Dead Rust OCR stub | Low | `commands/ocr.rs` returns placeholder, never used |
| `ocrScreenRegion` unused | Low | Convenience function in ocr.ts, never called from monitoring |

## Verification Required

All features marked ❌ in the table above need actual UI testing. Key test scenarios:

1. **Basic OCR cycle**: Select region → see overlay with translated text
2. **Region accuracy**: Select region on second monitor / offset window → capture matches selection
3. **Pause/Resume**: Pause → resume → monitoring continues correctly
4. **Window binding**: Bind to target window → move window → region follows
5. **Click-through**: Enable → mouse passes through overlay
6. **Pin toggle**: Pin → unpin → state matches UI
7. **Auto-pause**: Switch to another app → monitoring pauses → return → resumes
