/**
 * Moon Translator Plugin SDK - Type Definitions
 *
 * All types used by the plugin API contract.
 */

// ---------------------------------------------------------------------------
// Plugin Manifest
// ---------------------------------------------------------------------------

/** Plugin capability type */
export type PluginType = "translation" | "ocr" | "tts" | "datasource";

/** Permissions a plugin can declare in its manifest */
export type PluginPermission =
  | "network"
  | "fileRead"
  | "fileWrite"
  | "clipboard"
  | "process"
  | "history"
  | "ocr"
  | "tts";

/** Sandbox resource limits for a plugin subprocess */
export interface SandboxConfig {
  enabled?: boolean;
  maxMemoryMb?: number;
  maxCpuPercent?: number;
  maxConnections?: number;
  maxRestarts?: number;
}

/** Translation plugin-specific configuration */
export interface TranslationConfig {
  endpoint: string;
  supportedLanguages?: [string, string][];
  headers?: Record<string, string>;
  supportsStream?: boolean;
  supportsBatch?: boolean;
}

/** OCR plugin-specific configuration */
export interface OcrConfig {
  endpoint: string;
  supportedLanguages?: string[];
  supportsDetailed?: boolean;
}

/** TTS plugin-specific configuration */
export interface TtsConfig {
  endpoint: string;
  supportedLanguages?: string[];
  voices?: TtsVoice[];
}

/** DataSource plugin-specific configuration */
export interface DatasourceConfig {
  endpoint: string;
  capabilities?: ("translate" | "lookup" | "memory" | "glossary")[];
}

/** Plugin manifest (manifest.json) */
export interface PluginManifest {
  name: string;
  version: string;
  description?: string;
  author?: string;
  type: PluginType;
  enabled?: boolean;
  minApiVersion?: number;
  updateUrl?: string;
  permissions?: PluginPermission[];
  sandbox?: SandboxConfig;
  entryPoint?: string;
  translation?: TranslationConfig;
  ocr?: OcrConfig;
  tts?: TtsConfig;
  datasource?: DatasourceConfig;
}

/** Plugin info returned by the host */
export interface PluginInfo {
  manifest: PluginManifest;
  path: string;
}

// ---------------------------------------------------------------------------
// Translation API
// ---------------------------------------------------------------------------

/** Translation request sent to a translation plugin endpoint */
export interface TranslateRequest {
  text: string;
  from: string;
  to: string;
  context?: { source: string; translation: string }[];
}

/** Translation response from a translation plugin endpoint */
export interface TranslateResponse {
  translated: string;
}

/** Batch translation request */
export interface BatchTranslateRequest {
  texts: string[];
  from: string;
  to: string;
}

/** Batch translation response */
export interface BatchTranslateResponse {
  translations: string[];
}

/** Streaming chunk from a translation plugin */
export interface StreamChunk {
  chunk: string;
  done?: boolean;
}

// ---------------------------------------------------------------------------
// OCR API
// ---------------------------------------------------------------------------

/** OCR request sent to an OCR plugin endpoint */
export interface OcrRequest {
  image: string;
  lang?: string;
  detailed?: boolean;
}

/** A single recognized text line with optional position info */
export interface OcrLine {
  text: string;
  boundingBox?: { x: number; y: number; width: number; height: number };
  confidence?: number;
}

/** OCR response from an OCR plugin endpoint */
export interface OcrResponse {
  text: string;
  lines?: OcrLine[];
}

// ---------------------------------------------------------------------------
// TTS API
// ---------------------------------------------------------------------------

/** TTS request sent to a TTS plugin endpoint */
export interface TtsRequest {
  text: string;
  lang?: string;
  voice?: string;
  speed?: number;
  format?: "mp3" | "wav" | "ogg" | "aac";
}

/** TTS response from a TTS plugin endpoint */
export interface TtsResponse {
  audio: string;
  format?: string;
  durationMs?: number;
}

/** A voice option returned by a TTS plugin */
export interface TtsVoice {
  id: string;
  name: string;
  lang: string;
  gender?: "male" | "female" | "neutral";
}

// ---------------------------------------------------------------------------
// DataSource API
// ---------------------------------------------------------------------------

/** Lookup request for translation memory / glossary */
export interface LookupRequest {
  query: string;
  from: string;
  to: string;
  threshold?: number;
  limit?: number;
}

/** A single match result */
export interface Match {
  source: string;
  target: string;
  similarity?: number;
  metadata?: Record<string, unknown>;
}

/** Lookup response */
export interface LookupResponse {
  matches: Match[];
}

/** Add entry to translation memory */
export interface MemoryAddRequest {
  source: string;
  target: string;
  from: string;
  to: string;
  engine?: string;
}

// ---------------------------------------------------------------------------
// Sandbox IPC Protocol
// ---------------------------------------------------------------------------

/** Messages from host to plugin subprocess */
export type HostToPluginMessage =
  | { type: "Init"; payload: { pluginName: string; pluginDir: string; permissions: string[] } }
  | { type: "Translate"; payload: { requestId: string; text: string; from: string; to: string } }
  | { type: "Ping"; payload: { requestId: string } }
  | { type: "Shutdown" };

/** Messages from plugin subprocess to host */
export type PluginToHostMessage =
  | { type: "InitOk" }
  | { type: "TranslateResult"; payload: { requestId: string; result: { ok?: string; err?: string } } }
  | { type: "Pong"; payload: { requestId: string } }
  | { type: "Error"; payload: { requestId?: string; message: string } }
  | { type: "CheckPermission"; payload: { requestId: string; permission: string } };

// ---------------------------------------------------------------------------
// Plugin Runtime Status
// ---------------------------------------------------------------------------

export type PluginRunState = "stopped" | "running" | "crashed" | "restarting";

export interface PluginSandboxStatus {
  pluginName: string;
  pid?: number;
  state: PluginRunState;
  memoryUsageMb: number;
  cpuUsagePercent: number;
  restartCount: number;
  uptimeMs: number;
}

export interface PluginErrorLog {
  pluginName: string;
  timestamp: string;
  error: string;
}

export interface PluginUpdateInfo {
  hasUpdate: boolean;
  latestVersion: string;
}

// ---------------------------------------------------------------------------
// Debug / Tracing
// ---------------------------------------------------------------------------

export type ApiCallDirection = "request" | "response" | "error";

export interface ApiCallRecord {
  id: string;
  pluginName: string;
  direction: ApiCallDirection;
  method: string;
  url: string;
  timestamp: number;
  durationMs?: number;
  status?: number;
  requestSummary?: string;
  responseSummary?: string;
  error?: string;
}

export interface PluginLogEntry {
  pluginName: string;
  level: "debug" | "info" | "warn" | "error";
  message: string;
  timestamp: number;
}
