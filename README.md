# Moon Translator

Desktop translation tool with browser extension, overlay UI, and multi-engine support.

## Prerequisites

- **Rust** (stable, 1.70+)
- **Node.js** (18+)
- **Windows 10/11** (UI Automation API required)

## Build

### Desktop App (Tauri)

```bash
pnpm install
pnpm run tauri build
```

Tauri runs the configured frontend build first, then writes bundles under
`src-tauri/target/release/bundle/`.

### Browser Extension

```bash
cd extension
node build.js
```

The built extension will be in `extension/dist/`.
Load `extension/dist/chrome/` in Chrome/Edge via `chrome://extensions` > Load unpacked.
Load `extension/dist/firefox/manifest.json` in Firefox via `about:debugging`.

## Development

```bash
# Run desktop app in dev mode
pnpm install
pnpm run tauri dev

# Run tests
pnpm test
cd src-tauri && cargo test

# Check for warnings
pnpm run check
pnpm run lint
cd src-tauri && cargo check
```

## Docs

Canonical index: [`docs/README.md`](docs/README.md). Active work focus: [`docs/CURRENT_FOCUS.md`](docs/CURRENT_FOCUS.md). Feature checklist: [`FEATURES.md`](FEATURES.md).

## Architecture

```
moontranslator/
├── src/                    # React UI (Vite)
├── src-tauri/              # Rust / Tauri v2 (engine, OCR, hook, overlay)
├── extension/              # Chrome MV3 / Firefox
├── sdk/                    # Plugin SDK
├── docs/                   # Canonical docs (+ docs/archive/)
├── cloudflare-api/         # Planned multi-end API (not primary path)
└── scripts/                # Utilities (e.g. cleanup-temps.ps1)
```

Details: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

### Key Components

- **Selection Providers**: UIA (priority 10) → Clipboard (priority 100).
- **Overlay**: Transient/Interactive/Pinned; OCR region frame has hard invariants in `docs/OCR_INVARIANTS.md`.
- **Desktop Bridge**: Local HTTP API at `127.0.0.1:60828` for the browser extension.
- **Hook inject**: experimental until a real smoke pass; see `docs/CURRENT_FOCUS.md`.

## Configuration

Config file location: `%APPDATA%/moontranslator/config.json` (camelCase JSON).

```json
{
  "defaultFrom": "auto",
  "defaultTo": "zh",
  "overlayLevel": 1,
  "overlayFollowMode": "cursor",
  "apiServerEnabled": false,
  "apiServerPort": 60828,
  "llm": {
    "apiKeys": ["sk-..."],
    "baseUrl": "https://api.deepseek.com/v1",
    "model": "deepseek-chat"
  }
}
```

**Browser extension bridge:** desktop local API is **off by default**. Enable *API server* in settings (`apiServerEnabled`) so the extension can use `http://127.0.0.1:60828`; otherwise the extension falls back to its own engines.

## Cleanup

```powershell
powershell -ExecutionPolicy Bypass -File scripts/cleanup-temps.ps1
```

Removes temporary files (`tmpclaude-*`, `*.tmp`).
