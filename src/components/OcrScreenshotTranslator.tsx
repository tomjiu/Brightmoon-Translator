import { useCallback, useEffect, useRef, useState } from 'react';
import { listen, emitTo } from '@tauri-apps/api/event';
import { safeInvoke, invokeOrThrow } from '../services/invoke';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { isTauriRuntime } from '../services/tauriRuntime';
import { useI18n } from '../i18n';
import {
  captureScreenshotRegion,
  cropScreenshotSnapshot,
  imageDataUrlFingerprint,
  ocrWithEngine,
  prepareScreenshotSnapshot,
  type ScreenshotSnapshotInfo,
  type ScreenshotRegion,
  type OcrResultDetailed,
} from '../services/ocr';
import { useConfigStore } from '../stores/configStore';
import type { AppConfig, TranslateResponse, DetectionResult } from '../types';
import { WindowBindingManager } from '../hooks/ocrWindowBinding';
import { normalizeOcrText, textSimilarity, OCR_TEXT_SIMILARITY_SKIP } from '../hooks/ocrQuality';
import {
  OCR_MIN_FRAME_WIDTH_CSS,
  OCR_SELECTION_PAD_PX,
  probeDataUrlImageSize,
} from './ocrRegionGeometry';
import { OCR_WATCH_INTERVAL_DEFAULT_MS, OCR_WATCH_INTERVAL_MIN_MS } from '../services/ocrConstants';

interface OcrScreenshotTranslatorProps {
  launchNonce?: number;
}

interface RegionRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Cold webview create; shortened after instant crop preview (was 2500). */
const OCR_FRAME_READY_TIMEOUT_MS = 1200;

/** true = frame listeners registered; false = timeout (do not OCR blindly). */
function waitForOcrRegionFrameReady(timeoutMs: number): Promise<boolean> {
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
    void listen('ocr-region-frame-ready', () => finish(true)).then((fn) => {
      unlisten = fn;
      if (done) {
        fn();
        return;
      }
      void emitTo('ocr-region-frame', 'ocr-region-ping-ready', null).catch(() => undefined);
    });
  });
}

/** Wait until frame applied session-reset (or timeout). */
function waitForSessionResetAck(timeoutMs: number): Promise<boolean> {
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
    void listen('ocr-region-session-reset-ack', () => finish(true)).then((fn) => {
      unlisten = fn;
      if (done) {
        fn();
        return;
      }
      void emitTo('ocr-region-frame', 'ocr-region-session-reset', null).catch(() => undefined);
    });
  });
}

/** Selector freeze painted + window shown (or timeout). */
function waitForOcrScreenshotReady(timeoutMs: number): Promise<boolean> {
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

function withTimeout<T>(promise: Promise<T>, timeoutMs: number, message: string): Promise<T> {
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

export default function OcrScreenshotTranslator({ launchNonce = 0 }: OcrScreenshotTranslatorProps) {
  const { t } = useI18n();
  const config = useConfigStore((state) => state.config);
  const updateConfig = useConfigStore((state) => state.updateConfig);
  const saveConfig = useConfigStore((state) => state.saveConfig);
  const isTauri = isTauriRuntime();
  // I1: continuous MUST default false (also forced off after selection). true here restarts the flicker loop.
  const ocrIntervalMs = Math.max(
    OCR_WATCH_INTERVAL_MIN_MS,
    config.ocrInterval ?? OCR_WATCH_INTERVAL_DEFAULT_MS,
  );
  const [continuous, setContinuous] = useState(false);

  // Refs so captureAndTranslate identity stays stable (avoids re-binding all listeners → double OCR).
  const ocrEngineRef = useRef(config.ocrEngine || 'winrt');
  const tRef = useRef(t);
  useEffect(() => {
    ocrEngineRef.current = config.ocrEngine || 'winrt';
  }, [config.ocrEngine]);
  useEffect(() => {
    tRef.current = t;
  }, [t]);

  // Refs for stable access inside callbacks/intervals
  const regionRef = useRef<RegionRect | null>(null);
  const snapshotInfoRef = useRef<ScreenshotSnapshotInfo | null>(null);
  const busyRef = useRef(false);
  const pendingRegionRef = useRef<RegionRect | null>(null);
  const continuousRef = useRef(false);
  const frameClosedRef = useRef(true);
  /** OCR session owns main visibility: hidden from session start until result close / cancel. */
  const ocrSessionActiveRef = useRef(false);
  const sessionIdRef = useRef(0);
  const lastOcrTextRef = useRef<string>('');
  /** Last successful translation for I7 geometry-only updates (skip re-translate). */
  const lastTranslatedRef = useRef<string>('');
  const lastLineTranslationsRef = useRef<string[]>([]);
  /** Skip OCR+translate when continuous capture image is unchanged (region watch). */
  const lastImageFpRef = useRef<string>('');
  /** Consecutive fingerprint skips — stretch continuous interval to reduce wakeups. */
  const consecutiveSkipRef = useRef(0);
  const hasOcrRef = useRef(false); // Track whether OCR has been performed (avoids ocrSource dependency)
  const sourceLangRef = useRef(config.defaultFrom);
  const targetLangRef = useRef(config.defaultTo);
  const followEnabledRef = useRef(false);
  const movingFrameRef = useRef(false);
  const windowBindingRef = useRef<WindowBindingManager | null>(null);
  const normalizeLang = (code: string) => {
    const c = (code || '').toLowerCase();
    if (c === 'zh-cn' || c === 'zh-hans' || c === 'zh_cn') return 'zh';
    if (c === 'zh-tw' || c === 'zh-hant') return 'zh-TW';
    return c || 'auto';
  };

  useEffect(() => {
    continuousRef.current = continuous;
  }, [continuous]);

  // Config may load after mount — keep OCR lang refs aligned with store defaults
  // until the region frame overrides them via ocr-region-lang-change.
  // Do not overwrite after user changed langs on the frame (hasOcr / explicit override).
  const langOverriddenRef = useRef(false);
  useEffect(() => {
    if (langOverriddenRef.current) return;
    if (config.defaultFrom) sourceLangRef.current = config.defaultFrom;
    if (config.defaultTo) targetLangRef.current = config.defaultTo;
  }, [config.defaultFrom, config.defaultTo]);

  // ---- Send data to the merged region frame ----
  const ocrIntervalMsRef = useRef(ocrIntervalMs);
  useEffect(() => {
    ocrIntervalMsRef.current = ocrIntervalMs;
  }, [ocrIntervalMs]);

  const sendToRegionFrame = useCallback(
    async (
      screenshot: string,
      ocrResult: OcrResultDetailed,
      translatedText: string,
      lineTranslations: string[] = [],
      detectedLangName?: string,
      imageNatural?: { width: number; height: number },
      /** When true, frame keeps existing error (translate fail after partial update). */
      keepError?: boolean,
    ) => {
      const payload = {
        screenshot,
        sourceText: ocrResult.text,
        translatedText,
        ocrLines: ocrResult.lines,
        lineTranslations,
        sourceLang: sourceLangRef.current,
        targetLang: targetLangRef.current,
        detectedLang: detectedLangName,
        refreshIntervalMs: ocrIntervalMsRef.current,
        imageWidth: imageNatural?.width,
        imageHeight: imageNatural?.height,
        keepError: !!keepError,
      };

      for (let attempt = 0; attempt < 3; attempt++) {
        try {
          await emitTo('ocr-region-frame', 'ocr-region-update-data', payload);
          return;
        } catch (err) {
          console.warn('[OCR] emitTo failed on attempt', attempt + 1, ':', err);
          if (attempt < 2) {
            await new Promise((resolve) => window.setTimeout(resolve, 80 * (attempt + 1)));
          }
        }
      }
      console.error('[OCR] emitTo failed after all attempts');
    },
    [],
  );

  // ---- Core OCR + Translate pipeline ----
  // If image is provided, skip capture (used when image was captured before region frame creation)
  const captureAndTranslate = useCallback(
    async (region: RegionRect, preCapturedImage?: string) => {
      if (busyRef.current) {
        pendingRegionRef.current = region;
        return;
      }
      busyRef.current = true;
      pendingRegionRef.current = null;
      const sessionId = sessionIdRef.current;
      let hidRegionFrame = false;

      try {
        let image: string;

        if (preCapturedImage) {
          // Use already-captured image (no need to hide/show region frame)
          image = preCapturedImage;
        } else {
          // Refresh + continuous: same physical-screen GDI path (STranslate/Luna style).
          // Never full-desktop prepare(true) here — that was ~2s PNG encode on every refresh.
          // First selection still uses snapshot crop (preCapturedImage) for exact freeze-frame match.
          const sRegion: ScreenshotRegion = {
            left: Math.round(region.x),
            top: Math.round(region.y),
            width: Math.round(region.width),
            height: Math.round(region.height),
          };

          // I1: exclude frame from capture without hide flash when possible.
          // Rust returns true = affinity OK (short settle); false = hide/show (need DWM).
          const [affinityOk] = await safeInvoke<boolean>(
            'set_ocr_region_frame_sampling',
            { sampling: true },
            { silent: true },
          );
          hidRegionFrame = true;
          // Only true means WDA affinity; null/false → hide path needs longer DWM settle.
          const usedAffinity = affinityOk === true;
          const settleMs = usedAffinity
            ? continuousRef.current
              ? 12
              : 24
            : continuousRef.current
              ? 40
              : 50;
          await new Promise((resolve) => window.setTimeout(resolve, settleMs));
          if (frameClosedRef.current || sessionId !== sessionIdRef.current) return;

          try {
            image = await captureScreenshotRegion(sRegion);
          } catch (gdiErr) {
            // Prefer a second GDI attempt over full-desktop prepare (overwrites selection cache).
            console.warn('[OCR] region GDI failed, retry once:', gdiErr);
            await new Promise((r) => window.setTimeout(r, 30));
            if (frameClosedRef.current || sessionId !== sessionIdRef.current) return;
            try {
              image = await captureScreenshotRegion(sRegion);
            } catch (gdiErr2) {
              console.warn('[OCR] region GDI retry failed, snapshot fallback:', gdiErr2);
              // Use cached snapshot if fresh; force only if crop fails
              try {
                const info = snapshotInfoRef.current || (await prepareScreenshotSnapshot(false));
                snapshotInfoRef.current = info;
                const sx = info.imageWidth > 0 ? info.imageWidth / info.screenWidth : 1;
                const sy = info.imageHeight > 0 ? info.imageHeight / info.screenHeight : 1;
                image = await cropScreenshotSnapshot({
                  left: Math.max(0, Math.round((sRegion.left - info.screenX) * sx)),
                  top: Math.max(0, Math.round((sRegion.top - info.screenY) * sy)),
                  width: Math.max(1, Math.round(sRegion.width * sx)),
                  height: Math.max(1, Math.round(sRegion.height * sy)),
                });
              } catch {
                const info = await prepareScreenshotSnapshot(true);
                snapshotInfoRef.current = info;
                const sx = info.imageWidth > 0 ? info.imageWidth / info.screenWidth : 1;
                const sy = info.imageHeight > 0 ? info.imageHeight / info.screenHeight : 1;
                image = await cropScreenshotSnapshot({
                  left: Math.max(0, Math.round((sRegion.left - info.screenX) * sx)),
                  top: Math.max(0, Math.round((sRegion.top - info.screenY) * sy)),
                  width: Math.max(1, Math.round(sRegion.width * sx)),
                  height: Math.max(1, Math.round(sRegion.height * sy)),
                });
              }
            }
          }

          // Grab done — re-include frame in capture while OCR/API run (no black window).
          if (hidRegionFrame) {
            await safeInvoke(
              'set_ocr_region_frame_sampling',
              { sampling: false },
              { silent: true },
            );
            hidRegionFrame = false;
          }
        }

        // Region watch: pixel fingerprint (Rust 24×24 luma) then JS fallback.
        // First capture always runs (empty fingerprint / empty last text).
        let fp = await imageDataUrlFingerprint(image);
        // Rust fail → do not trust weak JS hash for skip (false match would freeze watch).
        if (!fp) {
          fp = '';
          lastImageFpRef.current = '';
        }
        if (
          lastImageFpRef.current &&
          fp &&
          fp === lastImageFpRef.current &&
          lastOcrTextRef.current
        ) {
          consecutiveSkipRef.current = Math.min(consecutiveSkipRef.current + 1, 8);
          console.info('[OCR] continuous skip (fingerprint unchanged)', {
            skips: consecutiveSkipRef.current,
            fp: fp.slice(0, 12),
          });
          return;
        }
        consecutiveSkipRef.current = 0;

        // Soft loading for manual refresh only — continuous ticks are frequent; spinner would flash.
        if (!preCapturedImage && !continuousRef.current) {
          void emitTo('ocr-region-frame', 'ocr-region-loading', { loading: true }).catch(
            () => undefined,
          );
        }

        // I5 natural size + OCR in parallel (faster first paint).
        const ocrEngine = ocrEngineRef.current || 'winrt';
        let imageNatural = { width: 0, height: 0 };
        let ocrResult: OcrResultDetailed;
        const tr = tRef.current;
        try {
          [imageNatural, ocrResult] = await Promise.all([
            probeDataUrlImageSize(image),
            ocrWithEngine(image, ocrEngine, 'auto'),
          ]);
        } catch (ocrErr) {
          hasOcrRef.current = true;
          // Do not lock fingerprint — continuous watch should retry when content appears.
          lastImageFpRef.current = '';
          lastOcrTextRef.current = '';
          const msg =
            ocrErr instanceof Error ? ocrErr.message : tr('ocr.noTextRecognized') || 'OCR 失败';
          await sendToRegionFrame(image, { text: '', lines: [] }, '', [], undefined, imageNatural);
          await emitTo('ocr-region-frame', 'ocr-region-error', { message: msg }).catch(
            () => undefined,
          );
          return;
        }
        if (frameClosedRef.current || sessionId !== sessionIdRef.current) return;

        // Empty OCR must NOT kill the region frame — show error in-frame and keep chrome usable.
        if (!ocrResult.text.trim()) {
          hasOcrRef.current = true;
          // Clear fp so continuous can re-OCR when text appears in the pin region.
          lastImageFpRef.current = '';
          lastOcrTextRef.current = '';
          await sendToRegionFrame(image, { text: '', lines: [] }, '', [], undefined, imageNatural);
          await emitTo('ocr-region-frame', 'ocr-region-error', {
            message: tr('ocr.noTextRecognized') || 'OCR 没有识别到文本',
          }).catch(() => undefined);
          return;
        }

        const sourceTextTrimmed = ocrResult.text.trim();
        hasOcrRef.current = true;

        // Auto-detect language from full OCR text
        let effectiveSourceLang = normalizeLang(sourceLangRef.current);
        let detectedLangName: string | undefined;
        if (effectiveSourceLang === 'auto' && sourceTextTrimmed.length >= 2) {
          try {
            const detected = await invokeOrThrow<DetectionResult>('detect_language', {
              text: sourceTextTrimmed,
            });
            if (detected.language !== 'auto') {
              effectiveSourceLang = normalizeLang(detected.language);
              detectedLangName = detected.name;
            }
          } catch {
            // Language detection failure is non-fatal, continue with "auto"
          }
        }
        const effectiveTargetLang = normalizeLang(targetLangRef.current);

        // Skip tiny OCR jitter (same content with 1-2 char noise / whitespace)
        const normalized = normalizeOcrText(sourceTextTrimmed);
        const prevNormalized = normalizeOcrText(lastOcrTextRef.current);
        const similar =
          !!prevNormalized &&
          (normalized === prevNormalized ||
            textSimilarity(normalized, prevNormalized) >= OCR_TEXT_SIMILARITY_SKIP);
        let translatedText = '';
        let lineTranslations: string[] = [];

        if (!similar || !lastOcrTextRef.current) {
          lastOcrTextRef.current = sourceTextTrimmed;
          lastImageFpRef.current = fp;

          // One translate call for the full OCR text (avoids N× engine/API blowup).
          // lineTranslations must be the SAME length as ocrResult.lines for frame indexing.
          const allLines = ocrResult.lines;
          const nonEmptyIdx: number[] = [];
          for (let i = 0; i < allLines.length; i++) {
            if (allLines[i].text.trim().length > 0) nonEmptyIdx.push(i);
          }

          lineTranslations = allLines.map((l) => l.text);
          translatedText = '';
          let translateFailed = false;

          // Progressive paint: show OCR text immediately, then replace with translation.
          // Cuts perceived latency when network engines are slow.
          if (nonEmptyIdx.length > 0 && preCapturedImage) {
            void sendToRegionFrame(
              image,
              ocrResult,
              sourceTextTrimmed,
              lineTranslations,
              detectedLangName,
              imageNatural,
              false,
            );
          }

          if (nonEmptyIdx.length === 0) {
            await emitTo('ocr-region-frame', 'ocr-region-error', {
              message: tr('ocr.noTextRecognized') || 'OCR 没有识别到文本',
            }).catch(() => undefined);
          } else if (
            effectiveSourceLang !== 'auto' &&
            effectiveSourceLang === effectiveTargetLang
          ) {
            // Same lang: show source as "translation", soft hint (not red error blocking UI).
            translatedText = sourceTextTrimmed;
            lineTranslations = allLines.map((l) => l.text);
            void emitTo('ocr-region-frame', 'ocr-region-hint', {
              message:
                tr('ocr.sameLang') ||
                `源语言与目标语言相同（${effectiveTargetLang}），请切换目标语`,
            }).catch(() => undefined);
          } else {
            try {
              const sourcePieces = nonEmptyIdx.map((i) => allLines[i].text);
              // Batch path (translate_embedded → run_batch): one IPC, concurrent segments.
              // Avoid N× full translate for ≤5 lines and fragile whole-blob align for many lines.
              if (sourcePieces.length === 1) {
                const response = await invokeOrThrow<TranslateResponse>('translate', {
                  request: {
                    text: sourcePieces[0].trim(),
                    from: effectiveSourceLang,
                    to: effectiveTargetLang,
                    channel: 'ocr',
                  },
                });
                const out = response.results[0]?.text?.trim() || sourcePieces[0];
                if (frameClosedRef.current || sessionId !== sessionIdRef.current) return;
                lineTranslations[nonEmptyIdx[0]] = out;
                translatedText = out;
              } else {
                interface EmbeddedLine {
                  lineNumber: number;
                  original: string;
                  translated: string;
                }
                const batch = await invokeOrThrow<EmbeddedLine[]>('translate_embedded', {
                  text: sourcePieces.map((s) => s.trim()).join('\n'),
                  from: effectiveSourceLang,
                  to: effectiveTargetLang,
                  channel: 'ocr',
                });
                if (frameClosedRef.current || sessionId !== sessionIdRef.current) return;
                // Index-aligned only (lineNumber from run_batch). Do NOT Map by original —
                // duplicate OCR lines would collapse and mis-assign translations.
                const byOrder = [...batch].sort((a, b) => a.lineNumber - b.lineNumber);
                for (let p = 0; p < nonEmptyIdx.length; p++) {
                  const tLine = byOrder[p]?.translated?.trim() || '';
                  lineTranslations[nonEmptyIdx[p]] = tLine || allLines[nonEmptyIdx[p]].text;
                }
                translatedText = nonEmptyIdx.map((i) => lineTranslations[i]).join('\n');
                if (!translatedText.trim()) {
                  throw new Error('empty translation result');
                }
              }
            } catch (err) {
              console.warn('[OCR] Translation failed:', err);
              translateFailed = true;
              translatedText = '';
              await emitTo('ocr-region-frame', 'ocr-region-error', {
                message:
                  tr('ocr.translateFailed') || '翻译失败：引擎无结果（请检查密钥/网络/引擎开关）',
              }).catch(() => undefined);
            }
          }
          if (translatedText) {
            lastTranslatedRef.current = translatedText;
            lastLineTranslationsRef.current = lineTranslations;
          }
          await sendToRegionFrame(
            image,
            ocrResult,
            translatedText || sourceTextTrimmed,
            lineTranslations,
            detectedLangName,
            imageNatural,
            translateFailed,
          );
        } else {
          // I7: text similar but image/layout changed — update boxes, keep last translation (no API).
          lastImageFpRef.current = fp;
          const kept =
            lastLineTranslationsRef.current.length === ocrResult.lines.length
              ? lastLineTranslationsRef.current
              : ocrResult.lines.map((l) => l.text);
          const keptText = lastTranslatedRef.current || sourceTextTrimmed;
          await sendToRegionFrame(
            image,
            ocrResult,
            keptText,
            kept,
            detectedLangName,
            imageNatural,
            false,
          );
        }
      } finally {
        // Only restore sampling / show if THIS session still owns the frame.
        // Otherwise sampling:false always show()s and can pop the frame over a new selector.
        const sessionAlive = !frameClosedRef.current && sessionId === sessionIdRef.current;
        if (hidRegionFrame && sessionAlive) {
          await safeInvoke('set_ocr_region_frame_sampling', { sampling: false }, { silent: true });
        } else if (hidRegionFrame) {
          // Session cancelled mid-grab: clear affinity without forcing show (Rust still shows —
          // best-effort: re-hide if we intended the frame gone).
          await safeInvoke('set_ocr_region_frame_sampling', { sampling: false }, { silent: true });
          if (frameClosedRef.current) {
            await safeInvoke('set_ocr_region_frame_visible', { visible: false }, { silent: true });
          }
        }
        if (sessionAlive) {
          void emitTo('ocr-region-frame', 'ocr-region-loading', { loading: false }).catch(
            () => undefined,
          );
        }
        busyRef.current = false;
        const pendingRegion = pendingRegionRef.current;
        if (pendingRegion && sessionAlive) {
          pendingRegionRef.current = null;
          queueMicrotask(() => {
            if (frameClosedRef.current || sessionId !== sessionIdRef.current) return;
            void captureAndTranslate(pendingRegion);
          });
        } else if (!sessionAlive) {
          pendingRegionRef.current = null;
        }
      }
    },
    [sendToRegionFrame],
  );

  // ---- Listen for events from the region frame window ----
  // Uses `cancelled` flag to handle React StrictMode double-mount:
  // StrictMode runs setup() twice; the first instance's cleanup runs before
  // await listen() resolves, so `unlisteners` is empty. The `cancelled` flag
  // ensures the first listener self-destructs when its callback fires.
  useEffect(() => {
    if (!isTauri) return;

    let cancelled = false;
    const unlisteners: Array<() => void> = [];

    const registerListener = async <T,>(
      eventName: string,
      handler: (event: { payload: T }) => void,
    ) => {
      const unlisten = await listen<T>(eventName, (event) => {
        if (cancelled) return;
        handler(event);
      });
      if (cancelled) {
        unlisten();
      } else {
        unlisteners.push(unlisten);
      }
    };

    // Position changed (drag / mount sync) — update X/Y only.
    // NEVER re-OCR here. NEVER adopt width/height from the frame window: min-width
    // expansion (I3 min frame width) makes getCaptureRegion() wider than the real OCR crop and
    // used to corrupt regionRef → refresh/follow offset drift.
    void registerListener<RegionRect>('ocr-region-position-changed', (event) => {
      const r = event.payload;
      const prev = regionRef.current;
      // Drag: X/Y from frame; keep true crop size (ignore min-width expanded getCaptureRegion).
      regionRef.current = {
        x: r.x,
        y: r.y,
        width: prev && prev.width > 0 ? prev.width : r.width,
        height: prev && prev.height > 0 ? prev.height : r.height,
      };
      if (followEnabledRef.current && windowBindingRef.current) {
        windowBindingRef.current.setRegionRef(regionRef.current);
        void windowBindingRef.current.refreshOffset(regionRef.current);
      }
    });

    // Size changed (user resize) — adopt size after first OCR; debounce re-OCR while dragging corner.
    let resizeOcrTimer: ReturnType<typeof setTimeout> | null = null;
    void registerListener<RegionRect>('ocr-region-size-changed', (event) => {
      const r = event.payload;
      const prev = regionRef.current;
      if (!hasOcrRef.current) {
        if (prev) {
          regionRef.current = { ...prev, x: r.x, y: r.y };
        }
        return;
      }
      // Outer capture rect is floored by min frame width (I3). Don't treat that floor
      // as a real widen of a narrow selection crop (would OCR empty right padding).
      let nextW = r.width;
      let nextH = r.height;
      if (prev && prev.width > 0 && prev.height > 0) {
        const sx = r.width / prev.width;
        const sy = r.height / prev.height;
        // Ignore min-frame floor expand (I3), not real user resize.
        const floorish = prev.width < OCR_MIN_FRAME_WIDTH_CSS * 0.75;
        const minWidthJump = floorish && sx > 1.25 && Math.abs(sy - 1) < 0.2;
        if (minWidthJump) {
          nextW = prev.width;
          nextH = Math.round(prev.height * sy);
        } else {
          nextW = r.width;
          nextH = r.height;
        }
      }
      regionRef.current = { x: r.x, y: r.y, width: nextW, height: nextH };
      if (followEnabledRef.current && windowBindingRef.current) {
        windowBindingRef.current.setRegionRef(regionRef.current);
        void windowBindingRef.current.refreshOffset(regionRef.current);
      }
      const effectiveChanged =
        !prev || Math.abs(prev.width - nextW) > 2 || Math.abs(prev.height - nextH) > 2;
      if (!effectiveChanged) return;
      if (resizeOcrTimer) window.clearTimeout(resizeOcrTimer);
      resizeOcrTimer = window.setTimeout(() => {
        resizeOcrTimer = null;
        if (frameClosedRef.current) return;
        const cur = regionRef.current;
        if (!cur) return;
        lastOcrTextRef.current = '';
        lastImageFpRef.current = '';
        if (busyRef.current) {
          // Coalesce like other busy paths — don't drop the final size.
          pendingRegionRef.current = { x: cur.x, y: cur.y, width: cur.width, height: cur.height };
          return;
        }
        void captureAndTranslate({ x: cur.x, y: cur.y, width: cur.width, height: cur.height });
      }, 180);
    });

    // Manual refresh
    void registerListener<unknown>('ocr-region-refresh', () => {
      if (regionRef.current) {
        lastOcrTextRef.current = '';
        lastImageFpRef.current = '';
        void captureAndTranslate(regionRef.current);
      }
    });

    // Continuous toggle — only user gesture should enable; never auto-on
    void registerListener<{ enabled: boolean }>('ocr-region-continuous', (event) => {
      const enabled = !!event.payload.enabled;
      continuousRef.current = enabled;
      if (enabled) {
        consecutiveSkipRef.current = 0;
        // Force next tick to re-fingerprint; effect schedules first sample (avoid double OCR here).
        lastImageFpRef.current = '';
      }
      setContinuous(enabled);
    });

    // Follow target window toggle (pin region to a window and track moves)
    void registerListener<{ enabled: boolean }>('ocr-region-follow', (event) => {
      const enabled = event.payload.enabled;
      followEnabledRef.current = enabled;
      const binding = windowBindingRef.current;
      if (!binding) return;

      if (!enabled) {
        binding.unbind();
        return;
      }

      const region = regionRef.current;
      if (!region) return;
      binding.setRegionRef(region);
      // Click-through (no hide flash) so hwnd_from_point hits content under the frame (I6).
      void (async () => {
        const followSession = sessionIdRef.current;
        let restoredClick = false;
        try {
          await safeInvoke(
            'set_ocr_region_frame_click_through',
            { ignore: true },
            { silent: true },
          );
          await new Promise((r) => window.setTimeout(r, 16));
          if (frameClosedRef.current || followSession !== sessionIdRef.current) return;
          let bound = await binding.bind(region);
          if (!bound) {
            const retryRegion = {
              ...region,
              y: region.y + Math.min(40, Math.max(8, region.height * 0.15)),
            };
            bound = await binding.bind(retryRegion);
            if (bound) {
              binding.setRegionRef(region);
              await binding.refreshOffset(region);
            }
          }
          if (frameClosedRef.current || followSession !== sessionIdRef.current) {
            binding.unbind();
            return;
          }
          await safeInvoke(
            'set_ocr_region_frame_click_through',
            { ignore: false },
            { silent: true },
          );
          restoredClick = true;
          if (!bound) {
            console.warn('[OCR] Failed to bind target window for follow mode');
            followEnabledRef.current = false;
            void emitTo('ocr-region-frame', 'ocr-region-follow-state', {
              enabled: false,
            }).catch(() => undefined);
            void emitTo('ocr-region-frame', 'ocr-region-hint', {
              message: '无法跟随目标窗口（请点在内容上再开跟随）',
            }).catch(() => undefined);
          }
        } finally {
          if (!restoredClick) {
            await safeInvoke(
              'set_ocr_region_frame_click_through',
              { ignore: false },
              { silent: true },
            );
          }
        }
      })();
    });

    // Language change from region frame
    void registerListener<{ sourceLang: string; targetLang: string }>(
      'ocr-region-lang-change',
      (event) => {
        const p = event.payload;
        langOverriddenRef.current = true;
        sourceLangRef.current = p.sourceLang;
        targetLangRef.current = p.targetLang;
        lastOcrTextRef.current = '';
        lastImageFpRef.current = '';
        if (regionRef.current) {
          void captureAndTranslate(regionRef.current);
        }
      },
    );

    // Engine switch / enable from region frame toolbar
    void registerListener<{ engineId: string; enabled?: boolean; promote?: boolean }>(
      'ocr-region-engine-change',
      (event) => {
        const { engineId, enabled, promote } = event.payload || {};
        if (!engineId) return;
        updateConfig((prev) => {
          const engines = { ...prev.engines } as AppConfig['engines'] &
            Record<string, { enabled?: boolean }>;
          const keyMap: Record<string, keyof AppConfig['engines']> = {
            google: 'google',
            youdao: 'youdao',
            baidu: 'baidu',
            deepl: 'deepl',
            deeplx: 'deeplx',
            microsoft: 'microsoft',
            yandex: 'yandex',
            offline: 'offline',
            caiyun: 'caiyun',
            tatoeba: 'tatoeba',
            baidu_web: 'baiduWeb',
            caiyun_web: 'caiyunWeb',
            volcengine_web: 'volcengineWeb',
            transmart: 'transmart',
            papago: 'papago',
          };
          // Only known engine keys (llm lives on config.llm, not engines).
          const cfgKey = keyMap[engineId];
          if (cfgKey) {
            const cur = engines[cfgKey] as { enabled?: boolean };
            engines[cfgKey] = {
              ...cur,
              enabled: enabled === undefined ? true : enabled,
            } as never;
          }
          let engineOrder = [...(prev.engineOrder || [])];
          if (promote) {
            engineOrder = [engineId, ...engineOrder.filter((id) => id !== engineId)];
          }
          return { ...prev, engines, engineOrder };
        });
        void saveConfig();
        lastOcrTextRef.current = '';
        lastImageFpRef.current = '';
        if (regionRef.current) {
          void captureAndTranslate(regionRef.current);
        }
      },
    );

    // Region frame closed → OCR session ends → main may return (lifecycle, not focus fight).
    void registerListener<unknown>('ocr-region-close', async () => {
      frameClosedRef.current = true;
      ocrSessionActiveRef.current = false;
      sessionIdRef.current += 1;
      pendingRegionRef.current = null;
      setContinuous(false);
      regionRef.current = null;
      lastOcrTextRef.current = '';
      lastTranslatedRef.current = '';
      lastLineTranslationsRef.current = [];
      lastImageFpRef.current = '';
      consecutiveSkipRef.current = 0;
      hasOcrRef.current = false;
      langOverriddenRef.current = false;
      followEnabledRef.current = false;
      windowBindingRef.current?.unbind();
      await safeInvoke('set_ocr_region_frame_click_through', { ignore: false }, { silent: true });
      await safeInvoke('set_ocr_region_frame_sampling', { sampling: false }, { silent: true });
      await safeInvoke('close_ocr_region_frame', undefined, { silent: true });
      await safeInvoke('ocr_end_session_show_main', undefined, { silent: true });
    });

    return () => {
      cancelled = true;
      if (resizeOcrTimer) window.clearTimeout(resizeOcrTimer);
      unlisteners.forEach((fn) => fn());
      windowBindingRef.current?.unbind();
    };
  }, [captureAndTranslate, isTauri, updateConfig, saveConfig]);

  // Window-binding manager for follow mode (move region frame with target window)
  useEffect(() => {
    if (!isTauri) return;

    const binding = new WindowBindingManager({
      onRegionUpdate: (newRegion) => {
        if (frameClosedRef.current || !followEnabledRef.current) return;
        regionRef.current = newRegion;
        if (movingFrameRef.current) return;
        movingFrameRef.current = true;
        void safeInvoke(
          'move_ocr_region_frame',
          {
            x: newRegion.x,
            y: newRegion.y,
            width: newRegion.width,
            height: newRegion.height,
          },
          { silent: true },
        ).finally(() => {
          movingFrameRef.current = false;
        });
      },
      onWindowMinimized: () => {
        // Keep last translation; pause continuous capture while minimized
        if (continuousRef.current) {
          continuousRef.current = false;
          setContinuous(false);
          void emitTo('ocr-region-frame', 'ocr-region-continuous-state', {
            enabled: false,
          }).catch(() => undefined);
        }
      },
      onWindowRestored: () => {
        // User re-enables watch from toolbar if desired
      },
      onOverlayPositionSync: () => {
        // Screenshot OCR uses in-region overlays, not the side overlay window
      },
    });
    windowBindingRef.current = binding;

    return () => {
      binding.dispose();
      if (windowBindingRef.current === binding) {
        windowBindingRef.current = null;
      }
    };
  }, [isTauri]);

  // ---- Continuous / pinned region watch (SKELETON — product reserved) ----
  // Spec: docs/OCR_STRATEGY.md. Tick: sampling exclude→GDI→fingerprint/OCR.
  // Adaptive delay: more consecutive skips → longer wait (less CPU when idle).
  useEffect(() => {
    if (!continuous) return;
    if (!regionRef.current) {
      // Region not ready yet — wait briefly and allow effect re-run via continuous still true
      return;
    }

    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const schedule = () => {
      if (cancelled) return;
      const skips = consecutiveSkipRef.current;
      const delay = Math.min(ocrIntervalMs * 3, Math.round(ocrIntervalMs * (1 + skips * 0.25)));
      timer = window.setTimeout(async () => {
        if (cancelled) return;
        const r = regionRef.current;
        if (
          r &&
          continuousRef.current &&
          !busyRef.current &&
          !frameClosedRef.current &&
          !movingFrameRef.current
        ) {
          await captureAndTranslate(r);
        }
        schedule();
      }, delay);
    };

    // First sample quickly so enabling watch feels live (not a full interval wait).
    const firstDelay = Math.min(600, Math.max(200, Math.floor(ocrIntervalMs * 0.35)));
    timer = window.setTimeout(() => {
      void (async () => {
        if (cancelled) return;
        const r = regionRef.current;
        if (
          r &&
          continuousRef.current &&
          !busyRef.current &&
          !frameClosedRef.current &&
          !movingFrameRef.current
        ) {
          await captureAndTranslate(r);
        }
        schedule();
      })();
    }, firstDelay);

    return () => {
      cancelled = true;
      if (timer) window.clearTimeout(timer);
    };
  }, [continuous, captureAndTranslate, ocrIntervalMs]);

  // ---- Screenshot selection listener ----
  useEffect(() => {
    if (!isTauri) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<ScreenshotRegion>('ocr-screenshot-selected', async (event) => {
      if (cancelled) return; // guard against StrictMode double-mount
      const sel = event.payload;
      const info = snapshotInfoRef.current;
      // Selector emits IMAGE-pixel coords. Crop the same snapshot the user saw
      // (avoids GDI re-capture DPI drift that shifts content right/down).
      // Pad a few pixels so edge glyphs are not clipped for WinRT OCR.
      const PAD = OCR_SELECTION_PAD_PX;
      const imgW = info?.imageWidth ?? Number.MAX_SAFE_INTEGER;
      const imgH = info?.imageHeight ?? Number.MAX_SAFE_INTEGER;
      const cropLeft = Math.max(0, Math.round(sel.left) - PAD);
      const cropTop = Math.max(0, Math.round(sel.top) - PAD);
      const cropRight = Math.min(imgW, Math.round(sel.left + sel.width) + PAD);
      const cropBottom = Math.min(imgH, Math.round(sel.top + sel.height) + PAD);
      const cropW = Math.max(1, cropRight - cropLeft);
      const cropH = Math.max(1, cropBottom - cropTop);

      // Snapshot pixels are 1:1 with virtual-screen physical (GDI). Do not use
      // screenWidth/imageWidth as a "scale" when both are already physical px —
      // any mismatch (stale meta) would shift the region frame.
      const offsetX = info ? info.screenX : 0;
      const offsetY = info ? info.screenY : 0;

      // Frame matches padded crop so background + OCR line coords share one origin
      const screenX = Math.round(offsetX + cropLeft);
      const screenY = Math.round(offsetY + cropTop);
      const screenW = Math.round(cropW);
      const screenH = Math.round(cropH);

      console.info('[OCR] selection→frame', {
        sel,
        crop: { cropLeft, cropTop, cropW, cropH },
        screen: { screenX, screenY, screenW, screenH },
        info,
      });

      const region: RegionRect = { x: screenX, y: screenY, width: screenW, height: screenH };
      regionRef.current = region;

      // Crop + create frame first; close selector after frame exists (less naked-desktop gap).
      const cropPromise = cropScreenshotSnapshot({
        left: cropLeft,
        top: cropTop,
        width: cropW,
        height: cropH,
      }).catch(async (err: unknown) => {
        console.warn('[OCR] Snapshot crop failed, falling back to live capture:', err);
        return captureScreenshotRegion({
          left: screenX,
          top: screenY,
          width: screenW,
          height: screenH,
        });
      });

      let image: string;
      try {
        // Lifecycle handoff (pot): result show+focus FIRST, then destroy screenshot.
        // Main stays out of session (STranslate) — no focus fight with DWM.
        const framePromise = invokeOrThrow('create_ocr_region_frame', {
          x: screenX,
          y: screenY,
          width: screenW,
          height: screenH,
        });
        const [img] = await Promise.all([cropPromise, framePromise]);
        image = img;
        frameClosedRef.current = false;
        ocrSessionActiveRef.current = true;
        await safeInvoke('set_ocr_region_frame_click_through', { ignore: false }, { silent: true });
        // Result takes foreground before selector dies (no empty-foreground gap).
        await safeInvoke('set_ocr_region_frame_visible', { visible: true }, { silent: true });
        await safeInvoke('close_ocr_screenshot_selector', undefined, { silent: true });
      } catch (err) {
        if (cancelled) return;
        ocrSessionActiveRef.current = false;
        await safeInvoke('close_ocr_screenshot_selector', undefined, { silent: true });
        await safeInvoke('close_ocr_region_frame', undefined, { silent: true });
        await safeInvoke('ocr_end_session_show_main', undefined, { silent: true });
        console.error('[OCR] Failed to create region frame or crop:', err);
        return;
      }

      continuousRef.current = false;
      setContinuous(false);
      if (selectorSafetyTimerRef.current) {
        window.clearTimeout(selectorSafetyTimerRef.current);
        selectorSafetyTimerRef.current = null;
      }

      // Instant preview while OCR/translate run.
      void emitTo('ocr-region-frame', 'ocr-region-update-data', {
        screenshot: image,
        sourceText: '',
        translatedText: '',
        ocrLines: [],
        lineTranslations: [],
        sourceLang: sourceLangRef.current,
        targetLang: targetLangRef.current,
        refreshIntervalMs: ocrIntervalMsRef.current,
      }).catch(() => undefined);

      const frameReady = await waitForOcrRegionFrameReady(OCR_FRAME_READY_TIMEOUT_MS);
      if (!frameReady) {
        console.warn('[OCR] region frame ready timeout — retrying create once');
        try {
          await invokeOrThrow('create_ocr_region_frame', {
            x: screenX,
            y: screenY,
            width: screenW,
            height: screenH,
          });
          await safeInvoke('set_ocr_region_frame_visible', { visible: true }, { silent: true });
          const ready2 = await waitForOcrRegionFrameReady(OCR_FRAME_READY_TIMEOUT_MS);
          if (!ready2) {
            throw new Error('OCR 区域窗口未就绪，请重试');
          }
        } catch (readyErr) {
          if (cancelled) return;
          console.error('[OCR] frame not ready:', readyErr);
          ocrSessionActiveRef.current = false;
          await safeInvoke('close_ocr_region_frame', undefined, { silent: true });
          await safeInvoke('ocr_end_session_show_main', undefined, { silent: true });
          return;
        }
      }
      void emitTo('ocr-region-frame', 'ocr-region-continuous-state', { enabled: false }).catch(
        () => undefined,
      );
      const resetOk = await waitForSessionResetAck(200);
      if (!resetOk) {
        console.warn('[OCR] session-reset ack timeout — continuing');
      }
      void emitTo('ocr-region-frame', 'ocr-region-update-data', {
        screenshot: image,
        sourceText: '',
        translatedText: '',
        ocrLines: [],
        lineTranslations: [],
        sourceLang: sourceLangRef.current,
        targetLang: targetLangRef.current,
        refreshIntervalMs: ocrIntervalMsRef.current,
      }).catch(() => undefined);

      lastOcrTextRef.current = '';
      lastImageFpRef.current = '';
      try {
        await captureAndTranslate(region, image);
      } catch (err) {
        if (cancelled) return;
        console.error('[OCR] captureAndTranslate failed after selection:', err);
        void emitTo('ocr-region-frame', 'ocr-region-error', {
          message: err instanceof Error ? err.message : String(err),
        }).catch(() => undefined);
      }
    }).then((fn) => {
      if (cancelled) {
        fn(); // StrictMode cleanup came before registration — remove immediately
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [captureAndTranslate, isTauri, updateConfig, saveConfig]);

  // Guard ref to prevent concurrent startScreenshotTranslate calls
  const startingRef = useRef(false);
  /** Stuck-selector watchdog — cleared on selection done / cancel / next start. */
  const selectorSafetyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearSelectorSafetyTimer = useCallback(() => {
    if (selectorSafetyTimerRef.current) {
      window.clearTimeout(selectorSafetyTimerRef.current);
      selectorSafetyTimerRef.current = null;
    }
  }, []);

  // ---- Cancelled listener (Esc / selector fail) → end session, restore main ----
  useEffect(() => {
    if (!isTauri) return;

    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen('ocr-screenshot-cancelled', () => {
      if (cancelled) return;
      clearSelectorSafetyTimer();
      ocrSessionActiveRef.current = false;
      void (async () => {
        await safeInvoke('close_ocr_screenshot_selector', undefined, { silent: true });
        await safeInvoke('ocr_end_session_show_main', undefined, { silent: true });
      })();
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [isTauri, clearSelectorSafetyTimer]);

  // ---- Start screenshot translate ----
  // Lifecycle (STranslate + pot):
  //   session start → main collapsed (out of OCR)
  //   selector visible → user selects
  //   result show+focus → destroy selector (result takes baton)
  //   session end (close/cancel) → main show
  const startScreenshotTranslate = useCallback(async () => {
    if (!isTauri) return;
    if (startingRef.current) return;
    startingRef.current = true;

    continuousRef.current = false;
    frameClosedRef.current = true;
    ocrSessionActiveRef.current = true;
    sessionIdRef.current += 1;
    pendingRegionRef.current = null;
    setContinuous(false);
    regionRef.current = null;
    lastOcrTextRef.current = '';
    lastTranslatedRef.current = '';
    lastLineTranslationsRef.current = [];
    lastImageFpRef.current = '';
    consecutiveSkipRef.current = 0;
    hasOcrRef.current = false;
    langOverriddenRef.current = false;
    followEnabledRef.current = false;
    windowBindingRef.current?.unbind();
    clearSelectorSafetyTimer();

    try {
      await safeInvoke('set_ocr_region_frame_sampling', { sampling: false }, { silent: true });
      await safeInvoke('set_ocr_region_frame_visible', { visible: false }, { silent: true });
      void emitTo('ocr-region-frame', 'ocr-region-continuous-state', { enabled: false }).catch(
        () => undefined,
      );
      void emitTo('ocr-region-frame', 'ocr-region-session-reset', null).catch(() => undefined);
      await safeInvoke('close_ocr_screenshot_selector', undefined, { silent: true });
      frameClosedRef.current = true;

      // STranslate: collapse main for whole OCR session (not just capture).
      await safeInvoke('ocr_begin_session_hide_main', undefined, { silent: true });
      await new Promise((resolve) => setTimeout(resolve, 32));

      let info: ScreenshotSnapshotInfo;
      try {
        info = await withTimeout(
          prepareScreenshotSnapshot(true),
          10000,
          t('ocr.captureTimeout') || '屏幕捕获超时：当前桌面会话可能不允许截图',
        );
      } catch (capErr) {
        ocrSessionActiveRef.current = false;
        await safeInvoke('ocr_end_session_show_main', undefined, { silent: true });
        throw capErr;
      }
      snapshotInfoRef.current = info;

      try {
        const readyPromise = waitForOcrScreenshotReady(8000);
        await invokeOrThrow('create_ocr_screenshot_selector');
        const ready = await readyPromise;
        if (!ready) {
          console.warn('[OCR] selector freeze ready timeout — force-show selector');
          try {
            const sel = await WebviewWindow.getByLabel('ocr-screenshot');
            await sel?.show();
            await sel?.setFocus();
          } catch {
            /* ignore */
          }
        }
      } catch (selErr) {
        ocrSessionActiveRef.current = false;
        await safeInvoke('close_ocr_screenshot_selector', undefined, { silent: true });
        await safeInvoke('ocr_end_session_show_main', undefined, { silent: true });
        throw selErr;
      }

      // Main stays collapsed for the rest of the session (until result close / cancel).
      // Stuck-selector safety: end session after 20s if still no selection.
      selectorSafetyTimerRef.current = window.setTimeout(() => {
        selectorSafetyTimerRef.current = null;
        void (async () => {
          try {
            if (!ocrSessionActiveRef.current) return;
            if (!frameClosedRef.current) return; // result already up
            console.warn('[OCR] Selector stuck 20s — end session, restore main');
            ocrSessionActiveRef.current = false;
            await safeInvoke('close_ocr_screenshot_selector', undefined, { silent: true });
            await safeInvoke('set_ocr_region_frame_visible', { visible: false }, { silent: true });
            await safeInvoke('ocr_end_session_show_main', undefined, { silent: true });
          } catch {
            /* ignore */
          }
        })();
      }, 20_000);
    } catch (err) {
      console.error('[OCR] Error in startScreenshotTranslate:', err);
      clearSelectorSafetyTimer();
      ocrSessionActiveRef.current = false;
      try {
        await safeInvoke('ocr_end_session_show_main', undefined, { silent: true });
      } catch {
        // ignore
      }
    } finally {
      startingRef.current = false;
    }
  }, [isTauri, t, clearSelectorSafetyTimer]);

  useEffect(() => {
    if (launchNonce <= 0) return;
    void startScreenshotTranslate();
  }, [launchNonce, startScreenshotTranslate]);

  return null;
}
