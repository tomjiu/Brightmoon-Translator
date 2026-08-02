/**
 * Typed labels for OCR region-frame cross-window events.
 * Keeps OcrScreenshotTranslator / OcrRegionFrame event names in one place.
 */

import { emitTo } from '@tauri-apps/api/event';
import type { OcrLineResult } from './ocr';

export const OCR_REGION_LABEL = 'ocr-region-frame';
export const OCR_MAIN_LABEL = 'main';

/**
 * Reserved legacy single-frame region id (M3). Its window label degrades to the
 * bare `OCR_REGION_LABEL` (no suffix) so M0/M1/M2 zero-change shim works.
 */
export const DEFAULT_REGION_ID = 'default';

/**
 * M3: Resolve the window label for a region id.
 * `"default"` → `ocr-region-frame` (legacy bare label); otherwise
 * `ocr-region-frame-{id}`.
 */
export function regionLabel(id: string): string {
  return id === DEFAULT_REGION_ID ? OCR_REGION_LABEL : `${OCR_REGION_LABEL}-${id}`;
}

/**
 * M3: Per-region event name. For the default id, keeps the legacy base name
 * (so the single-frame listeners stay unchanged); otherwise appends `-{id}`.
 */
export function regionEventName(base: string, id: string): string {
  return id === DEFAULT_REGION_ID ? base : `${base}-${id}`;
}

/**
 * M3: Per-region event name builders (BE→FE events). Default id keeps the
 * legacy un-suffixed names; non-default ids use the design's renamed events
 * (design §2.5): ready/text/error/mode. (Frame→main control events just get a
 * plain `-{id}` suffix — see `regionEventName`.)
 */
export const REGION_EVENTS_BY_ID = {
  ready: (id: string) => (id === DEFAULT_REGION_ID ? OcrMainEvents.frameReady : `ocr-region-ready-${id}`),
  text: (id: string) => (id === DEFAULT_REGION_ID ? OcrRegionEvents.updateData : `ocr-region-text-${id}`),
  error: (id: string) => regionEventName(OcrRegionEvents.error, id),
  mode: (id: string) => regionEventName(OcrRegionEvents.mode, id),
} as const;

/** Events emitted toward the region frame window. */
export const OcrRegionEvents = {
  updateData: 'ocr-region-update-data',
  loading: 'ocr-region-loading',
  error: 'ocr-region-error',
  continuousState: 'ocr-region-continuous-state',
  followState: 'ocr-region-follow-state',
  hint: 'ocr-region-hint',
  pingReady: 'ocr-region-ping-ready',
  sessionReset: 'ocr-region-session-reset',
  /** M3: per-region display mode push (image/source/translated). */
  mode: 'ocr-region-mode',
} as const;

/** Events emitted from the region frame toward main. */
export const OcrMainEvents = {
  frameReady: 'ocr-region-frame-ready',
  sessionResetAck: 'ocr-region-session-reset-ack',
  close: 'ocr-region-close',
  refresh: 'ocr-region-refresh',
  continuous: 'ocr-region-continuous',
  follow: 'ocr-region-follow',
  langChange: 'ocr-region-lang-change',
  engineChange: 'ocr-region-engine-change',
  positionChanged: 'ocr-region-position-changed',
  sizeChanged: 'ocr-region-size-changed',
} as const;

export type OcrRegionEventName = (typeof OcrRegionEvents)[keyof typeof OcrRegionEvents];
export type OcrMainEventName = (typeof OcrMainEvents)[keyof typeof OcrMainEvents];

export type OcrDisplayMode = 'translation' | 'source' | 'image';

/** Capture / frame geometry in CSS logical px (screen-relative). */
export interface OcrRegionRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

// ── Payloads: main → region frame ──────────────────────────────────────────

/** Full / partial OCR result push (preview or final). */
export interface OcrRegionUpdateData {
  screenshot: string;
  sourceText: string;
  translatedText: string;
  ocrLines: OcrLineResult[];
  lineTranslations: string[];
  sourceLang: string;
  targetLang: string;
  detectedLang?: string;
  refreshIntervalMs?: number;
  /** Crop / OCR image size in pixels — set early so lines align before img onLoad. */
  imageWidth?: number;
  imageHeight?: number;
  /** When true, do not clear existing error banner (translate failed after OCR). */
  keepError?: boolean;
}

export interface OcrRegionLoadingPayload {
  loading: boolean;
}

export interface OcrRegionErrorPayload {
  message: string;
}

export interface OcrRegionEnabledPayload {
  enabled: boolean;
}

export interface OcrRegionHintPayload {
  message: string;
}

// ── Payloads: region frame → main ──────────────────────────────────────────

export interface OcrRegionLangChangePayload {
  sourceLang: string;
  targetLang: string;
}

export interface OcrRegionEngineChangePayload {
  engineId: string;
  enabled?: boolean;
  promote?: boolean;
}

export type OcrRegionPositionPayload = OcrRegionRect;
export type OcrRegionSizePayload = OcrRegionRect;

// ── Timeouts for ready / session-reset waits ───────────────────────────────

/** Cold webview create; shortened after instant crop preview (was 2500). */
export const OCR_FRAME_READY_TIMEOUT_MS = 1200;

/** Wait for frame to apply session-reset before first OCR push. */
export const OCR_SESSION_RESET_ACK_TIMEOUT_MS = 200;

/** Selector freeze painted + window shown. */
export const OCR_SCREENSHOT_READY_TIMEOUT_MS = 8000;

// ── Emit helpers ───────────────────────────────────────────────────────────

/**
 * M3: Emit an event to a specific region frame window by region id.
 * Resolves the window label via `regionLabel(id)`.
 */
export function emitToRegionId(
  id: string,
  event: string,
  payload: unknown = null,
): Promise<void> {
  return emitTo(regionLabel(id), event, payload);
}

/** Emit toward the legacy default region frame window (bare label). */
export function emitToRegion(event: OcrRegionEventName, payload: unknown = null): Promise<void> {
  return emitToRegionId(DEFAULT_REGION_ID, event, payload);
}

export function emitToMain(event: OcrMainEventName, payload: unknown = null): Promise<void> {
  return emitTo(OCR_MAIN_LABEL, event, payload);
}
