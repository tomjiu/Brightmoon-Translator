// ─── OCR Quality Check Utilities ─────────────────────────────────────────────
// Pure functions for evaluating OCR text quality. No React dependencies.

/** Minimum similarity threshold to consider two texts "the same" */
const SIMILARITY_THRESHOLD = 0.92;

/** Texts shorter than this are considered too short */
const MIN_TEXT_LENGTH = 2;

/** Number of recent texts to check for jitter patterns */
export const JITTER_WINDOW = 5;

/** After this many consecutive empty results, suppress overlay updates */
export const MAX_CONSECUTIVE_EMPTY = 3;

/** Character-level similarity between two strings (0..1). */
function textSimilarity(a: string, b: string): number {
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
  const alphanum = text.replace(/[^a-zA-Z0-9\u4e00-\u9fff]/g, "");
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
export function checkQuality(
  text: string,
  lastText: string,
  recentTexts: string[]
): QualityResult {
  if (!text || text.trim().length === 0) {
    return { ok: false, reason: "empty", score: 0 };
  }
  const trimmed = text.trim();
  if (trimmed.length < MIN_TEXT_LENGTH) {
    return { ok: false, reason: "too_short", score: 0.1 };
  }
  if (isNoisyText(trimmed)) {
    return { ok: false, reason: "noisy", score: 0.2 };
  }
  if (isJittery([...recentTexts, trimmed])) {
    return { ok: false, reason: "jitter", score: 0.3 };
  }
  if (lastText) {
    const sim = textSimilarity(trimmed, lastText);
    if (sim >= SIMILARITY_THRESHOLD) {
      return { ok: false, reason: "similar", score: sim };
    }
  }
  return { ok: true, reason: "", score: 1.0 };
}
