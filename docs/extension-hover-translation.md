# Extension Hover Translation

## Overview

Hover translation shows a tooltip with the translated text when you hover over text elements on any webpage. It runs as a content script in the Moon Translator browser extension.

## Trigger Rules

| Rule | Value | Configurable |
|------|-------|-------------|
| Hover delay | 300ms | Yes (`hover.delay`) |
| Min text length | 2 chars | Yes (`hover.minTextLength`) |
| Max text length | 2000 chars | No |
| Modifier key | None (always on) | Yes (`hover.modifierKey`) |

### How it works

1. Mouse hovers over an element
2. After `hoverDelay` ms of continuous hover, the script walks up the DOM to find a meaningful text block (P, LI, TD, DIV, etc.)
3. Extracts visible text (skipping hidden elements, scripts, styles)
4. Sends `{ type: "translate" }` message to the service worker
5. Service worker translates via desktop bridge or local engines
6. Tooltip appears below the element with the result

### Element skipping

These elements are **never** triggered:

- Form controls: `INPUT`, `TEXTAREA`, `BUTTON`, `SELECT`
- Links: `A`
- Code: `CODE`, `PRE`
- Media: `SVG`, `CANVAS`, `VIDEO`, `AUDIO`, `IFRAME`, `EMBED`, `OBJECT`
- Hidden: `SCRIPT`, `STYLE`
- Interactive: any `contenteditable` element, elements with `role="textbox"` or `role="button"`, elements with `tabindex`

### Modifier key modes

| Mode | Behavior |
|------|----------|
| `none` | Hover always triggers (default) |
| `alt` | Only triggers while Alt key is held |
| `ctrl` | Only triggers while Ctrl/Cmd key is held |
| `shift` | Only triggers while Shift key is held |

When the modifier key is released, the tooltip hides immediately.

## Configuration

Settings are in the extension popup under "悬停翻译 (Hover)":

| Setting | Default | Description |
|---------|---------|-------------|
| Enable hover | On | Master switch for hover translation |
| Trigger delay | 300 | Milliseconds of hover before translating |
| Min text length | 2 | Ignore text shorter than this |
| Modifier key | None | Require a key to be held |

Changes take effect immediately via `chrome.storage.onChanged` listener — no page refresh needed.

## Bridge vs Local Fallback

The hover translator uses the same translation pipeline as all other extension features:

1. **Desktop bridge** (`http://127.0.0.1:60828`): If the Moon Translator desktop app is running, the service worker forwards the request to it. Desktop handles glossary, blacklist, caching, and multi-engine results internally.

2. **Local fallback**: If desktop is unreachable, the service worker uses browser-based engines (Google, Youdao, DeepL, etc.) directly from the content script's perspective — the content script doesn't know which path was used.

The tooltip shows a single translation result regardless of which path was used.

## Known Limitations

1. **Dynamic content**: On SPAs where content changes frequently, the hover target may change before the delay expires. The 300ms debounce helps but doesn't eliminate this.

2. **Overlapping elements**: If a text block contains many nested inline elements (e.g., `<span>` inside `<a>` inside `<p>`), hovering over the link skips the entire block because `A` is in SKIP_TAGS. The walk-up stops at the first interactive ancestor.

3. **Tooltip positioning**: The tooltip tries to stay within the viewport, but on pages with complex layouts (fixed headers, sidebars), it may overlap interactive elements.

4. **Scroll behavior**: The tooltip hides on scroll. This prevents stale positioning but means you can't read a tooltip while scrolling.

5. **No caching**: Each hover triggers a new translation request. Rapidly hovering over the same text block sends duplicate requests.

6. **Long text**: Text over 2000 characters is ignored to avoid slow translations and oversized tooltips.

## Unsuitable Sites/Elements

- **Code editors** (GitHub, CodePen): `CODE` and `PRE` are skipped, but surrounding UI may still trigger
- **Chat applications** (Slack, Discord web): Messages are often in interactive containers
- **Form-heavy pages**: Most form elements are skipped, but labels and descriptions may trigger
- **Image-heavy pages**: Text over images is not recognized (OCR is a separate feature)
- **PDF viewers**: In-browser PDF viewers use canvas, not selectable text

## Files

| File | Purpose |
|------|---------|
| `extension/content/hover-translator.js` | Content script — event handling, text extraction, tooltip |
| `extension/content/hover-translator.css` | Tooltip styling |
| `extension/background/service-worker.js` | Message handling, translation routing |
| `extension/popup/popup.html` | Settings UI |
