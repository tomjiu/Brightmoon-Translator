import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
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
import { alignTranslationToLines } from '../services/ocrLineAlign';
import { useConfigStore } from '../stores/configStore';
import type { AppConfig, TranslateResponse, DetectionResult, DictionaryResult } from '../types';
import { WindowBindingManager } from '../hooks/ocrWindowBinding';
import { normalizeOcrText, textSimilarity, OCR_TEXT_SIMILARITY_SKIP } from '../hooks/ocrQuality';
import {
  OCR_MIN_FRAME_WIDTH_CSS,
  OCR_SELECTION_PAD_PX,
  probeDataUrlImageSize,
} from './ocrRegionGeometry';
import { OCR_WATCH_INTERVAL_DEFAULT_MS, OCR_WATCH_INTERVAL_MIN_MS } from '../services/ocrConstants';
import {
  OcrRegionEvents,
  OcrMainEvents,
  DEFAULT_REGION_ID,
  REGION_EVENTS_BY_ID,
  regionEventName,
  emitToRegionId,
  OCR_FRAME_READY_TIMEOUT_MS,
  OCR_SESSION_RESET_ACK_TIMEOUT_MS,
  OCR_SCREENSHOT_READY_TIMEOUT_MS,
  type OcrRegionRect,
  type OcrRegionUpdateData,
  type OcrRegionLangChangePayload,
  type OcrRegionEngineChangePayload,
  type OcrRegionPositionPayload,
} from '../services/ocrRegionProtocol';
import {
  waitForOcrRegionFrameReady,
  waitForSessionResetAck,
  waitForOcrScreenshotReady,
  withTimeout,
  isOcrSingleWord,
  formatOcrDictBody,
} from '../services/ocrScreenshotHelpers';

interface OcrScreenshotTranslatorProps {
  launchNonce?: number;
}

type RegionRect = OcrRegionRect;

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
  /** M3: bumped whenever any region's continuous flag changes, so the per-region
   * continuous loop re-evaluates its liveness gate. */
  const [regionsVersion, setRegionsVersion] = useState(0);
  const bumpRegionsVersion = useCallback(() => setRegionsVersion((v) => v + 1), []);

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
  // M3: per-region state. Each live region frame owns an independent
  // `RegionState` so a second selection never clobbers the first region's
  // continuous / follow / fingerprint / pending region (fixes design §1.3 B4).
  // The legacy single-frame path uses regionId === DEFAULT_REGION_ID.
  interface RegionState {
    region: RegionRect | null;
    busy: boolean;
    pendingRegion: RegionRect | null;
    continuous: boolean;
    continuousWasOnBeforeMinimize: boolean;
    frameClosed: boolean;
    sessionId: number;
    lastOcrText: string;
    lastTranslated: string;
    lastLineTranslations: string[];
    lastImageFp: string;
    consecutiveSkip: number;
    hasOcr: boolean;
    sourceLang: string;
    targetLang: string;
    langOverridden: boolean;
    followEnabled: boolean;
    movingFrame: boolean;
    windowBinding: WindowBindingManager | null;
    /** M4: per-region OCR engine override ('' = use global config.ocrEngine). */
    engine: string;
  }

  const regionsRef = useRef<Map<string, RegionState>>(new Map());
  const createRegionState = useCallback(
    (_regionId: string): RegionState => ({
      region: null,
      busy: false,
      pendingRegion: null,
      continuous: false,
      continuousWasOnBeforeMinimize: false,
      frameClosed: true,
      sessionId: 0,
      lastOcrText: '',
      lastTranslated: '',
      lastLineTranslations: [],
      lastImageFp: '',
      consecutiveSkip: 0,
      hasOcr: false,
      sourceLang: config.defaultFrom || 'auto',
      targetLang: config.defaultTo || 'zh',
      langOverridden: false,
      followEnabled: false,
      movingFrame: false,
      windowBinding: null,
      engine: '',
    }),
    [config.defaultFrom, config.defaultTo],
  );
  const getRegionState = useCallback(
    (regionId: string): RegionState => {
      let st = regionsRef.current.get(regionId);
      if (!st) {
        st = createRegionState(regionId);
        regionsRef.current.set(regionId, st);
      }
      return st;
    },
    [createRegionState],
  );

  const snapshotInfoRef = useRef<ScreenshotSnapshotInfo | null>(null);
  /** M3: region that currently owns the global follow binding (null = none). */
  const followRegionIdRef = useRef<string | null>(null);
  /** Global WindowBindingManager singleton (tracks one target window at a time). */
  const windowBindingRef = useRef<WindowBindingManager | null>(null);
  /**
   * C6: selection three-state machine (global — one selector at a time).
   * - `idle`: no session, or session closed.
   * - `selecting`: fullscreen selector is up, waiting for the user to draw a region.
   * - `captured`: a region was selected and handed off to the region frame.
   */
  type SelectionPhase = 'idle' | 'selecting' | 'captured';
  const selectionPhaseRef = useRef<SelectionPhase>('idle');
  /** C7: take-once flag for the `ocr-screenshot-selected` event (global). */
  const selectionTakenRef = useRef(false);
  /** OCR session owns main visibility (global — hidden from session start until result close). */
  const ocrSessionActiveRef = useRef(false);
  const normalizeLang = (code: string) => {
    const c = (code || '').toLowerCase();
    if (c === 'zh-cn' || c === 'zh-hans' || c === 'zh_cn') return 'zh';
    if (c === 'zh-tw' || c === 'zh-hant') return 'zh-TW';
    return c || 'auto';
  };

  useEffect(() => {
    // M3: the `continuous` React state mirrors the default region's toggle.
    getRegionState(DEFAULT_REGION_ID).continuous = continuous;
  }, [continuous, getRegionState]);

  // Config may load after mount — keep OCR lang refs aligned with store defaults
  // until the region frame overrides them via ocr-region-lang-change.
  // Do not overwrite after user changed langs on the frame (hasOcr / explicit override).
  useEffect(() => {
    const st = getRegionState(DEFAULT_REGION_ID);
    if (st.langOverridden) return;
    if (config.defaultFrom) st.sourceLang = config.defaultFrom;
    if (config.defaultTo) st.targetLang = config.defaultTo;
  }, [config.defaultFrom, config.defaultTo, getRegionState]);

  // ---- Send data to the merged region frame ----
  const ocrIntervalMsRef = useRef(ocrIntervalMs);
  useEffect(() => {
    ocrIntervalMsRef.current = ocrIntervalMs;
  }, [ocrIntervalMs]);

  const sendToRegionFrame = useCallback(
    async (
      regionId: string,
      screenshot: string,
      ocrResult: OcrResultDetailed,
      translatedText: string,
      lineTranslations: string[] = [],
      detectedLangName?: string,
      imageNatural?: { width: number; height: number },
      /** When true, frame keeps existing error (translate fail after partial update). */
      keepError?: boolean,
    ) => {
      const st = getRegionState(regionId);
      const payload: OcrRegionUpdateData = {
        screenshot,
        sourceText: ocrResult.text,
        translatedText,
        ocrLines: ocrResult.lines,
        lineTranslations,
        sourceLang: st.sourceLang,
        targetLang: st.targetLang,
        detectedLang: detectedLangName,
        engine: st.engine || undefined,
        refreshIntervalMs: ocrIntervalMsRef.current,
        imageWidth: imageNatural?.width,
        imageHeight: imageNatural?.height,
        keepError: !!keepError,
      };

      for (let attempt = 0; attempt < 3; attempt++) {
        try {
          await emitToRegionId(regionId, REGION_EVENTS_BY_ID.text(regionId), payload);
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
    [getRegionState],
  );

  // ---- Core OCR + Translate pipeline ----
  // If image is provided, skip capture (used when image was captured before region frame creation)
  // M3: `regionId` selects the per-region session state (default → legacy single frame).
  const captureAndTranslate = useCallback(
    async (regionId: string, region: RegionRect, preCapturedImage?: string) => {
      const st = getRegionState(regionId);
      if (st.busy) {
        st.pendingRegion = region;
        return;
      }
      st.busy = true;
      st.pendingRegion = null;
      const sessionId = st.sessionId;
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
            { sampling: true, id: regionId },
            { silent: true },
          );
          hidRegionFrame = true;
          // Only true means WDA affinity; null/false → hide path needs longer DWM settle.
          const usedAffinity = affinityOk === true;
          const settleMs = usedAffinity
            ? st.continuous
              ? 12
              : 24
            : st.continuous
              ? 40
              : 50;
          await new Promise((resolve) => window.setTimeout(resolve, settleMs));
          if (st.frameClosed || sessionId !== st.sessionId) return;

          try {
            image = await captureScreenshotRegion(sRegion);
          } catch (gdiErr) {
            // Prefer a second GDI attempt over full-desktop prepare (overwrites selection cache).
            console.warn('[OCR] region GDI failed, retry once:', gdiErr);
            await new Promise((r) => window.setTimeout(r, 30));
            if (st.frameClosed || sessionId !== st.sessionId) return;
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
              { sampling: false, id: regionId },
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
          st.lastImageFp = '';
        }
        if (st.lastImageFp && fp && fp === st.lastImageFp && st.lastOcrText) {
          st.consecutiveSkip = Math.min(st.consecutiveSkip + 1, 8);
          return;
        }
        st.consecutiveSkip = 0;

        // Soft loading for manual refresh only — continuous ticks are frequent; spinner would flash.
        if (!preCapturedImage && !st.continuous) {
          void emitToRegionId(regionId, regionEventName(OcrRegionEvents.loading, regionId), {
            loading: true,
          }).catch(() => undefined);
        }

        // I5 natural size + OCR in parallel (faster first paint).
        // OCR engine is GLOBAL (config.ocrEngine) — st.engine is a per-region
        // TRANSLATE engine and must never feed the OCR pipeline (P0 fix).
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
          st.hasOcr = true;
          // Do not lock fingerprint — continuous watch should retry when content appears.
          st.lastImageFp = '';
          st.lastOcrText = '';
          const msg =
            ocrErr instanceof Error ? ocrErr.message : tr('ocr.noTextRecognized') || 'OCR 失败';
          await sendToRegionFrame(
            regionId,
            image,
            { text: '', lines: [] },
            '',
            [],
            undefined,
            imageNatural,
          );
          await emitToRegionId(regionId, regionEventName(OcrRegionEvents.error, regionId), {
            message: msg,
          }).catch(() => undefined);
          return;
        }
        if (st.frameClosed || sessionId !== st.sessionId) return;

        // Empty OCR must NOT kill the region frame — show error in-frame and keep chrome usable.
        if (!ocrResult.text.trim()) {
          st.hasOcr = true;
          // Clear fp so continuous can re-OCR when text appears in the pin region.
          st.lastImageFp = '';
          st.lastOcrText = '';
          await sendToRegionFrame(
            regionId,
            image,
            { text: '', lines: [] },
            '',
            [],
            undefined,
            imageNatural,
          );
          await emitToRegionId(regionId, regionEventName(OcrRegionEvents.error, regionId), {
            message: tr('ocr.noTextRecognized') || 'OCR 没有识别到文本',
          }).catch(() => undefined);
          return;
        }

        const sourceTextTrimmed = ocrResult.text.trim();
        st.hasOcr = true;

        // Auto-detect language from full OCR text
        let effectiveSourceLang = normalizeLang(st.sourceLang);
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
        const effectiveTargetLang = normalizeLang(st.targetLang);

        // Skip tiny OCR jitter (same content with 1-2 char noise / whitespace)
        const normalized = normalizeOcrText(sourceTextTrimmed);
        const prevNormalized = normalizeOcrText(st.lastOcrText);
        const similar =
          !!prevNormalized &&
          (normalized === prevNormalized ||
            textSimilarity(normalized, prevNormalized) >= OCR_TEXT_SIMILARITY_SKIP);
        let translatedText = '';
        let lineTranslations: string[] = [];

        if (!similar || !st.lastOcrText) {
          st.lastOcrText = sourceTextTrimmed;
          st.lastImageFp = fp;

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
              regionId,
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
            await emitToRegionId(regionId, regionEventName(OcrRegionEvents.error, regionId), {
              message: tr('ocr.noTextRecognized') || 'OCR 没有识别到文本',
            }).catch(() => undefined);
          } else if (
            effectiveSourceLang !== 'auto' &&
            effectiveSourceLang === effectiveTargetLang
          ) {
            // Same lang: show source as "translation", soft hint (not red error blocking UI).
            translatedText = sourceTextTrimmed;
            lineTranslations = allLines.map((l) => l.text);
            void emitToRegionId(regionId, regionEventName(OcrRegionEvents.hint, regionId), {
              message:
                tr('ocr.sameLang') ||
                `源语言与目标语言相同（${effectiveTargetLang}），请切换目标语`,
            }).catch(() => undefined);
          } else {
            try {
              const sourcePieces = nonEmptyIdx.map((i) => allLines[i].text);
              // Dict-first for single word (ECDICT/Youdao via lookup_dictionary) — skip full MT on hit.
              let dictHit = false;
              const dictCandidate =
                sourcePieces.length === 1
                  ? sourcePieces[0].trim()
                  : isOcrSingleWord(sourceTextTrimmed)
                    ? sourceTextTrimmed
                    : '';
              if (dictCandidate && isOcrSingleWord(dictCandidate)) {
                try {
                  const [dictResults] = await safeInvoke<DictionaryResult[]>(
                    'lookup_dictionary',
                    { text: dictCandidate },
                    { silent: true },
                  );
                  const body = dictResults && formatOcrDictBody(dictCandidate, dictResults);
                  if (body) {
                    if (st.frameClosed || sessionId !== st.sessionId) return;
                    lineTranslations[nonEmptyIdx[0]] = body;
                    for (let p = 1; p < nonEmptyIdx.length; p++) {
                      lineTranslations[nonEmptyIdx[p]] = '';
                    }
                    translatedText = body;
                    dictHit = true;
                  }
                } catch {
                  // miss / error → fall through to MT
                }
              }

              // Batch path (translate_embedded → run_batch): one IPC, concurrent segments.
              // Avoid N× full translate for ≤5 lines and fragile whole-blob align for many lines.
              if (!dictHit && sourcePieces.length === 1) {
                const response = await invokeOrThrow<TranslateResponse>('translate', {
                  request: {
                    text: sourcePieces[0].trim(),
                    from: effectiveSourceLang,
                    to: effectiveTargetLang,
                    channel: 'ocr',
                    // M4: per-region translate engine override ('' = global primary).
                    engine: st.engine || undefined,
                  },
                });
                const out = response.results[0]?.text?.trim() || sourcePieces[0];
                if (st.frameClosed || sessionId !== st.sessionId) return;
                lineTranslations[nonEmptyIdx[0]] = out;
                translatedText = out;
              } else if (!dictHit) {
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
                  // M4: per-region translate engine override ('' = global primary).
                  engine: st.engine || undefined,
                });
                if (st.frameClosed || sessionId !== st.sessionId) return;
                // Prefer batch lineNumber order; on count mismatch pack via ocrLineAlign.
                const byOrder = [...batch].sort((a, b) => a.lineNumber - b.lineNumber);
                const batchLines = byOrder.map((b) => b.translated.trim() || '');
                const sourcePiecesTrim = sourcePieces.map((s) => s.trim());
                const aligned =
                  batchLines.length === sourcePiecesTrim.length
                    ? batchLines
                    : alignTranslationToLines(
                        sourcePiecesTrim,
                        batchLines.filter(Boolean).join('\n') ||
                          byOrder.map((b) => b.translated || '').join('\n'),
                      );
                for (let p = 0; p < nonEmptyIdx.length; p++) {
                  const tLine = aligned[p]?.trim() || '';
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
              await emitToRegionId(regionId, regionEventName(OcrRegionEvents.error, regionId), {
                message:
                  tr('ocr.translateFailed') || '翻译失败：引擎无结果（请检查密钥/网络/引擎开关）',
              }).catch(() => undefined);
            }
          }
          if (translatedText) {
            st.lastTranslated = translatedText;
            st.lastLineTranslations = lineTranslations;
          }
          await sendToRegionFrame(
            regionId,
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
          st.lastImageFp = fp;
          const kept =
            st.lastLineTranslations.length === ocrResult.lines.length
              ? st.lastLineTranslations
              : ocrResult.lines.map((l) => l.text);
          const keptText = st.lastTranslated || sourceTextTrimmed;
          await sendToRegionFrame(
            regionId,
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
        const sessionAlive = !st.frameClosed && sessionId === st.sessionId;
        if (hidRegionFrame && sessionAlive) {
          await safeInvoke(
            'set_ocr_region_frame_sampling',
            { sampling: false, id: regionId },
            { silent: true },
          );
        } else if (hidRegionFrame) {
          // Session cancelled mid-grab: clear affinity without forcing show (Rust still shows —
          // best-effort: re-hide if we intended the frame gone).
          await safeInvoke(
            'set_ocr_region_frame_sampling',
            { sampling: false, id: regionId },
            { silent: true },
          );
          if (st.frameClosed) {
            await safeInvoke(
              'set_ocr_region_frame_visible',
              { visible: false },
              { silent: true },
            );
          }
        }
        if (sessionAlive) {
          void emitToRegionId(regionId, regionEventName(OcrRegionEvents.loading, regionId), {
            loading: false,
          }).catch(() => undefined);
        }
        st.busy = false;
        const pendingRegion = st.pendingRegion;
        if (pendingRegion && sessionAlive) {
          st.pendingRegion = null;
          queueMicrotask(() => {
            if (st.frameClosed || sessionId !== st.sessionId) return;
            void captureAndTranslate(regionId, pendingRegion);
          });
        } else if (!sessionAlive) {
          st.pendingRegion = null;
        }
      }
    },
    [getRegionState, sendToRegionFrame],
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
    void registerListener<OcrRegionPositionPayload & { regionId?: string }>(
      OcrMainEvents.positionChanged,
      (event) => {
        const rid = event.payload.regionId ?? DEFAULT_REGION_ID;
        const st = getRegionState(rid);
        const r = event.payload;
        const prev = st.region;
        // Drag: X/Y from frame; keep true crop size (ignore min-width expanded getCaptureRegion).
        st.region = {
          x: r.x,
          y: r.y,
          width: prev && prev.width > 0 ? prev.width : r.width,
          height: prev && prev.height > 0 ? prev.height : r.height,
        };
        if (st.followEnabled && st.windowBinding) {
          st.windowBinding.setRegionRef(st.region);
          void st.windowBinding.refreshOffset(st.region);
        }
      },
    );

    // Size changed (user resize) — adopt size after first OCR; debounce re-OCR while dragging corner.
    let resizeOcrTimer: ReturnType<typeof setTimeout> | null = null;
    void registerListener<OcrRegionPositionPayload & { regionId?: string }>(
      OcrMainEvents.sizeChanged,
      (event) => {
        const rid = event.payload.regionId ?? DEFAULT_REGION_ID;
        const st = getRegionState(rid);
        const r = event.payload;
        const prev = st.region;
        if (!st.hasOcr) {
          if (prev) {
            st.region = { ...prev, x: r.x, y: r.y };
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
        st.region = { x: r.x, y: r.y, width: nextW, height: nextH };
        if (st.followEnabled && st.windowBinding) {
          st.windowBinding.setRegionRef(st.region);
          void st.windowBinding.refreshOffset(st.region);
        }
        const effectiveChanged =
          !prev || Math.abs(prev.width - nextW) > 2 || Math.abs(prev.height - nextH) > 2;
        if (!effectiveChanged) return;
        if (resizeOcrTimer) window.clearTimeout(resizeOcrTimer);
        resizeOcrTimer = window.setTimeout(() => {
          resizeOcrTimer = null;
          if (st.frameClosed) return;
          const cur = st.region;
          if (!cur) return;
          st.lastOcrText = '';
          st.lastImageFp = '';
          if (st.busy) {
            // Coalesce like other busy paths — don't drop the final size.
            st.pendingRegion = { x: cur.x, y: cur.y, width: cur.width, height: cur.height };
            return;
          }
          void captureAndTranslate(rid, { x: cur.x, y: cur.y, width: cur.width, height: cur.height });
        }, 180);
      },
    );

    // Manual refresh
    void registerListener<{ regionId?: string }>(OcrMainEvents.refresh, (event) => {
      const rid = event.payload?.regionId ?? DEFAULT_REGION_ID;
      const st = getRegionState(rid);
      if (st.region) {
        st.lastOcrText = '';
        st.lastImageFp = '';
        void captureAndTranslate(rid, st.region);
      }
    });

    // Continuous toggle — only user gesture should enable; never auto-on
    void registerListener<{ enabled: boolean; regionId?: string }>(
      OcrMainEvents.continuous,
      (event) => {
        const rid = event.payload.regionId ?? DEFAULT_REGION_ID;
        const st = getRegionState(rid);
        const enabled = !!event.payload.enabled;
        st.continuous = enabled;
        if (enabled) {
          st.consecutiveSkip = 0;
          // Force next tick to re-fingerprint; effect schedules first sample (avoid double OCR here).
          st.lastImageFp = '';
        }
        setContinuous(enabled);
        bumpRegionsVersion();
      },
    );

    // Follow target window toggle (pin region to a window and track moves)
    void registerListener<{ enabled: boolean; regionId?: string }>(OcrMainEvents.follow, (event) => {
      const rid = event.payload.regionId ?? DEFAULT_REGION_ID;
      const st = getRegionState(rid);
      const enabled = event.payload.enabled;
      st.followEnabled = enabled;
      const binding = windowBindingRef.current;
      if (!binding) return;

      if (!enabled) {
        binding.unbind();
        if (followRegionIdRef.current === rid) followRegionIdRef.current = null;
        return;
      }
      followRegionIdRef.current = rid;

      const region = st.region;
      if (!region) return;
      binding.setRegionRef(region);
      // Click-through (no hide flash) so hwnd_from_point hits content under the frame (I6).
      void (async () => {
        const followSession = st.sessionId;
        let restoredClick = false;
        try {
          await safeInvoke(
            'set_ocr_region_frame_click_through',
            { ignore: true, id: rid },
            { silent: true },
          );
          await new Promise((r) => window.setTimeout(r, 16));
          if (st.frameClosed || followSession !== st.sessionId) return;
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
          if (st.frameClosed || followSession !== st.sessionId) {
            binding.unbind();
            return;
          }
          await safeInvoke(
            'set_ocr_region_frame_click_through',
            { ignore: false, id: rid },
            { silent: true },
          );
          restoredClick = true;
          if (!bound) {
            console.warn('[OCR] Failed to bind target window for follow mode');
            st.followEnabled = false;
            void emitToRegionId(rid, regionEventName(OcrRegionEvents.followState, rid), {
              enabled: false,
            }).catch(() => undefined);
            void emitToRegionId(rid, regionEventName(OcrRegionEvents.hint, rid), {
              message: '无法跟随目标窗口（请点在内容上再开跟随）',
            }).catch(() => undefined);
          }
        } finally {
          if (!restoredClick) {
            await safeInvoke(
              'set_ocr_region_frame_click_through',
              { ignore: false, id: rid },
              { silent: true },
            );
          }
        }
      })();
    });

    // Language change from region frame
    void registerListener<OcrRegionLangChangePayload & { regionId?: string }>(
      OcrMainEvents.langChange,
      (event) => {
        const rid = event.payload.regionId ?? DEFAULT_REGION_ID;
        const st = getRegionState(rid);
        const p = event.payload;
        st.langOverridden = true;
        st.sourceLang = p.sourceLang;
        st.targetLang = p.targetLang;
        st.lastOcrText = '';
        st.lastImageFp = '';
        if (st.region) {
          void captureAndTranslate(rid, st.region);
        }
      },
    );

    // Engine switch / enable from region frame toolbar
    void registerListener<OcrRegionEngineChangePayload>(OcrMainEvents.engineChange, (event) => {
      const { engineId, enabled, promote } = event.payload || {};
      if (!engineId) return;
      const rid = (event.payload as { regionId?: string }).regionId ?? DEFAULT_REGION_ID;
      const st = getRegionState(rid);
      // M4: per-region engine selection — the frame's engine dropdown chooses
      // THIS region's engine (independent of the global default). Global
      // enable/promote stays global for engine management, but selecting an
      // engine in a region never clobbers the global primary order.
      const perRegion = (event.payload as { perRegion?: boolean }).perRegion === true;
      if (perRegion) {
        st.engine = engineId;
        // Mirror to backend RegionSession so `ocr_region_list` exposes it.
        void invokeOrThrow('ocr_region_set_engine', {
          id: rid,
          engine: engineId,
        }).catch((e: unknown) => console.warn('[OCR] sync per-region engine failed:', e));
      } else {
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
      }
      st.lastOcrText = '';
      st.lastImageFp = '';
      if (st.region) {
        void captureAndTranslate(rid, st.region);
      }
    });

    // Region frame closed → OCR session ends → main may return (lifecycle, not focus fight).
    void registerListener<{ regionId?: string }>(OcrMainEvents.close, async (event) => {
      const rid = event.payload?.regionId ?? DEFAULT_REGION_ID;
      const st = getRegionState(rid);
      st.frameClosed = true;
      st.sessionId += 1;
      st.pendingRegion = null;
      st.region = null;
      st.lastOcrText = '';
      st.lastTranslated = '';
      st.lastLineTranslations = [];
      st.lastImageFp = '';
      st.consecutiveSkip = 0;
      st.hasOcr = false;
      st.langOverridden = false;
      st.followEnabled = false;
      st.windowBinding?.unbind();
      setContinuous(false);
      bumpRegionsVersion();
      // C6+C7: session closed — return to idle so the next OCR launch can select.
      selectionPhaseRef.current = 'idle';
      selectionTakenRef.current = false;
      ocrSessionActiveRef.current = false;
      await safeInvoke(
        'set_ocr_region_frame_click_through',
        { ignore: false, id: rid },
        { silent: true },
      );
      await safeInvoke(
        'set_ocr_region_frame_sampling',
        { sampling: false, id: rid },
        { silent: true },
      );
      await safeInvoke(
        'ocr_end_session',
        { id: rid },
        { silent: true },
      ).catch(() => undefined);
    });

    return () => {
      cancelled = true;
      if (resizeOcrTimer) window.clearTimeout(resizeOcrTimer);
      unlisteners.forEach((fn) => fn());
      regionsRef.current.forEach((r) => r.windowBinding?.unbind());
    };
  }, [captureAndTranslate, getRegionState, isTauri, updateConfig, saveConfig, bumpRegionsVersion]);

  // Window-binding manager for follow mode (move region frame with target window)
  // M3: a single binding tracks one target at a time; `followRegionIdRef` routes
  // its callbacks to the region that most recently enabled follow.
  useEffect(() => {
    if (!isTauri) return;

    const binding = new WindowBindingManager({
      onRegionUpdate: (newRegion) => {
        const rid = followRegionIdRef.current;
        const st = rid ? getRegionState(rid) : null;
        if (!st || st.frameClosed || !st.followEnabled) return;
        st.region = newRegion;
        if (st.movingFrame) return;
        st.movingFrame = true;
        void safeInvoke(
          'move_ocr_region_frame',
          {
            x: newRegion.x,
            y: newRegion.y,
            width: newRegion.width,
            height: newRegion.height,
            id: rid,
          },
          { silent: true },
        ).finally(() => {
          if (st) st.movingFrame = false;
        });
      },
      onWindowMinimized: () => {
        // Keep last translation; pause continuous capture while minimized
        const rid = followRegionIdRef.current;
        if (!rid) return;
        const st = getRegionState(rid);
        if (st.continuous) {
          st.continuousWasOnBeforeMinimize = true;
          st.continuous = false;
          setContinuous(false);
          void emitToRegionId(rid, regionEventName(OcrRegionEvents.continuousState, rid), {
            enabled: false,
          }).catch(() => undefined);
        } else {
          st.continuousWasOnBeforeMinimize = false;
        }
      },
      onWindowRestored: () => {
        const rid = followRegionIdRef.current;
        if (!rid) return;
        const st = getRegionState(rid);
        if (!st.continuousWasOnBeforeMinimize || st.frameClosed) return;
        st.continuousWasOnBeforeMinimize = false;
        st.continuous = true;
        setContinuous(true);
        void emitToRegionId(rid, regionEventName(OcrRegionEvents.continuousState, rid), {
          enabled: true,
        }).catch(() => undefined);
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
  }, [getRegionState, isTauri]);

  // ---- Continuous / pinned region watch (SKELETON — product reserved) ----
  // Spec: docs/OCR_STRATEGY.md. Tick: sampling exclude→GDI→fingerprint/OCR.
  // Adaptive delay: more consecutive skips → longer wait (less CPU when idle).
  // M3.4: per-region continuous loops. Runs while ANY region has continuous on;
  // each tick scans all regions and captures those ready (continuous + region +
  // not busy + frame open). The legacy default frame's toolbar drives the
  // `continuous` React state (UI mirror); non-default regions toggle their own
  // `st.continuous` via the per-region continuous event.
  useEffect(() => {
    const anyContinuous = continuous || Array.from(regionsRef.current.values()).some((s) => s.continuous);
    if (!anyContinuous) return;

    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const schedule = () => {
      if (cancelled) return;
      const anySkips = Array.from(regionsRef.current.values()).reduce(
        (max, s) => Math.max(max, s.consecutiveSkip),
        0,
      );
      const delay = Math.min(
        ocrIntervalMs * 3,
        Math.round(ocrIntervalMs * (1 + anySkips * 0.25)),
      );
      timer = window.setTimeout(async () => {
        if (cancelled) return;
        for (const [rid, s] of regionsRef.current) {
          const r = s.region;
          if (
            r &&
            s.continuous &&
            !s.busy &&
            !s.frameClosed &&
            !s.movingFrame
          ) {
            await captureAndTranslate(rid, r);
          }
        }
        schedule();
      }, delay);
    };

    // First sample quickly so enabling watch feels live (not a full interval wait).
    const firstDelay = Math.min(600, Math.max(200, Math.floor(ocrIntervalMs * 0.35)));
    timer = window.setTimeout(() => {
      void (async () => {
        if (cancelled) return;
        for (const [rid, s] of regionsRef.current) {
          const r = s.region;
          if (
            r &&
            s.continuous &&
            !s.busy &&
            !s.frameClosed &&
            !s.movingFrame
          ) {
            await captureAndTranslate(rid, r);
          }
        }
        schedule();
      })();
    }, firstDelay);

    return () => {
      cancelled = true;
      if (timer) window.clearTimeout(timer);
    };
  }, [continuous, regionsVersion, captureAndTranslate, ocrIntervalMs]);

  // ---- Screenshot selection listener ----
  useEffect(() => {
    if (!isTauri) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<ScreenshotRegion>('ocr-screenshot-selected', async (event) => {
      if (cancelled) return; // guard against StrictMode double-mount
      // C6+C7: take-once — only the first selection event per session is
      // processed. If the selector emits twice (race between pointerup and
      // key-Esc-cancel, or a double-fire from the selector's finishingRef),
      // the second event is dropped instead of creating a second frame.
      if (selectionPhaseRef.current !== 'selecting' || selectionTakenRef.current) {
        console.warn(
          '[OCR] ignoring duplicate ocr-screenshot-selected (phase=%s, taken=%s)',
          selectionPhaseRef.current,
          selectionTakenRef.current,
        );
        return;
      }
      selectionTakenRef.current = true;
      selectionPhaseRef.current = 'captured';
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

      const region: RegionRect = { x: screenX, y: screenY, width: screenW, height: screenH };
      // M3: selection creates the legacy default region session.
      const selRegionId = DEFAULT_REGION_ID;
      const selSt = getRegionState(selRegionId);
      selSt.region = region;

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
        selSt.frameClosed = false;
        ocrSessionActiveRef.current = true;
        await safeInvoke(
          'set_ocr_region_frame_click_through',
          { ignore: false, id: selRegionId },
          { silent: true },
        );
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

      selSt.continuous = false;
      setContinuous(false);
      bumpRegionsVersion();
      if (selectorSafetyTimerRef.current) {
        window.clearTimeout(selectorSafetyTimerRef.current);
        selectorSafetyTimerRef.current = null;
      }

      // Instant preview while OCR/translate run.
      void emitToRegionId(selRegionId, REGION_EVENTS_BY_ID.text(selRegionId), {
        screenshot: image,
        sourceText: '',
        translatedText: '',
        ocrLines: [],
        lineTranslations: [],
        sourceLang: selSt.sourceLang,
        targetLang: selSt.targetLang,
        refreshIntervalMs: ocrIntervalMsRef.current,
      }).catch(() => undefined);

      const frameReady = await waitForOcrRegionFrameReady(OCR_FRAME_READY_TIMEOUT_MS, selRegionId);
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
          const ready2 = await waitForOcrRegionFrameReady(OCR_FRAME_READY_TIMEOUT_MS, selRegionId);
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
      void emitToRegionId(
        selRegionId,
        regionEventName(OcrRegionEvents.continuousState, selRegionId),
        { enabled: false },
      ).catch(() => undefined);
      const resetOk = await waitForSessionResetAck(OCR_SESSION_RESET_ACK_TIMEOUT_MS);
      if (!resetOk) {
        console.warn('[OCR] session-reset ack timeout — continuing');
      }
      void emitToRegionId(selRegionId, REGION_EVENTS_BY_ID.text(selRegionId), {
        screenshot: image,
        sourceText: '',
        translatedText: '',
        ocrLines: [],
        lineTranslations: [],
        sourceLang: selSt.sourceLang,
        targetLang: selSt.targetLang,
        refreshIntervalMs: ocrIntervalMsRef.current,
      }).catch(() => undefined);

      selSt.lastOcrText = '';
      selSt.lastImageFp = '';
      try {
        await captureAndTranslate(selRegionId, region, image);
      } catch (err) {
        if (cancelled) return;
        console.error('[OCR] captureAndTranslate failed after selection:', err);
        void emitToRegionId(selRegionId, regionEventName(OcrRegionEvents.error, selRegionId), {
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
  }, [captureAndTranslate, getRegionState, isTauri, updateConfig, saveConfig, bumpRegionsVersion]);

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
      // C6: cancelled from selecting → back to idle.
      selectionPhaseRef.current = 'idle';
      selectionTakenRef.current = false;
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

    // M3: session start resets the legacy default region session.
    const st = getRegionState(DEFAULT_REGION_ID);
    st.continuous = false;
    st.frameClosed = true;
    st.sessionId += 1;
    st.pendingRegion = null;
    setContinuous(false);
    bumpRegionsVersion();
    st.region = null;
    st.lastOcrText = '';
    st.lastTranslated = '';
    st.lastLineTranslations = [];
    st.lastImageFp = '';
    st.consecutiveSkip = 0;
    st.hasOcr = false;
    st.langOverridden = false;
    st.followEnabled = false;
    ocrSessionActiveRef.current = true;
    // C6+C7: reset selection phase + take-once at session start.
    // M3.5: while the selector is up, pause ALL regions' continuous watch so a
    // live region's sampling tick never interferes with the user's new selection
    // (design §6 selector/active-region mutex). Each frame sees continuous:false.
    for (const [rid, rs] of regionsRef.current) {
      if (rs.continuous) {
        rs.continuous = false;
        rs.consecutiveSkip = 0;
        void emitToRegionId(rid, regionEventName(OcrRegionEvents.continuousState, rid), {
          enabled: false,
        }).catch(() => undefined);
      }
    }
    setContinuous(false);
    bumpRegionsVersion();
    selectionPhaseRef.current = 'idle';
    selectionTakenRef.current = false;
    windowBindingRef.current?.unbind();
    followRegionIdRef.current = null;
    clearSelectorSafetyTimer();

    try {
      await safeInvoke(
        'set_ocr_region_frame_sampling',
        { sampling: false, id: DEFAULT_REGION_ID },
        { silent: true },
      );
      await safeInvoke('set_ocr_region_frame_visible', { visible: false }, { silent: true });
      void emitToRegionId(DEFAULT_REGION_ID, regionEventName(OcrRegionEvents.continuousState, DEFAULT_REGION_ID), {
        enabled: false,
      }).catch(() => undefined);
      void emitToRegionId(DEFAULT_REGION_ID, regionEventName(OcrRegionEvents.sessionReset, DEFAULT_REGION_ID), null).catch(
        () => undefined,
      );
      await safeInvoke('close_ocr_screenshot_selector', undefined, { silent: true });
      st.frameClosed = true;

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
        const readyPromise = waitForOcrScreenshotReady(OCR_SCREENSHOT_READY_TIMEOUT_MS);
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
        // C6: selector is up and accepting input — enter `selecting` phase.
        // C7: arm take-once so a duplicate `ocr-screenshot-selected` event
        // does not trigger a second crop / frame create.
        selectionTakenRef.current = false;
        selectionPhaseRef.current = 'selecting';
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
            if (!st.frameClosed) return; // result already up
            // C6: only abort if still in selecting phase (captured means hand-off done).
            if (selectionPhaseRef.current !== 'selecting') return;
            console.warn('[OCR] Selector stuck 20s — end session, restore main');
            ocrSessionActiveRef.current = false;
            selectionPhaseRef.current = 'idle';
            selectionTakenRef.current = false;
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
  }, [isTauri, t, clearSelectorSafetyTimer, getRegionState]);

  useEffect(() => {
    if (launchNonce <= 0) return;
    void startScreenshotTranslate();
  }, [launchNonce, startScreenshotTranslate]);

  return null;
}
