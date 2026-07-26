/** Shared OCR pipeline knobs (keep in sync with UI copy in OcrSettings). */

/** Max non-empty OCR lines that use per-line translate API. */
export const OCR_PER_LINE_TRANSLATE_MAX = 5;

/** Concurrency for per-line translate batches. */
export const OCR_PER_LINE_CONCURRENCY = 3;

/** Floor for continuous / region-watch interval (ms). */
export const OCR_WATCH_INTERVAL_MIN_MS = 750;

/** Default region-watch interval (ms). */
export const OCR_WATCH_INTERVAL_DEFAULT_MS = 2000;
