/**
 * Typed labels for OCR region-frame cross-window events.
 * Keeps OcrScreenshotTranslator / OcrRegionFrame event names in one place.
 */

export const OCR_REGION_LABEL = 'ocr-region-frame';

export const OcrRegionEvents = {
  updateData: 'ocr-region-update-data',
  loading: 'ocr-region-loading',
  error: 'ocr-region-error',
  continuousState: 'ocr-region-continuous-state',
  close: 'ocr-region-close',
  ready: 'ocr-region-ready',
  refresh: 'ocr-region-refresh',
  langChange: 'ocr-region-lang-change',
  engineChange: 'ocr-region-engine-change',
  followToggle: 'ocr-region-follow-toggle',
  pinToggle: 'ocr-region-pin-toggle',
  resize: 'ocr-region-resize',
  move: 'ocr-region-move',
} as const;

export type OcrRegionEventName = (typeof OcrRegionEvents)[keyof typeof OcrRegionEvents];

export type OcrDisplayMode = 'translation' | 'source' | 'image';
