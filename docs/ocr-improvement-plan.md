# OCR Screenshot Translation Improvement Plan

## Issues Identified

### Critical Issues (Causing Bugs)
1. **Second screenshot no-translation bug**
   - Root cause: `captureAndTranslate` catches errors internally without re-throwing
   - When OCR fails, region frame stays open but shows no data
   - Outer catch in `ocr-screenshot-selected` never fires

2. **OcrRegionFrame.tsx StrictMode listener leak**
   - Status: FIXED
   - Old `unlistenPromise.then(fn => fn())` pattern replaced with `cancelled` flag

3. **Continuous state mismatch between windows**
   - Status: FIXED
   - OcrRegionFrame defaults to `true`, main window defaults to `false`
   - Fixed by changing OcrRegionFrame default to `false`

4. **ocrSource dependency causing excessive listener re-registration**
   - Status: FIXED
   - `ocrSource` in useEffect dependency array caused re-registration on every OCR result
   - Fixed by using `hasOcrRef` instead

### Performance Issues
5. **Black screen flash during screenshot**
   - Root cause: Webview loading time before React renders
   - Background color fix helps but doesn't eliminate flash
   - Need alternative approach: pass snapshot data directly to selector

6. **Slow OCR recognition**
   - Youdao OCR API network latency (~1-2s)
   - WinRT OCR temp file I/O overhead
   - tesseract.js fallback is very slow (~5-10s)

### Code Quality Issues
7. **Error handling in captureAndTranslate**
   - Catches errors internally, doesn't re-throw
   - Prevents proper error propagation to caller

8. **sendToRegionFrame swallows errors**
   - Line 88 catches all errors silently
   - If region frame isn't ready, data is lost

9. **Hardcoded Youdao OCR credentials**
   - app_key and app_secret are hardcoded in capture.rs
   - Should be configurable via config store

## Improvement Plan

### Phase 1: Fix Critical Bugs (Priority: HIGH)

#### 1.1 Fix captureAndTranslate error handling
**File**: `src/components/OcrScreenshotTranslator.tsx`

**Current behavior**:
```typescript
try {
  // ... OCR and translation logic ...
} catch (err) {
  setError(String(err));  // Error caught, not re-thrown
} finally {
  busyRef.current = false;
}
```

**Problem**: Error is caught but not re-thrown, so caller can't handle it.

**Fix**: Re-throw error after setting state:
```typescript
try {
  // ... OCR and translation logic ...
} catch (err) {
  setError(String(err));
  throw err;  // Re-throw for caller to handle
} finally {
  busyRef.current = false;
}
```

#### 1.2 Improve error handling in ocr-screenshot-selected listener
**File**: `src/components/OcrScreenshotTranslator.tsx`

**Current behavior** (lines 294-303):
```typescript
try {
  await captureAndTranslate(region);
} catch (err) {
  if (cancelled) return;
  console.error("[OCR] captureAndTranslate failed after selection:", err);
  setError(String(err));
  setStatus("error");
  try { await invoke("close_ocr_region_frame"); } catch {}
  await getCurrentWindow().show();
}
```

**Problem**: This catch block never fires because `captureAndTranslate` catches internally.

**Fix**: After fixing 1.1, this catch block will properly handle errors. Add user-visible error feedback.

#### 1.3 Add error state to OcrRegionFrame
**File**: `src/components/OcrRegionFrame.tsx`

**Current behavior**: Shows nothing when no data received.

**Fix**: Add loading/error state display:
- Show "等待数据..." when first opened
- Show "OCR 识别失败" if no data received within timeout
- Add retry button

### Phase 2: Performance Improvements (Priority: MEDIUM)

#### 2.1 Eliminate black screen flash
**File**: `src/components/OcrScreenshotSelector.tsx`

**Current approach**: Async load screenshot snapshot after window opens.

**Better approach**: Pass snapshot data via URL hash or postMessage:
- Encode snapshot as base64 in URL hash (limited to ~2MB)
- Or use Tauri's `eval` to inject data after window load
- Or use a shared state mechanism (localStorage equivalent)

#### 2.2 Optimize OCR pipeline
**File**: `src/services/ocr.ts`

**Current chain**: Youdao → WinRT → tesseract.js

**Optimizations**:
- Run Youdao and WinRT in parallel (first success wins)
- Cache OCR results by image hash to avoid re-processing
- Pre-load tesseract.js worker in background

#### 2.3 Reduce WinRT OCR temp file overhead
**File**: `src-tauri/src/commands/capture.rs`

**Current approach**: Write temp file, read with WinRT, delete file.

**Better approach**: Use in-memory stream if WinRT supports it (check API).

### Phase 3: Code Quality Improvements (Priority: LOW)

#### 3.1 Make Youdao OCR credentials configurable
**File**: `src-tauri/src/commands/capture.rs`

**Current**: Hardcoded `app_key` and `app_secret`.

**Fix**: Read from config store or environment variables.

#### 3.2 Improve error messages
**Files**: Multiple

**Current**: Generic error strings.

**Fix**: Add error codes and user-friendly messages in Chinese.

#### 3.3 Add retry logic for network failures
**File**: `src-tauri/src/commands/capture.rs`

**Current**: Single attempt per endpoint.

**Fix**: Add exponential backoff retry (max 3 attempts).

## Execution Order

1. [x] Fix OcrRegionFrame.tsx listener pattern (DONE)
2. [x] Fix continuous state mismatch (DONE)
3. [x] Fix ocrSource dependency issue (DONE)
4. [ ] Fix captureAndTranslate error re-throw
5. [ ] Add error state to OcrRegionFrame
6. [ ] Test second screenshot flow end-to-end
7. [ ] Optimize OCR pipeline (parallel execution)
8. [ ] Reduce black screen flash duration
9. [ ] Make credentials configurable
10. [ ] Add retry logic

## Testing Checklist

- [ ] First screenshot works correctly
- [ ] Second screenshot works correctly (after closing first)
- [ ] Region frame shows loading state
- [ ] Region frame shows error state on OCR failure
- [ ] Continuous refresh works
- [ ] Manual refresh works
- [ ] Language change triggers re-OCR
- [ ] Region resize triggers re-OCR
- [ ] Region drag updates position
- [ ] Close button works correctly
- [ ] No black screen flash (or minimal duration)
- [ ] OCR accuracy is acceptable (Youdao > WinRT > tesseract.js)
