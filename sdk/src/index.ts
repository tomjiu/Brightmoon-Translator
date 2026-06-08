/**
 * @moontranslator/plugin-sdk
 *
 * Official SDK for developing Moon Translator plugins.
 *
 * @example Translation Plugin
 * ```ts
 * import { TranslationPlugin } from "@moontranslator/plugin-sdk";
 *
 * class MyTranslator extends TranslationPlugin {
 *   protected async translate(text: string, from: string, to: string): Promise<string> {
 *     // Your translation logic here
 *     return "translated text";
 *   }
 * }
 *
 * new MyTranslator().run();
 * ```
 *
 * @example OCR Plugin
 * ```ts
 * import { OcrPlugin } from "@moontranslator/plugin-sdk";
 *
 * class MyOcr extends OcrPlugin {
 *   protected async recognize(image: string, lang?: string) {
 *     return { text: "recognized text" };
 *   }
 * }
 *
 * new MyOcr().run();
 * ```
 */

// Plugin base classes
export { BasePlugin } from "./base-plugin.js";
export type { PluginInitData } from "./base-plugin.js";

export { TranslationPlugin } from "./translation-plugin.js";

export { OcrPlugin } from "./ocr-plugin.js";
export type { OcrResult } from "./ocr-plugin.js";

export { TtsPlugin } from "./tts-plugin.js";
export type { TtsResult } from "./tts-plugin.js";

export { DatasourcePlugin } from "./datasource-plugin.js";

// IPC
export { PluginIpc } from "./ipc.js";

// Logging
export { PluginLogger } from "./logger.js";
export type { LogLevel } from "./logger.js";

// Debug tools
export { ApiCallTracer, PerfTimer, HealthMonitor } from "./debug.js";
export type { HealthStatus } from "./debug.js";

// All types
export type {
  // Manifest
  PluginType,
  PluginPermission,
  SandboxConfig,
  TranslationConfig,
  OcrConfig,
  TtsConfig,
  DatasourceConfig,
  PluginManifest,
  PluginInfo,

  // Translation API
  TranslateRequest,
  TranslateResponse,
  BatchTranslateRequest,
  BatchTranslateResponse,
  StreamChunk,

  // OCR API
  OcrRequest,
  OcrLine,
  OcrResponse,

  // TTS API
  TtsRequest,
  TtsResponse,
  TtsVoice,

  // DataSource API
  LookupRequest,
  LookupResponse,
  Match,
  MemoryAddRequest,

  // IPC
  HostToPluginMessage,
  PluginToHostMessage,

  // Runtime
  PluginRunState,
  PluginSandboxStatus,
  PluginErrorLog,
  PluginUpdateInfo,

  // Debug
  ApiCallDirection,
  ApiCallRecord,
  PluginLogEntry,
} from "./types.js";
