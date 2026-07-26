// ─── OCR Quality Check Utilities ─────────────────────────────────────────────
// Pure functions for evaluating OCR text quality. No React dependencies.

/** Minimum similarity threshold to consider two texts "the same" (I7). */
export const OCR_TEXT_SIMILARITY_SKIP = 0.92;
const SIMILARITY_THRESHOLD = OCR_TEXT_SIMILARITY_SKIP;

/** Texts shorter than this are considered too short */
const MIN_TEXT_LENGTH = 2;

/** Number of recent texts to check for jitter patterns */
export const JITTER_WINDOW = 5;

/** After this many consecutive empty results, suppress overlay updates */
export const MAX_CONSECUTIVE_EMPTY = 3;

/** Collapse whitespace for OCR comparison. */
export function normalizeOcrText(text: string): string {
  return text.replace(/\s+/g, ' ').trim();
}

/**
 * Fallback fingerprint when Rust pixel hash is unavailable (tests / IPC fail).
 * Samples base64 head/mid/tail — weaker than `imageDataUrlFingerprint` (24×24 luma).
 */
export function imageFingerprint(dataUrlOrBase64: string): string {
  const raw = dataUrlOrBase64.includes(',')
    ? dataUrlOrBase64.slice(dataUrlOrBase64.indexOf(',') + 1)
    : dataUrlOrBase64;
  if (!raw) return '';
  let h = raw.length >>> 0;
  const n = raw.length;
  const zones = [
    [0, Math.min(n, 2048)],
    [Math.max(0, Math.floor(n / 2) - 1024), Math.min(n, Math.floor(n / 2) + 1024)],
    [Math.max(0, n - 2048), n],
  ] as const;
  for (const [a, b] of zones) {
    const step = Math.max(16, Math.floor((b - a) / 256) || 16);
    for (let i = a; i < b; i += step) {
      h = Math.imul(h ^ raw.charCodeAt(i), 16777619) >>> 0;
    }
  }
  return `${n}:${h.toString(16)}`;
}

/** Character-level similarity between two strings (0..1). */
export function textSimilarity(a: string, b: string): number {
  if (a === b) return 1;
  if (!a || !b) return 0;
  const maxLen = Math.max(a.length, b.length);
  if (maxLen === 0) return 1;
  let matches = 0;
  let bi = 0;
  for (let ai = 0; ai < a.length && bi < b.length; ai++) {
    const idx = b.indexOf(a[ai], bi);
    if (idx !== -1) {
      matches++;
      bi = idx + 1;
    }
  }
  return matches / maxLen;
}

/** Check if text is likely OCR noise (random single chars, symbols). */
function isNoisyText(text: string): boolean {
  if (text.length < 3) return true;
  const alphanum = text.replace(/[^a-zA-Z0-9\u4e00-\u9fff]/g, '');
  if (alphanum.length / text.length < 0.3) return true;
  return false;
}

/** Check if recent texts form a jitter pattern (oscillating between similar results). */
function isJittery(recentTexts: string[]): boolean {
  if (recentTexts.length < JITTER_WINDOW) return false;
  const last = recentTexts.slice(-JITTER_WINDOW);
  const unique = new Set(last);
  if (unique.size <= 2 && last.length >= JITTER_WINDOW) {
    return true;
  }
  return false;
}

export interface QualityResult {
  ok: boolean;
  reason: string;
  score: number;
}

/**
 * Evaluate the quality of OCR text against recent history.
 * @param text - The OCR result to evaluate
 * @param lastText - The previous OCR result for similarity comparison
 * @param recentTexts - Recent OCR results for jitter detection
 */
export function checkQuality(text: string, lastText: string, recentTexts: string[]): QualityResult {
  if (!text || text.trim().length === 0) {
    return { ok: false, reason: 'empty', score: 0 };
  }
  const trimmed = text.trim();
  if (trimmed.length < MIN_TEXT_LENGTH) {
    return { ok: false, reason: 'too_short', score: 0.1 };
  }
  if (isNoisyText(trimmed)) {
    return { ok: false, reason: 'noisy', score: 0.2 };
  }
  if (isJittery([...recentTexts, trimmed])) {
    return { ok: false, reason: 'jitter', score: 0.3 };
  }
  if (lastText) {
    const sim = textSimilarity(trimmed, lastText);
    if (sim >= SIMILARITY_THRESHOLD) {
      return { ok: false, reason: 'similar', score: sim };
    }
  }
  return { ok: true, reason: '', score: 1.0 };
}
