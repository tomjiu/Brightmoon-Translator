# OCR Monitor: Known Limitations and Usage Notes

## Overview

OCR Monitor captures a fixed screen region, runs OCR on the captured image, translates the recognized text, and displays the result in an overlay window. This document describes the current capability boundaries.

---

## Suitable Use Cases

These scenarios work well with the current implementation:

- **PDF readers** (SumatraPDF, Adobe Reader): Clean rendered text, high recognition accuracy, static content benefits from adaptive interval
- **Plain text editors** (Notepad, WordPad): Native GDI rendering gives the best OCR quality
- **Electron apps** (VS Code, Obsidian): Standard window behavior, reliable HWND tracking
- **Browser webpage reading** (Chrome, Edge): Works well for semi-static content like articles and documentation
- **E-book readers**: Similar to PDF, clean text rendering

## Unsuitable Use Cases

These scenarios have significant issues:

- **Video subtitles with rapid changes**: The adaptive interval doesn't help when text changes every 1-2 seconds; CPU stays at base interval
- **Live chat with high message frequency**: Similar to video subtitles — constant new content prevents adaptive slowdown
- **Games in exclusive fullscreen**: Screen capture may fail or capture black frames
- **UAC-elevated windows**: UIPI (User Interface Privilege Isolation) prevents capture of elevated process windows from a non-elevated process
- **Remote desktop / VNC**: Capture works on the local viewer window, not the remote content
- **Windows with per-pixel alpha** (`WS_EX_LAYERED`): Some layered windows render with transparency that confuses OCR

---

## Known Limitations

### 1. Fixed Region vs True Embedded Translation

The OCR approach captures a rectangular screen area and runs recognition on the pixel data. This is fundamentally different from reading text directly from the application's accessibility tree or DOM:

- **No structural information**: Line breaks, paragraphs, tables, and code blocks are lost in pixel-to-text conversion
- **Font size matters**: Very small fonts (< 10px) or decorative fonts reduce accuracy significantly
- **Background complexity**: Text over images, gradients, or video backgrounds has lower recognition quality
- **CJK mixed content**: Mixed Chinese/Japanese/Korean with English or numbers can cause segmentation errors

### 2. OCR Misrecognition

Common OCR errors include:

- Similar characters: `0` vs `O`, `1` vs `l` vs `I`, `rn` vs `m`
- Punctuation: CJK periods, commas, and quotation marks may be misread
- Special characters: Mathematical symbols, emoji, and Unicode special characters are often garbled
- Line merging: Adjacent lines may merge into one, or one line may split into two

### 3. High-Frequency Refresh Overhead

Each OCR cycle involves:
1. Screen capture (GDI/BitBlt) — ~5-15ms
2. OCR processing (Tesseract/custom) — ~50-200ms depending on region size
3. Translation API call — ~100-500ms depending on engine and network
4. Overlay update — ~1-5ms

At the default 2-second interval, this is acceptable. At 500ms (minimum), the combined latency of capture + OCR + translate may exceed the interval, causing the skip-if-busy guard to drop cycles.

**Recommendation**: For rapidly changing content, use intervals of 1-2 seconds or more.

### 4. Window Follow Instability

The follow loop polls `GetWindowRect` every 500ms and adjusts the OCR region by the delta. Issues:

- **DPI scaling**: On multi-monitor setups with different DPI, coordinates may drift
- **Window animations**: Minimize/maximize animations cause transient coordinate jumps
- **Layered windows**: Some windows report incorrect or zero-size rects
- **Fullscreen transitions**: Going from windowed to fullscreen may break the offset calculation

### 5. Overlay State Management

- **Pin state** is not persisted across app restarts
- **Click-through** is now persisted in config, but the overlay window itself doesn't remember it if destroyed and recreated
- **Multiple monitors**: Overlay position is in screen coordinates; moving the target window between monitors works, but the overlay may appear on the wrong monitor briefly

### 6. Quality Filter Limitations

The quality filter uses:
- Minimum text length (2 chars)
- Noise detection (alphanumeric ratio < 30%)
- Jitter detection (oscillating between 2-3 variants)
- Similarity threshold (0.92) to debounce unchanged text

These heuristics work for most cases but:
- May skip valid short translations (e.g., single words)
- Jitter detection can false-positive on legitimate alternating content
- Similarity threshold may be too aggressive for content with minor corrections

---

## Future Optimization Directions

### Short Term
- **Region size hint**: Warn user if selected region is very large (OCR time scales with pixel count)
- **Interval recommendation**: Suggest interval based on observed cycle time
- **Overlay memory**: Persist overlay position and pin state across sessions

### Medium Term
- **UIA text extraction**: For apps that support UI Automation, read text directly instead of OCR (much faster and more accurate)
- **Incremental OCR**: Only re-OCR the portion of the region that changed (requires image diffing)
- **Configurable quality thresholds**: Let users tune sensitivity, jitter window, and similarity threshold

### Long Term
- **Hybrid approach**: Use UIA when available, fall back to OCR for unsupported windows
- **GPU-accelerated OCR**: Use ONNX Runtime with GPU for faster recognition
- **Translation memory**: Cache OCR+translation pairs to avoid re-translating unchanged content
- **Smart region detection**: Auto-detect text regions within the selected area

---

## Debugging

When OCR Monitor isn't working as expected:

1. **Check the Diagnostics panel** (expandable in the monitoring view):
   - `Capture ms`: Should be < 20ms. High values indicate screen capture issues.
   - `OCR ms`: Should be < 200ms for typical regions. High values suggest large region or slow engine.
   - `Translate ms`: Depends on engine and network. > 2s may indicate network issues.
   - `Quality score`: 1.0 = accepted, < 0.5 = filtered out. Check skip reason.
   - `Skip reason`: `similar` = text hasn't changed, `empty` = no text detected, `noisy` = garbled result, `jitter` = oscillating text.

2. **Check console logs**: OCR cycle details are logged with `[OCR]` prefix.

3. **Common issues**:
   - "No text recognized": Region may be too small, or background is complex
   - "Overlay flickers": Should be fixed with incremental updates; report if still occurring
   - "Window doesn't follow": Check if the target window reports correct HWND via `detect_foreground_hwnd`
