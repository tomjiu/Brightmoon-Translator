/**
 * Align a bulk translation string onto OCR lines (index order).
 * Prefer newline 1:1; else token-boundary packing by source length weights.
 * Avoid raw char-slicing mid-grapheme (was scrambling EN↔CJK overlays).
 */

export function splitTranslationParts(translated: string): string[] {
  return translated.replace(/\n+$/, '').split(/\r?\n/);
}

/** Grapheme/token units: CJK chars alone, otherwise runs of non-space or single spaces. */
export function tokenizeForAlign(text: string): string[] {
  const chars = Array.from(text);
  const tokens: string[] = [];
  let buf = '';
  const flush = () => {
    if (buf) {
      tokens.push(buf);
      buf = '';
    }
  };
  for (const ch of chars) {
    if (/\s/.test(ch)) {
      flush();
      tokens.push(ch);
      continue;
    }
    // CJK / fullwidth → one token each
    if (/[\u3000-\u9fff\uf900-\ufaff]/.test(ch)) {
      flush();
      tokens.push(ch);
      continue;
    }
    buf += ch;
  }
  flush();
  return tokens;
}

/**
 * Map full translation onto `sourceLines` (non-empty OCR texts in order).
 * Returns one string per source line.
 */
export function alignTranslationToLines(sourceLines: string[], translated: string): string[] {
  const n = sourceLines.length;
  if (n === 0) return [];
  const out = translated.trim();
  if (!out) return sourceLines.map(() => '');

  const parts = splitTranslationParts(out).map((p) => p.trimEnd());
  // Exact line count match (engine preserved newlines)
  if (parts.length === n) {
    return parts.map((p, i) => p || sourceLines[i]);
  }
  if (n === 1) {
    return [out];
  }
  // Drop blank split artifacts then retry 1:1 (common LLM trailing/blank lines)
  const compact = parts.map((p) => p.trim()).filter((p) => p.length > 0);
  if (compact.length === n) {
    return compact;
  }
  // Single blob or mismatched multi-line → pack by source weights at token boundaries
  return packBySourceWeights(sourceLines, out);
}

function packBySourceWeights(sourceLines: string[], translated: string): string[] {
  const n = sourceLines.length;
  const weights = sourceLines.map((s) => Math.max(1, s.trim().length));
  const totalW = weights.reduce((a, b) => a + b, 0);
  const tokens = tokenizeForAlign(translated);
  const totalTok = tokens.length;
  if (totalTok === 0) return sourceLines.map(() => '');

  const result: string[] = [];
  let cursor = 0;
  for (let i = 0; i < n; i++) {
    if (i === n - 1) {
      result.push(tokens.slice(cursor).join('').trim());
      break;
    }
    const share = weights[i] / totalW;
    // soft factor slightly over-allocate early lines so last isn't starved of glue
    let target = Math.max(1, Math.round(share * totalTok * 1.05));
    const remainingLines = n - i;
    const remainingTok = totalTok - cursor;
    target = Math.min(target, Math.max(1, remainingTok - (remainingLines - 1)));
    const slice = tokens
      .slice(cursor, cursor + target)
      .join('')
      .trim();
    result.push(slice || sourceLines[i]);
    cursor += target;
  }
  return result;
}
