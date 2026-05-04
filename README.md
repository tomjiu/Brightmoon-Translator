# Brightmoon Translator

Brightmoon Translator is a desktop-first translation toolkit built with `Tauri + Rust + React`, with an optional browser extension bridge.

It is designed for three translation workflows:

- desktop text translation
- browser translation
- selection-based translation with overlay output

The browser extension can run independently. If the desktop app is available, the extension can optionally reuse the desktop translation pipeline, glossary, blacklist, and cache through a local bridge.

## Current Status

This project is currently an engineering preview, not a fully polished production release.

Implemented foundations:

- desktop translation pipeline with routing, glossary, blacklist, cache, and history
- selection translation via `UI Automation -> clipboard fallback`
- overlay system with levels, pin, click-through, and follow modes
- browser extension with selection, page translation, and optional desktop bridge
- shared browser protocol models between desktop and extension

Still evolving:

- app-specific embedded adapters for `Electron / WebView2 / CEF`
- stronger hook/invasive translation capabilities
- OCR production readiness
- broader compatibility and diagnostics across desktop apps

## Architecture

### Desktop App

- `src-tauri/`: Rust backend, translation engines, routing, cache, capabilities, local API bridge
- `src/`: React frontend, settings UI, glossary/history pages, OCR UI, overlay controls

### Browser Extension

- `extension/`: Chromium-style extension for selection translation, page translation, popup settings
- independent fallback mode with local engines
- optional desktop bridge through `http://127.0.0.1:60828`

## Core Features

### Desktop

- text input translation
- selection translation
- replace-translate
- glossary and blacklist
- translation cache and metrics
- overlay levels `L1 / L2 / L3`
- configurable global hotkeys

### Browser

- selection translation
- page translation
- popup translation
- optional sync of glossary and blacklist from desktop
- desktop bridge with automatic fallback to local extension engines

## Tech Stack

- `Tauri 2`
- `Rust`
- `React 18`
- `TypeScript`
- `Vite`
- `Zustand`
- `Tailwind CSS`
- `tesseract.js`

## Development

### Prerequisites

- Node.js 18+
- Rust toolchain
- Tauri build environment
- Windows is currently the primary desktop target

### Install

```bash
npm install
```

### Frontend Check

```bash
npm run check
npm run lint
```

### Frontend Dev

```bash
npm run dev
```

### Desktop Dev

```bash
npm run tauri dev
```

### Build

```bash
npm run build
npm run tauri build
```

## Repository Layout

```text
.
├── src/                      # React frontend
├── src-tauri/                # Rust backend
├── extension/                # Browser extension
├── docs/                     # Project docs
├── FEATURES.md               # Feature tracking notes
└── ARCHITECTURE.md           # Architecture notes
```

## Roadmap

- improve UI Automation reliability across more desktop apps
- strengthen replace-translate stability
- add better overlay refresh and target-bound tracking
- expand optional desktop bridge capabilities for the browser extension
- add app-specific adapters for `Electron / WebView2 / CEF`
- evaluate deeper hook/embedded translation paths

## License

No license has been added yet.

