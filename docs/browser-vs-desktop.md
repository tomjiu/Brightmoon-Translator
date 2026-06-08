# Browser Extension vs Desktop App

Moon Translator operates as two interconnected products: a **Tauri desktop application** and a **browser extension** (Chrome/Firefox). This document explains how they relate, when to use which, and how the bridge between them works.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                   Browser Extension                   │
│  ┌───────────┐  ┌──────────────┐  ┌───────────────┐ │
│  │  Selector  │  │Page Translator│  │ Hover Tooltip │ │
│  │  (Alt+T)   │  │ (full page)   │  │  (hover 300ms)│ │
│  └─────┬─────┘  └──────┬───────┘  └──────┬────────┘ │
│        │               │                  │          │
│  ┌─────▼───────────────▼──────────────────▼────────┐ │
│  │              Service Worker                       │ │
│  │  DesktopBridge → health check (30s)              │ │
│  │  Local engines: Google, Youdao, Microsoft, LLM…  │ │
│  └──────────────────┬──────────────────────────────┘ │
└─────────────────────┼───────────────────────────────┘
                      │ HTTP (localhost:60828)
┌─────────────────────▼───────────────────────────────┐
│                  Desktop App (Tauri)                  │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ API Server    │  │TranslationSvc│  │   Cache     │ │
│  │ REST + CORS   │  │ Multi-engine │  │  SQLite 72h │ │
│  └──────────────┘  └──────────────┘  └────────────┘ │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │   Glossary    │  │  Blacklist   │  │   History   │ │
│  └──────────────┘  └──────────────┘  └────────────┘ │
└─────────────────────────────────────────────────────┘
```

---

## Quick Comparison

| Feature | Desktop App | Browser Extension |
|---------|------------|-------------------|
| **Selection Translate** | `Ctrl+Shift+Y` — reads via UIA/clipboard, shows overlay | `Alt+T` or select text — shows in-page popup |
| **Replace Translate** | `Ctrl+Shift+R` — replaces text in-place via clipboard | Not supported |
| **OCR Monitor** | Screen capture + OCR on any region | Not supported |
| **Full Page Translate** | N/A (document viewers for PDF/EPUB/subtitle) | Translates all text nodes in the DOM |
| **Hover Translate** | N/A | Hover 300ms on any text element |
| **Translation engines** | All engines (LLM, Google, Baidu, Youdao, DeepL, DeepLX, Microsoft, Yandex) | Desktop engines via bridge, or local fallback (Google, Youdao, Microsoft, LLM, DeepL, DeepLX) |
| **Glossary** | Full CRUD, applied as pre-processing | Synced from desktop, applied as post-processing in local mode |
| **Blacklist** | Full management | Synced from desktop, exact-match protection |
| **Cache** | SQLite, 72-hour TTL | Desktop cache via bridge; no local cache |
| **History** | Full history with search/export | Not tracked locally |
| **Works offline** | Yes (engines with API keys) | Yes (local fallback engines) |

---

## When to Use the Desktop App

**Use the desktop app when:**

- You need **Selection Translate** (`Ctrl+Shift+Y`) — reads text from any application via UI Automation or clipboard, not just the browser
- You need **Replace Translate** (`Ctrl+Shift+R`) — replaces selected text in-place in any editor, terminal, or chat app
- You need **OCR Monitor** — continuous screen region capture + OCR for video subtitles, scanned PDFs, or any non-selectable content
- You need **document translation** — PDF, EPUB, and subtitle file viewers with page-by-page or batch translation
- You need **translation history** — searchable, exportable log of all translations
- You need **glossary/blacklist management** — full CRUD interface for protecting terms
- You're translating text in **non-browser applications** (VS Code, Word, Notepad, Telegram, etc.)

**The desktop app is the primary product.** It has access to all engines, the full cache, and can translate text from any Windows application.

---

## When to Use the Browser Extension

**Use the browser extension when:**

- You're reading a **foreign language webpage** and want the entire page translated in-place
- You want **hover translation** — hover over any text element to see a quick tooltip translation
- You want **selection translation without leaving the browser** — select text and see results in an in-page popup, without switching to the desktop overlay
- The desktop app is **not running** — the extension works standalone with local fallback engines
- You're on a **machine where you can't install the desktop app** — the extension runs independently

---

## How the Bridge Works

The browser extension communicates with the desktop app via a local HTTP API:

- **Endpoint:** `http://127.0.0.1:60828`
- **Health check:** `GET /health` every 30 seconds
- **Translation:** `POST /browser/translate` with selection or full-page payload
- **Glossary sync:** `GET /glossary` and `GET /blacklist` (manual, triggered from popup)

### Desktop-First, Local Fallback

The extension always tries the desktop bridge first:

1. **Desktop reachable:** Request goes to desktop → full pipeline (glossary pre-processing, multi-engine, cache, history)
2. **Desktop unreachable:** Falls back to local engines running in the service worker

The fallback is **automatic and transparent**. The user sees results either way — the difference is which engines and features are available.

### Feature Differences by Mode

| Capability | Via Desktop Bridge | Local Fallback |
|-----------|-------------------|----------------|
| Glossary | Pre-processing (source text replacement) | Post-processing (translated text replacement) |
| Blacklist | Full blacklist support | Exact-match only |
| Cache | SQLite 72h TTL | No cache |
| History | Recorded | Not recorded |
| Engines | All configured engines | Google, Youdao, Microsoft, DeepL, DeepLX, LLM |
| Multi-engine | Yes (configurable) | Yes (all enabled engines in parallel) |

---

## Glossary Asymmetry

There is a known difference in how glossary is applied:

- **Desktop path:** Glossary terms are replaced in the **source text** before translation. This gives the engine context to produce a more coherent result.
- **Extension local path:** Glossary terms are replaced in the **translated text** after translation. This is a simpler approach but may produce less natural results when the term's translation doesn't match the glossary entry exactly.

When the desktop bridge is reachable, glossary handling is consistent because the desktop's `TranslationService` handles it.

---

## Cache Architecture

| Layer | Location | TTL | Scope |
|-------|----------|-----|-------|
| Desktop | SQLite database | 72 hours | Shared across all clients (extension, API, desktop UI) |
| Extension | None | N/A | Each request hits the engine directly in local mode |

When using the desktop bridge, the extension benefits from the desktop's cache transparently. Re-translating the same text within 72 hours returns the cached result without hitting remote APIs.

In local fallback mode, there is no caching. Every translation request goes directly to the remote engine APIs.

---

## Setup

### Desktop App Only

1. Install and run the Moon Translator desktop app
2. The API server starts on port `60828` (configurable in Settings → API Server)
3. Translation works via hotkeys (`Ctrl+Shift+Y`, `Ctrl+Shift+R`) and the main window

### Desktop + Extension

1. Install and run the desktop app
2. Install the browser extension (Chrome Web Store or Firefox Add-ons)
3. The extension automatically detects the desktop app via health checks
4. (Optional) Click "Sync Glossary" in the extension popup to pull glossary and blacklist from the desktop

### Extension Only (No Desktop)

1. Install the browser extension
2. Configure engine API keys in the extension popup (for LLM, DeepL)
3. Free engines (Google, Youdao, Microsoft) work without configuration
4. The extension operates fully standalone

---

## Limitations

### Browser Extension Limitations

- **No replace translate:** Cannot replace text in-place in the browser (paste into text fields)
- **No OCR:** Cannot translate content in images, videos, or canvas elements
- **No cross-app translation:** Only works within the browser
- **Glossary is post-processing in local mode:** Less natural results than desktop's pre-processing approach
- **No translation cache in local mode:** Every request hits remote APIs
- **Page translation can break SPAs:** Heavy DOM manipulation may conflict with React/Vue/Angular apps

### Desktop App Limitations

- **No full page translate:** Cannot translate entire web pages in-place (use the extension for this)
- **No hover translation:** Only hotkey-triggered translation
- **Windows only:** Tauri app requires Windows for UIA and clipboard integration
- **Requires installation:** Cannot run on locked-down machines

---

## Recommended Workflow

For the best experience, use both products together:

1. **Desktop app** for daily translation work — selection translate, replace translate, OCR monitor, document viewing
2. **Browser extension** for reading foreign web pages — full page translate, hover translate, quick selection translate
3. **Glossary sync** keeps both in sync — manage terms in the desktop app, sync to the extension periodically

The desktop app acts as the **backend** (engines, cache, glossary, history) and the browser extension acts as a **frontend** for web content.
