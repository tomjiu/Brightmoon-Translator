# Moon Translator Plugin API

Complete reference for developing plugins for Moon Translator.

---

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Plugin Types](#plugin-types)
- [Plugin Modes](#plugin-modes)
- [Manifest Schema](#manifest-schema)
- [HTTP API Contracts](#http-api-contracts)
  - [Translation Plugin API](#translation-plugin-api)
  - [OCR Plugin API](#ocr-plugin-api)
  - [TTS Plugin API](#tts-plugin-api)
  - [DataSource Plugin API](#datasource-plugin-api)
- [Sandbox IPC Protocol](#sandbox-ipc-protocol)
- [Plugin SDK](#plugin-sdk)
  - [Installation](#installation)
  - [Base Classes](#base-classes)
  - [Debug Tools](#debug-tools)
- [Example Plugins](#example-plugins)
- [Permissions](#permissions)
- [Error Handling](#error-handling)
- [Best Practices](#best-practices)

---

## Architecture Overview

Moon Translator supports two plugin modes:

1. **HTTP Mode** (simple) -- Plugin runs as an independent HTTP server. The host sends requests to the plugin's endpoint.
2. **Sandbox Mode** (advanced) -- Plugin runs as a subprocess managed by the host. Communication happens via stdin/stdout JSON-line IPC.

```
+--------------------------------------------------+
|                Moon Translator Host               |
|  +--------------------------------------------+  |
|  |           Plugin Manager                    |  |
|  |  - Scans plugins/ directory                 |  |
|  |  - Reads manifest.json                      |  |
|  |  - Registers plugin engines                 |  |
|  +--------------------------------------------+  |
|                       |                           |
|           +-----------+-----------+               |
|           |                       |               |
|     HTTP Request            stdin/stdout          |
|           |                       |               |
|  +----------------+    +-------------------+      |
|  | Plugin (HTTP)  |    | Plugin (Sandbox)  |      |
|  | External proc  |    | Managed subprocess|      |
|  +----------------+    +-------------------+      |
+--------------------------------------------------+
```

---

## Plugin Types

| Type | Description | Status |
|------|-------------|--------|
| `translation` | Translates text between languages | Implemented |
| `ocr` | Recognizes text from images | Implemented |
| `tts` | Converts text to speech audio | Implemented |
| `datasource` | Provides translation memory / glossary / dictionary data | Implemented |

---

## Plugin Modes

### HTTP Mode

The simplest way to create a plugin. You run an HTTP server and point the manifest at it.

**Pros**: Simple, language-agnostic, independent lifecycle.
**Cons**: No sandbox isolation, no automatic restart.

### Sandbox Mode

The plugin runs as a managed subprocess. The host spawns the process, communicates via stdin/stdout IPC, and monitors health.

**Pros**: Process isolation, automatic restart, resource limits, health monitoring.
**Cons**: More complex, requires using the SDK or implementing the IPC protocol manually.

---

## Manifest Schema

Every plugin must have a `manifest.json` file in its root directory.

### Full Schema

```jsonc
{
  // Required fields
  "name": "My Plugin",              // Unique name (alphanumeric, spaces, hyphens, underscores)
  "version": "1.0.0",               // Semantic version
  "type": "translation",            // "translation" | "ocr" | "tts" | "datasource"

  // Optional metadata
  "description": "Short description",
  "author": "Author Name",
  "enabled": true,                   // Default: true
  "minApiVersion": 1,               // Minimum host API version (default: 1)
  "updateUrl": "https://...",       // URL to check for updates

  // Permissions
  "permissions": ["network"],        // Required permissions

  // Sandbox config (only for sandbox mode)
  "sandbox": {
    "enabled": true,
    "maxMemoryMb": 256,             // Max memory in MB (default: 256)
    "maxCpuPercent": 50,            // Max CPU % (default: 50)
    "maxConnections": 10,           // Max network connections (default: 10)
    "maxRestarts": 3                // Max restart attempts (default: 3)
  },
  "entryPoint": "index.js",         // Path to executable (sandbox mode only)

  // Type-specific config (one of these based on "type")
  "translation": {
    "endpoint": "http://localhost:3001/translate",
    "supportedLanguages": [["en", "zh"], ["zh", "en"]],  // Empty = all
    "headers": { "Authorization": "Bearer xxx" },
    "supportsStream": false,
    "supportsBatch": false
  },
  "ocr": {
    "endpoint": "http://localhost:3002/ocr",
    "supportedLanguages": ["en", "zh", "ja"],
    "supportsDetailed": true
  },
  "tts": {
    "endpoint": "http://localhost:3003/tts",
    "supportedLanguages": ["en", "zh"],
    "voices": [
      { "id": "default", "name": "Default", "lang": "en", "gender": "neutral" }
    ]
  },
  "datasource": {
    "endpoint": "http://localhost:3004/data",
    "capabilities": ["lookup", "memory"]
  }
}
```

### Directory Structure

```
plugins/
  my-plugin/
    manifest.json          # Required
    index.js               # Entry point (sandbox mode) or HTTP server
    package.json           # If Node.js
    ...                    # Other plugin files
```

Plugins are stored in `%APPDATA%/moontranslator/plugins/` (Windows) or `~/.config/moontranslator/plugins/` (Linux/macOS).

---

## HTTP API Contracts

### Translation Plugin API

#### `POST /translate`

Translate a single text.

**Request**:
```json
{
  "text": "Hello World",
  "from": "en",
  "to": "zh",
  "context": [
    { "source": "Previous sentence.", "translation": "上一句的翻译。" }
  ]
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `text` | string | Yes | Source text to translate |
| `from` | string | Yes | Source language code (`"auto"` for auto-detect) |
| `to` | string | Yes | Target language code |
| `context` | array | No | Previous sentence pairs for context-aware translation |

**Response** (200):
```json
{
  "translated": "你好世界"
}
```

**Error Response** (400/500):
```json
{
  "error": "Translation failed: connection timeout"
}
```

#### `POST /translate/batch` (optional)

Translate multiple texts in one request.

**Request**:
```json
{
  "texts": ["Hello", "World"],
  "from": "en",
  "to": "zh"
}
```

**Response** (200):
```json
{
  "translations": ["你好", "世界"]
}
```

#### Streaming (SSE) (optional)

If `supportsStream` is true in the manifest, the plugin should support Server-Sent Events:

**Request**: Same as `/translate` with `"stream": true`

**Response**: SSE stream
```
data: {"chunk": "你"}
data: {"chunk": "好"}
data: {"chunk": "世界"}
data: [DONE]
```

---

### OCR Plugin API

#### `POST /ocr`

Recognize text from an image.

**Request**:
```json
{
  "image": "base64-encoded-image-data...",
  "lang": "en",
  "detailed": true
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `image` | string | Yes | Base64-encoded image data |
| `lang` | string | No | Language hint |
| `detailed` | boolean | No | Return detailed results with bounding boxes |

**Response** (200):
```json
{
  "text": "Recognized text from the image",
  "lines": [
    {
      "text": "Recognized text",
      "boundingBox": { "x": 10, "y": 20, "width": 200, "height": 30 },
      "confidence": 0.95
    }
  ]
}
```

---

### TTS Plugin API

#### `POST /tts`

Convert text to speech.

**Request**:
```json
{
  "text": "Hello World",
  "lang": "en",
  "voice": "default",
  "speed": 1.0,
  "format": "mp3"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `text` | string | Yes | Text to synthesize |
| `lang` | string | No | Language code |
| `voice` | string | No | Voice ID |
| `speed` | number | No | Playback speed (0.5 - 2.0) |
| `format` | string | No | Audio format: `mp3`, `wav`, `ogg`, `aac` |

**Response** (200):
```json
{
  "audio": "base64-encoded-audio-data...",
  "format": "mp3",
  "durationMs": 2500
}
```

#### `GET /tts/voices`

List available voices.

**Response** (200):
```json
{
  "voices": [
    { "id": "default", "name": "Default Voice", "lang": "en", "gender": "neutral" }
  ]
}
```

---

### DataSource Plugin API

#### `POST /lookup`

Search translation memory, glossary, or dictionary.

**Request**:
```json
{
  "query": "Hello",
  "from": "en",
  "to": "zh",
  "threshold": 0.8,
  "limit": 10
}
```

**Response** (200):
```json
{
  "matches": [
    {
      "source": "Hello World",
      "target": "你好世界",
      "similarity": 0.95,
      "metadata": { "engine": "Google", "timestamp": 1700000000 }
    }
  ]
}
```

#### `POST /memory/add`

Add an entry to translation memory.

**Request**:
```json
{
  "source": "Hello",
  "target": "你好",
  "from": "en",
  "to": "zh",
  "engine": "Google"
}
```

**Response** (200): `{ "ok": true }`

---

## Sandbox IPC Protocol

When running in sandbox mode, plugins communicate with the host via newline-delimited JSON over stdin/stdout.

### Host-to-Plugin Messages

```jsonc
// Initialize the plugin
{ "type": "Init", "payload": { "pluginName": "My Plugin", "pluginDir": "/path/to/plugin", "permissions": ["network"] } }

// Request translation
{ "type": "Translate", "payload": { "requestId": "uuid-1", "text": "Hello", "from": "en", "to": "zh" } }

// Health check
{ "type": "Ping", "payload": { "requestId": "uuid-2" } }

// Graceful shutdown
{ "type": "Shutdown" }
```

### Plugin-to-Host Messages

```jsonc
// Initialization complete
{ "type": "InitOk" }

// Translation result
{ "type": "TranslateResult", "payload": { "requestId": "uuid-1", "result": { "ok": "你好" } } }
{ "type": "TranslateResult", "payload": { "requestId": "uuid-1", "result": { "err": "Translation failed" } } }

// Pong (response to Ping)
{ "type": "Pong", "payload": { "requestId": "uuid-2" } }

// Error report
{ "type": "Error", "payload": { "requestId": "uuid-1", "message": "Something went wrong" } }

// Permission check request
{ "type": "CheckPermission", "payload": { "requestId": "uuid-3", "permission": "network" } }
```

### Environment Variables

When spawning a sandboxed plugin, the host sets these environment variables:

| Variable | Description |
|----------|-------------|
| `MOON_PLUGIN_NAME` | Plugin name |
| `MOON_PLUGIN_DIR` | Plugin directory path |
| `MOON_PLUGIN_PERMISSIONS` | JSON array of granted permissions |
| `MOON_PLUGIN_MAX_MEMORY_MB` | Memory limit |
| `MOON_PLUGIN_MAX_CPU_PERCENT` | CPU limit |

---

## Plugin SDK

The official TypeScript SDK simplifies plugin development.

### Installation

```bash
# In your plugin directory
npm install @moontranslator/plugin-sdk
```

Or copy the `sdk/` directory into your project.

### Base Classes

The SDK provides base classes for each plugin type:

#### TranslationPlugin

```typescript
import { TranslationPlugin } from "@moontranslator/plugin-sdk";

class MyTranslator extends TranslationPlugin {
  protected async translate(text: string, from: string, to: string): Promise<string> {
    // Your translation logic
    const resp = await fetch("https://api.example.com/translate", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ text, source: from, target: to }),
    });
    const data = await resp.json();
    return data.translated;
  }
}

new MyTranslator().run();
```

#### OcrPlugin

```typescript
import { OcrPlugin } from "@moontranslator/plugin-sdk";

class MyOcr extends OcrPlugin {
  protected async recognize(image: string, lang?: string, detailed?: boolean) {
    // Your OCR logic
    return { text: "recognized text", lines: [] };
  }
}

new MyOcr().run();
```

#### TtsPlugin

```typescript
import { TtsPlugin } from "@moontranslator/plugin-sdk";

class MyTts extends TtsPlugin {
  protected async synthesize(text: string, lang?: string, voice?: string) {
    // Your TTS logic
    return { audio: "base64-audio", format: "mp3", durationMs: 2000 };
  }
}

new MyTts().run();
```

#### DatasourcePlugin

```typescript
import { DatasourcePlugin } from "@moontranslator/plugin-sdk";

class MyDatasource extends DatasourcePlugin {
  protected async lookup(query: string, from: string, to: string) {
    // Your lookup logic
    return [{ source: "Hello", target: "你好", similarity: 1.0 }];
  }
}

new MyDatasource().run();
```

### Debug Tools

The SDK includes built-in debugging utilities:

```typescript
import { PluginLogger, ApiCallTracer, PerfTimer, HealthMonitor } from "@moontranslator/plugin-sdk";

// Structured logging (writes to stderr, captured by host)
const logger = new PluginLogger("MyPlugin");
logger.info("Starting up", { port: 3001 });

// API call tracing
const tracer = new ApiCallTracer();
const finish = tracer.startRequest("MyPlugin", "POST", "/translate", "text=5chars");
finish(200, "translated=3chars");

// Performance timing
const timer = new PerfTimer();
timer.start("translate");
// ... do work ...
const ms = timer.end("translate");

// Health monitoring
const health = new HealthMonitor();
health.recordRequest();
health.recordError("timeout");
const status = health.getStatus();
// { healthy: true, uptimeMs: 12345, memoryMb: 42, ... }
```

### JSON Schema Files

The SDK includes JSON Schema files for validating manifests and API contracts:

- `sdk/schemas/manifest.schema.json` -- Plugin manifest validation
- `sdk/schemas/translation-api.schema.json` -- Translation API contracts
- `sdk/schemas/ocr-api.schema.json` -- OCR API contracts
- `sdk/schemas/tts-api.schema.json` -- TTS API contracts
- `sdk/schemas/datasource-api.schema.json` -- DataSource API contracts

---

## Example Plugins

See the `examples/` directory in the repository:

### `examples/simple-translator/`

A complete Node.js translation plugin that works in both HTTP and sandbox modes. It supports:

- Single text translation
- Batch translation
- Configurable upstream API
- Mock mode for testing

**Quick start**:

```bash
cd examples/simple-translator
npm install express
node index.js
```

Copy to plugins directory:
```bash
cp -r examples/simple-translator %APPDATA%/moontranslator/plugins/simple-translator/
```

---

## Permissions

Plugins must declare required permissions in their manifest. The host checks these at runtime.

| Permission | Description |
|------------|-------------|
| `network` | Make outbound HTTP requests |
| `fileRead` | Read files from the plugin directory |
| `fileWrite` | Write files to the plugin directory |
| `clipboard` | Access the system clipboard |
| `process` | Spawn child processes |
| `history` | Access translation history |
| `ocr` | Use OCR capabilities |
| `tts` | Use TTS capabilities |

---

## Error Handling

### HTTP Error Responses

All plugin HTTP endpoints should return errors in this format:

```json
{
  "error": "Human-readable error message",
  "code": "MACHINE_READABLE_CODE"
}
```

Use appropriate HTTP status codes:
- `200`: Success
- `400`: Invalid request (missing fields, bad format)
- `403`: Permission denied
- `500`: Internal plugin error

### Sandbox Error Reporting

In sandbox mode, send errors via the IPC protocol:

```json
{ "type": "Error", "payload": { "message": "Something went wrong" } }
```

The host logs all errors and displays them in the plugin debug UI.

---

## Best Practices

1. **Validate input** -- Always check required fields before processing.
2. **Return meaningful errors** -- Include context in error messages.
3. **Use timeouts** -- Set reasonable timeouts for upstream API calls.
4. **Handle edge cases** -- Empty text, unsupported languages, very long text.
5. **Log to stderr** -- In sandbox mode, stdout is reserved for IPC.
6. **Respect resource limits** -- Check `MOON_PLUGIN_MAX_MEMORY_MB` etc.
7. **Graceful shutdown** -- Clean up resources when receiving Shutdown message.
8. **Use semantic versioning** -- Increment version correctly for each release.
9. **Document your plugin** -- Include a README with setup instructions.
10. **Test with curl** -- Verify your HTTP endpoints manually before integrating.
