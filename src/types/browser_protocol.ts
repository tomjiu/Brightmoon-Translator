/**
 * Shared browser translation protocol types.
 *
 * These types MUST match the Rust models in `src-tauri/src/models/browser_protocol.rs`.
 * Both the browser extension and desktop app reference these types for
 * wire-format compatibility.
 *
 * To update: edit the Rust model first, then mirror changes here.
 */

// ─── Request payloads (extension → desktop) ───────────────────────────

export interface ProtocolBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Selected text payload. Maps to `BrowserSelectionPayload` in Rust. */
export interface BrowserSelectionPayload {
  text: string;
  selector?: string;
  bounds?: ProtocolBounds;
  url: string;
  title: string;
}

/** A single text segment from a page. */
export interface PageSegment {
  selector: string;
  text: string;
  index: number;
}

/** Full page content payload. Maps to `BrowserPagePayload` in Rust. */
export interface BrowserPagePayload {
  url: string;
  title: string;
  segments: PageSegment[];
}

/** Hover element payload. Maps to `BrowserHoverPayload` in Rust. */
export interface BrowserHoverPayload {
  text: string;
  selector?: string;
  bounds?: ProtocolBounds;
  url: string;
  title: string;
}

// ─── Request wrapper (extension → desktop) ────────────────────────────

export type BrowserTranslateMode = "selection" | "full_page" | "hover";

export type BrowserTranslatePayload =
  | { type: "Selection"; data: BrowserSelectionPayload }
  | { type: "FullPage"; data: BrowserPagePayload }
  | { type: "Hover"; data: BrowserHoverPayload };

/** Unified translation request from browser extension to desktop. */
export interface BrowserTranslateRequest {
  mode: BrowserTranslateMode;
  payload: BrowserTranslatePayload;
  from?: string;
  to?: string;
  showOverlay: boolean;
  replaceInline: boolean;
}

// ─── Response types (desktop → extension) ─────────────────────────────

/** Single translation result from an engine. */
export interface TranslationResult {
  engine: string;
  text: string;
}

/** Translation response (mirrors Rust `TranslateResponse`). */
export interface TranslateResponse {
  results: TranslationResult[];
  detectedLanguage?: string;
}

/** Per-segment translation for full-page mode. */
export interface SegmentTranslation {
  selector: string;
  original: string;
  translated: string;
  index: number;
}

/** Successful translation response to browser extension. */
export interface BrowserTranslateResponse {
  mode: BrowserTranslateMode;
  response: TranslateResponse;
  overlayShown: boolean;
  replacedInline: boolean;
  segmentTranslations?: SegmentTranslation[];
}

// ─── Error response (desktop → extension) ─────────────────────────────

/** Structured translation error (mirrors Rust `TranslationError` enum). */
export type TranslationError =
  | { type: "NoEngine" }
  | { type: "AllEnginesFailed"; detail: { errors: string[] } }
  | { type: "EngineError"; detail: { engine: string; message: string } }
  | { type: "RateLimited"; detail: { engine: string; retryAfterMs?: number } }
  | { type: "InvalidInput"; detail: string }
  | { type: "ConfigError"; detail: string }
  | { type: "NetworkError"; detail: string }
  | { type: "CacheError"; detail: string }
  | { type: "PluginError"; detail: { name: string; message: string } }
  | { type: "StreamingNotSupported" }
  | { type: "Internal"; detail: string };

/** Error response to browser extension. */
export interface BrowserTranslateError {
  error: TranslationError;
  message: string;
}

// ─── Action payloads (desktop → extension) ────────────────────────────

/** Instruction to show an overlay in the browser page. */
export interface BrowserOverlayPayload {
  translated: string;
  source: string;
  bounds?: ProtocolBounds;
  level: number;
  dismissMs: number;
}

/** Instruction to replace text inline in the browser page. */
export interface BrowserReplacePayload {
  selector: string;
  translated: string;
  original: string;
}
