# Moon Translator

Desktop translation tool with browser extension, overlay UI, and multi-engine support.

## Prerequisites

- **Rust** (stable, 1.70+)
- **Node.js** (18+)
- **Windows 10/11** (UI Automation API required)

## Build

### Desktop App (Tauri)

```bash
cd src-tauri
cargo build --release
```

The executable will be at `src-tauri/target/release/moontranslator.exe`.

### Browser Extension

```bash
cd extension
node build.js
```

The built extension will be in `extension/dist/`. Load it in Chrome via `chrome://extensions` > Load unpacked.

## Development

```bash
# Run desktop app in dev mode
cd src-tauri
cargo run

# Run tests
cargo test

# Check for warnings
cargo check
```

## Architecture

```
moontranslator/
├── src-tauri/              # Rust desktop app (Tauri v2)
│   ├── src/
│   │   ├── capabilities/   # Feature implementations
│   │   │   ├── adapters.rs # Embedded app adapter trait
│   │   │   ├── browser_translation.rs
│   │   │   ├── input_replacement.rs
│   │   │   ├── selection_translation.rs
│   │   │   └── platform/   # Windows-specific code
│   │   ├── engine/         # Translation engines (Google, LLM, Youdao, etc.)
│   │   ├── models/         # Data types and protocol definitions
│   │   ├── overlay/        # Translation overlay UI
│   │   ├── selection/      # Text selection providers (UIA, clipboard)
│   │   └── services/       # Translation service layer
│   └── Cargo.toml
├── extension/              # Browser extension (Chrome MV3 / Firefox)
│   ├── background/         # Service worker
│   ├── content/            # Content scripts
│   └── popup/              # Extension popup UI
└── scripts/                # Utility scripts
```

### Key Components

- **Selection Providers**: UIA (priority 10) → Clipboard (priority 100). UIA tries TextPattern first, then ValuePattern with selection cross-reference, then children walk.
- **Embedded App Adapters**: Electron, WebView2, CEF — currently defer to generic UIA chain with diagnostics logging.
- **Overlay System**: Transient/Interactive/Pinned states with cursor-following and target-bounds tracking.
- **Desktop Bridge**: Local HTTP API at `127.0.0.1:60828` for browser extension communication.

## Configuration

Config file location: `%APPDATA%/moontranslator/config.json`

```json
{
  "default_from": "auto",
  "default_to": "zh",
  "overlay_level": 1,
  "overlay_follow_mode": "cursor",
  "llm": {
    "api_keys": ["sk-..."],
    "base_url": "https://api.deepseek.com/v1",
    "model": "deepseek-chat"
  }
}
```

## Cleanup

```powershell
powershell -ExecutionPolicy Bypass -File scripts/cleanup-temps.ps1
```

Removes temporary files (`tmpclaude-*`, `*.tmp`).
