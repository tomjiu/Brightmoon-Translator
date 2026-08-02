// OCR screenshot helper utilities — extracted from OcrScreenshotTranslator
import { listen } from '@tauri-apps/api/event';
import { emitToRegion, emitToRegionId, OcrMainEvents, OcrRegionEvents } from './ocrRegionProtocol';
import type { DictionaryResult } from '../types';

/** true = frame listeners registered; false = timeout (do not OCR blindly). */
export function waitForOcrRegionFrameReady(timeoutMs: number, regionId?: string): Promise<boolean> {
  return new Promise((resolve) => {
    let done = false;
    let unlisten: (() => void) | undefined;
    const finish = (ok: boolean) => {
      if (done) return;
      done = true;
      window.clearTimeout(timer);
      unlisten?.();
      resolve(ok);
    };
    const timer = window.setTimeout(() => finish(false), timeoutMs);
    // P0 fix: the frame answers `pingReady` (base event name for every region).
    // M3 renamed the ready event for non-default regions, but main always
    // pings via pingReady — frame must listen on pingReady, and main must ping
    // the REGION's window (not just the legacy default window).
    void listen(OcrMainEvents.frameReady, () => finish(true)).then((fn) => {
      unlisten = fn;
      if (done) {
        fn();
        return;
      }
      const ping = regionId
        ? emitToRegionId(regionId, OcrRegionEvents.pingReady)
        : emitToRegion(OcrRegionEvents.pingReady);
      void ping.catch(() => undefined);
    });
  });
}

/** Wait until frame applied session-reset (or timeout). */
export function waitForSessionResetAck(timeoutMs: number): Promise<boolean> {
  return new Promise((resolve) => {
    let done = false;
    let unlisten: (() => void) | undefined;
    const finish = (ok: boolean) => {
      if (done) return;
      done = true;
      window.clearTimeout(timer);
      unlisten?.();
      resolve(ok);
    };
    const timer = window.setTimeout(() => finish(false), timeoutMs);
    void listen(OcrMainEvents.sessionResetAck, () => finish(true)).then((fn) => {
      unlisten = fn;
      if (done) {
        fn();
        return;
      }
      void emitToRegion(OcrRegionEvents.sessionReset).catch(() => undefined);
    });
  });
}

/** Selector freeze painted + window shown (or timeout). */
export function waitForOcrScreenshotReady(timeoutMs: number): Promise<boolean> {
  return new Promise((resolve) => {
    let done = false;
    let unlisten: (() => void) | undefined;
    const finish = (ok: boolean) => {
      if (done) return;
      done = true;
      window.clearTimeout(timer);
      unlisten?.();
      resolve(ok);
    };
    const timer = window.setTimeout(() => finish(false), timeoutMs);
    void listen('ocr-screenshot-ready', () => finish(true)).then((fn) => {
      unlisten = fn;
      if (done) fn();
    });
  });
}

export function withTimeout<T>(promise: Promise<T>, timeoutMs: number, message: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = window.setTimeout(() => {
      reject(new Error(message));
    }, timeoutMs);
    promise.then(
      (value) => {
        window.clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        window.clearTimeout(timer);
        reject(error instanceof Error ? error : new Error(String(error)));
      },
    );
  });
}

/** Mirror Rust `dictionary::is_single_word` for OCR dict-first gate. */
export function isOcrSingleWord(text: string): boolean {
  const trimmed = text.trim();
  if (!trimmed || trimmed.length > 50) return false;
  const hasCjk = /[\u4e00-\u9fff\u3400-\u4dbf\uf900-\ufaff]/.test(trimmed);
  if (hasCjk) {
    let cjkCount = 0;
    for (const c of trimmed) {
      if (/[\u4e00-\u9fff\u3400-\u4dbf\uf900-\ufaff]/.test(c)) cjkCount += 1;
    }
    return cjkCount >= 1 && cjkCount <= 10;
  }
  return !/\s/.test(trimmed);
}

export function hasRealDictionaryMeanings(results: DictionaryResult[]): boolean {
  return results.some((r) =>
    r.meanings.some((m) => {
      if ((m.partOfSpeech || '').includes('未找到')) return false;
      return m.definitions.some((d) => {
        const def = d.definition || '';
        return def.length > 0 && !def.includes('未找到');
      });
    }),
  );
}

/** Compact dict body for OCR line overlay (word + phonetic + defs). */
export function formatOcrDictBody(word: string, results: DictionaryResult[]): string | null {
  if (!hasRealDictionaryMeanings(results)) return null;
  const r0 = results[0];
  const lines: string[] = [];
  if (r0.phonetic) {
    lines.push(`${word}  ${r0.phonetic}`);
  } else {
    lines.push(word);
  }
  for (const m of r0.meanings.slice(0, 6)) {
    if ((m.partOfSpeech || '').includes('未找到')) continue;
    const defs = m.definitions
      .slice(0, 3)
      .map((d) => d.definition)
      .filter((d) => d && !d.includes('未找到'));
    if (defs.length === 0) continue;
    for (const d of defs) {
      const pos = m.partOfSpeech || '';
      let line = !pos || pos === '基本释义' || pos === '扩展释义' ? d : `[${pos}] ${d}`;
      if ([...line].length > 120) {
        line = `${[...line].slice(0, 118).join('')}…`;
      }
      lines.push(line);
      if (lines.length >= 8) break;
    }
    if (lines.length >= 8) break;
  }
  return lines.length > 1 ? lines.join('\n') : null;
}