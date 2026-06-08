# Translation Modes Guide

Brightmoon Translator provides three distinct translation modes for different use cases. This guide helps you choose the right mode.

---

## Quick Comparison

| Feature | Selection Translate | Replace Translate | OCR Monitor |
|---------|-------------------|-------------------|-------------|
| Hotkey | `Ctrl+Shift+Y` | `Ctrl+Shift+R` | UI button or hotkey |
| Reads text via | UIA / Clipboard | UIA / Clipboard | Screen capture + OCR |
| Output | Overlay popup | Replaces in-place | Overlay popup |
| Works with | Most text editors | Apps that accept Ctrl+V | Anything on screen |
| Best for | Reading translations | Editing/translating in-place | Watching live content |

---

## 1. Selection Translate

**What it does**: Reads the text you have selected in any application, translates it, and shows the result in a floating overlay window.

### How to use
1. Select text in any application
2. Press `Ctrl+Shift+Y` (configurable)
3. Translation appears in a popup near your selection

### How it works
- First tries **UI Automation (UIA)** to read selected text directly from the application's accessibility interface
- Falls back to **clipboard** (simulates Ctrl+C, reads clipboard, restores original content)
- Translates using your configured engine
- Shows result in an overlay positioned near the text (or near cursor if bounds unavailable)

### Suitable scenarios
- Reading foreign language text in documents, web pages, or code editors
- Quick lookups without leaving your current application
- Applications that expose text via UIA (most modern Windows apps)

### Unsuitable scenarios
- Applications that don't support UIA and block clipboard access (some games, elevated windows)
- When you need the translation to replace the original text (use Replace Translate instead)
- Continuously changing content (use OCR Monitor instead)

### Dependencies
- **UIA provider**: Windows UI Automation API. Works with most standard controls (Edit, Document, Text patterns). Confidence: 0.95
- **Clipboard provider**: Fallback. Simulates Ctrl+C via `SendInput`. Confidence: 0.7. May interfere with clipboard if restore fails.

### Current limitations
- UIA doesn't work with custom-rendered UIs (games, some Electron apps with canvas)
- Clipboard fallback has no selection bounds — overlay appears at cursor position
- If clipboard restore fails, your original clipboard content may be lost

---

## 2. Replace Translate

**What it does**: Reads your selected text, translates it, and replaces the selection with the translation in-place.

### How to use
1. Select text in any application
2. Press `Ctrl+Shift+R` (configurable)
3. The selected text is replaced with its translation

### How it works
- Reads selected text via UIA or clipboard (same as Selection Translate)
- Translates using your primary engine
- Copies translation to clipboard and simulates Ctrl+V to paste

### Suitable scenarios
- Translating text you're editing (documents, emails, chat messages)
- When you want the translation to become the active text
- Applications that accept standard Ctrl+V paste

### Unsuitable scenarios
- Read-only text fields or viewers (paste will fail)
- Terminal emulators or apps with custom paste handling
- Elevated (admin) windows when running as non-admin
- When clipboard access is locked by another application

### Dependencies
- **Selection**: Same UIA/clipboard chain as Selection Translate
- **Paste**: Win32 `SetClipboardData` + `SendInput` (Ctrl+V simulation)

### Current limitations
- Success depends on the target app accepting Ctrl+V
- If paste fails, a toast notification warns the user and an overlay shows the translation as fallback
- Clipboard content is briefly replaced — if restore fails, your original clipboard is lost

### Failure types
- **Hard failure**: `OpenClipboard` or `SetClipboardData` failed. The app's clipboard is locked. Toast error shown.
- **Soft failure**: `SendInput` returned 0 or paste wasn't confirmed. Toast warning shown and translation displayed in an overlay as fallback.
- **No selection**: No text was found to translate. Toast warning shown.

---

## 3. OCR Monitor

**What it does**: Captures a screen region repeatedly, recognizes text via OCR, translates changes, and shows results in an overlay.

### How to use
1. Go to the OCR page in the sidebar
2. Click "Select Monitor Region"
3. Drag to select the area you want to watch
4. Translation overlay appears next to the region

### How it works
- Captures the selected screen region at regular intervals (default 2s)
- Runs OCR to recognize text
- Filters out noise, jitter, and unchanged text
- Translates new text and updates the overlay
- Tracks the bound window — region follows when the window moves
- Auto-pauses when window is minimized, auto-resumes on restore

### Suitable scenarios
- Watching video subtitles and wanting real-time translation
- Reading PDFs or images where text can't be selected
- Monitoring chat windows or live feeds
- Any content that's visible on screen but not accessible via UIA/clipboard

### Unsuitable scenarios
- Content that changes faster than the OCR+translate cycle (every <1s)
- Very small text (< 10px) or decorative fonts
- Text over complex backgrounds (images, gradients, video)
- When you need pixel-perfect accuracy (OCR always has some error rate)

### Dependencies
- **Screen capture**: GDI BitBlt
- **OCR engine**: Tesseract or custom OCR service
- **Translation**: Via the configured translation engine
- **Window tracking**: HWND-based position polling at 500ms intervals

### Current limitations
- OCR accuracy depends on font size, contrast, and background complexity
- Adaptive interval slows down when content is static (up to 4x base interval)
- Window follow has ~500ms latency — fast movements may cause brief misalignment
- Only one region at a time
- CJK text with many punctuation marks may be filtered as "noisy"

---

## Choosing the Right Mode

**Use Selection Translate when:**
- You want to read a translation without modifying the original
- The text is selectable in the application
- You need quick, one-off translations

**Use Replace Translate when:**
- You're editing text and want to swap it with a translation
- The target application accepts standard paste (Ctrl+V)
- You're translating content you're writing (chat, email, document)

**Use OCR Monitor when:**
- The text is not selectable (images, video, PDF viewer without text mode)
- You want continuous translation of changing content
- You're watching subtitles, monitoring a chat, or reading a scanned document

**When in doubt, start with Selection Translate** — it works with the most applications and has the least side effects.
