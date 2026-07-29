/**
 * Typed labels for OCR region-frame cross-window events.
 * Keeps OcrScreenshotTranslator / OcrRegionFrame event names in one place.
 */

export const OCR_REGION_LABEL = 'ocr-region-frame';
export const OCR_MAIN_LABEL = 'main';

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
