import { useCallback, useEffect, useRef, useState } from 'react';
import { listen, emitTo } from '@tauri-apps/api/event';
import { safeInvoke, invokeOrThrow } from '../services/invoke';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { isTauriRuntime } from '../services/tauriRuntime';
import { useI18n } from '../i18n';
import {
  captureScreenshotRegion,
  ocrWithEngine,
  prepareScreenshotSnapshot,
  type ScreenshotSnapshotInfo,
  type ScreenshotRegion,
  type OcrResultDetailed,
} from '../services/ocr';
import { useConfigStore } from '../stores/configStore';
import type { TranslateResponse, DetectionResult } from '../types';

interface OcrScreenshotTranslatorProps {
  launchNonce?: number;
}

interface RegionRect {
  x: number;
  y: number;
  width: number;
  height: number;
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
  const isTauri = isTauriRuntime();
  const ocrIntervalMs = Math.max(750, config.ocrInterval ?? 2000);
  const [continuous, setContinuous] = useState(true);

  // Refs for stable access inside callbacks/intervals
  const regionRef = useRef<RegionRect | null>(null);
  const snapshotInfoRef = useRef<ScreenshotSnapshotInfo | null>(null);
  const busyRef = useRef(false);
  const pendingRegionRef = useRef<RegionRect | null>(null);
  const continuousRef = useRef(true);
  const frameClosedRef = useRef(true);
  const sessionIdRef = useRef(0);
  const lastOcrTextRef = useRef<string>('');
  const hasOcrRef = useRef(false); // Track whether OCR has been performed (avoids ocrSource dependency)
  const sourceLangRef = useRef(config.defaultFrom);
  const targetLangRef = useRef(config.defaultTo);

  useEffect(() => {
    continuousRef.current = continuous;
  }, [continuous]);

  // ---- Send data to the merged region frame ----
  const sendToRegionFrame = useCallback(
    async (
      screenshot: string,
      ocrResult: OcrResultDetailed,
      translatedText: string,
      lineTranslations: string[] = [],
      detectedLangName?: string,
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
        refreshIntervalMs: ocrIntervalMs,
      };

      console.log('[OCR] sendToRegionFrame payload:', { ...payload, screenshot: '[base64]' });

      // Retry up to 3 times with increasing delays
      for (let attempt = 0; attempt < 3; attempt++) {
        try {
          console.log('[OCR] emitTo attempt', attempt + 1, '...');
          await emitTo('ocr-region-frame', 'ocr-region-update-data', payload);
          console.log('[OCR] emitTo succeeded on attempt', attempt + 1);
          return; // Success
        } catch (err) {
          console.warn('[OCR] emitTo failed on attempt', attempt + 1, ':', err);
          // region frame may not be ready yet, wait and retry
          if (attempt < 2) {
            await new Promise((resolve) => window.setTimeout(resolve, 200 * (attempt + 1)));
          }
        }
      }
      console.error('[OCR] emitTo failed after all attempts');
    },
    [ocrIntervalMs],
  );

  // ---- Core OCR + Translate pipeline ----
  // If image is provided, skip capture (used when image was captured before region frame creation)
  const captureAndTranslate = useCallback(
    async (region: RegionRect, preCapturedImage?: string) => {
      console.log(
        '[OCR] captureAndTranslate called, busy:',
        busyRef.current,
        'region:',
        region,
        'hasImage:',
        !!preCapturedImage,
      );
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
          console.log('[OCR] Using pre-captured image');
        } else {
          // Fallback: capture now (need to hide region frame first)
          const sRegion: ScreenshotRegion = {
            left: Math.round(region.x),
            top: Math.round(region.y),
            width: Math.round(region.width),
            height: Math.round(region.height),
          };

          console.log('[OCR] Hiding region frame before capture...');
          await safeInvoke('set_ocr_region_frame_visible', { visible: false }, { silent: true });
          hidRegionFrame = true;
          await new Promise((resolve) => window.setTimeout(resolve, 120));
          if (frameClosedRef.current || sessionId !== sessionIdRef.current) return;

          console.log('[OCR] Capturing screenshot region...');
          image = await captureScreenshotRegion(sRegion);
        }
        console.log('[OCR] Screenshot ready, running OCR...');

        const ocrEngine = config.ocrEngine || 'auto';
        const ocrResult = await ocrWithEngine(image, ocrEngine, 'auto');
        console.log('[OCR] OCR result:', ocrResult);
        if (!ocrResult.text.trim()) {
          throw new Error(t('ocr.noTextRecognized') || 'OCR 没有识别到文本');
        }

        const sourceTextTrimmed = ocrResult.text.trim();
        hasOcrRef.current = true;

        // Auto-detect language from full OCR text
        let effectiveSourceLang = sourceLangRef.current;
        let detectedLangName: string | undefined;
        if (sourceLangRef.current === 'auto' && sourceTextTrimmed.length >= 2) {
          try {
            const detected = await invokeOrThrow<DetectionResult>('detect_language', {
              text: sourceTextTrimmed,
            });
            if (detected.language !== 'auto') {
              effectiveSourceLang = detected.language;
              detectedLangName = detected.name;
            }
          } catch {
            // Language detection failure is non-fatal, continue with "auto"
          }
        }

        // Only translate if the OCR text actually changed
        const textChanged = sourceTextTrimmed !== lastOcrTextRef.current;
        let translatedText = '';
        let lineTranslations: string[] = [];

        if (textChanged || !lastOcrTextRef.current) {
          lastOcrTextRef.current = sourceTextTrimmed;

          // Translate each line separately for immersive replacement
          const lines = ocrResult.lines.filter((l) => l.text.trim().length > 0);
          console.log('[OCR] Translating', lines.length, 'lines...');
          const translatePromises = lines.map(async (line) => {
            try {
              const response = await invokeOrThrow<TranslateResponse>('translate', {
                request: {
                  text: line.text.trim(),
                  from: effectiveSourceLang,
                  to: targetLangRef.current,
                },
              });
              return response.results[0]?.text ?? line.text;
            } catch {
              return line.text; // Fallback to original text on error
            }
          });

          lineTranslations = await Promise.all(translatePromises);
          translatedText = lineTranslations.join('\n');
          console.log('[OCR] Translation complete:', translatedText);

          // Only send to region frame when content actually changed
          console.log('[OCR] Sending data to region frame...');
          await sendToRegionFrame(
            image,
            ocrResult,
            translatedText,
            lineTranslations,
            detectedLangName,
          );
          console.log('[OCR] Data sent to region frame');
        } else {
          console.log('[OCR] Content unchanged, skipping update');
        }
      } finally {
        // Only restore visibility if we hid the frame for capture (not needed for pre-captured images)
        if (hidRegionFrame && !frameClosedRef.current && sessionId === sessionIdRef.current) {
          await safeInvoke('set_ocr_region_frame_visible', { visible: true }, { silent: true });
        }
        busyRef.current = false;
        const pendingRegion = pendingRegionRef.current;
        if (pendingRegion && !frameClosedRef.current && sessionId === sessionIdRef.current) {
          pendingRegionRef.current = null;
          void captureAndTranslate(pendingRegion);
        }
      }
    },
    [config.ocrEngine, sendToRegionFrame, t],
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

    // Position changed (drag)
    void registerListener<RegionRect>('ocr-region-position-changed', (event) => {
      const r = event.payload;
      regionRef.current = { x: r.x, y: r.y, width: r.width, height: r.height };
    });

    // Size changed (resize) — re-run OCR
    void registerListener<RegionRect>('ocr-region-size-changed', (event) => {
      const r = event.payload;
      regionRef.current = { x: r.x, y: r.y, width: r.width, height: r.height };
      if (hasOcrRef.current) {
        lastOcrTextRef.current = '';
        void captureAndTranslate({ x: r.x, y: r.y, width: r.width, height: r.height });
      }
    });

    // Manual refresh
    void registerListener<unknown>('ocr-region-refresh', () => {
      if (regionRef.current) {
        void captureAndTranslate(regionRef.current);
      }
    });

    // Continuous toggle
    void registerListener<{ enabled: boolean }>('ocr-region-continuous', (event) => {
      continuousRef.current = event.payload.enabled;
      setContinuous(event.payload.enabled);
    });

    // Language change from region frame
    void registerListener<{ sourceLang: string; targetLang: string }>(
      'ocr-region-lang-change',
      (event) => {
        const p = event.payload;
        sourceLangRef.current = p.sourceLang;
        targetLangRef.current = p.targetLang;
        lastOcrTextRef.current = '';
        if (regionRef.current) {
          void captureAndTranslate(regionRef.current);
        }
      },
    );

    // Region frame closed
    void registerListener<unknown>('ocr-region-close', async () => {
      frameClosedRef.current = true;
      sessionIdRef.current += 1;
      pendingRegionRef.current = null;
      setContinuous(false);
      regionRef.current = null;
      lastOcrTextRef.current = '';
      hasOcrRef.current = false;
      await safeInvoke('close_ocr_region_frame', undefined, { silent: true });
      await getCurrentWindow().show();
    });

    return () => {
      cancelled = true;
      unlisteners.forEach((fn) => fn());
    };
  }, [captureAndTranslate, isTauri]);

  // ---- Continuous refresh timer ----
  useEffect(() => {
    if (!continuous || !regionRef.current) return;

    const id = window.setInterval(() => {
      const r = regionRef.current;
      if (r && continuousRef.current && !busyRef.current) {
        void captureAndTranslate(r);
      }
    }, ocrIntervalMs);

    return () => window.clearInterval(id);
  }, [continuous, captureAndTranslate, ocrIntervalMs]);

  // ---- Screenshot selection listener ----
  useEffect(() => {
    if (!isTauri) return;

    console.log('[OCR] Registering ocr-screenshot-selected listener...');
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<ScreenshotRegion>('ocr-screenshot-selected', async (event) => {
      console.log('[OCR] ocr-screenshot-selected received:', event.payload);
      if (cancelled) return; // guard against StrictMode double-mount
      const sel = event.payload;
      const info = snapshotInfoRef.current;
      const scaleX = info ? info.screenWidth / info.imageWidth : 1;
      const scaleY = info ? info.screenHeight / info.imageHeight : 1;
      const offsetX = info ? info.screenX : 0;
      const offsetY = info ? info.screenY : 0;

      const screenX = Math.round(offsetX + sel.left * scaleX);
      const screenY = Math.round(offsetY + sel.top * scaleY);
      const screenW = Math.round(sel.width * scaleX);
      const screenH = Math.round(sel.height * scaleY);

      const region: RegionRect = { x: screenX, y: screenY, width: screenW, height: screenH };
      regionRef.current = region;

      // Close selector window first
      await safeInvoke('close_ocr_screenshot_selector', undefined, { silent: true });

      // Capture screenshot BEFORE creating region frame (no flickering!)
      const sRegion: ScreenshotRegion = {
        left: screenX,
        top: screenY,
        width: screenW,
        height: screenH,
      };

      console.log('[OCR] Capturing screenshot region...');
      let image: string;
      try {
        image = await captureScreenshotRegion(sRegion);
      } catch (err) {
        if (cancelled) return;
        console.error('[OCR] Screenshot capture failed:', err);
        await getCurrentWindow().show();
        return;
      }

      // Now create region frame (will receive OCR results)
      try {
        await invokeOrThrow('create_ocr_region_frame', {
          x: screenX,
          y: screenY,
          width: screenW,
          height: screenH,
        });
        frameClosedRef.current = false;
      } catch (err) {
        if (cancelled) return;
        await safeInvoke('close_ocr_region_frame', undefined, { silent: true });
        await getCurrentWindow().show();
        console.error('[OCR] Failed to create region frame:', err);
        return;
      }

      // Enable continuous mode by default when creating region frame
      continuousRef.current = true;
      setContinuous(true);

      // Wait for region frame to initialize
      await new Promise((resolve) => window.setTimeout(resolve, 200));

      lastOcrTextRef.current = '';
      try {
        // Run OCR and translation with already-captured image
        await captureAndTranslate(region, image);
      } catch (err) {
        if (cancelled) return;
        console.error('[OCR] captureAndTranslate failed after selection:', err);
        await safeInvoke('close_ocr_region_frame', undefined, { silent: true });
        await getCurrentWindow().show();
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
  }, [captureAndTranslate, isTauri]);

  // ---- Cancelled listener ----
  useEffect(() => {
    if (!isTauri) return;

    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen('ocr-screenshot-cancelled', () => {
      if (cancelled) return;
      void getCurrentWindow().show();
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
  }, [isTauri]);

  // Guard ref to prevent concurrent startScreenshotTranslate calls
  const startingRef = useRef(false);

  // ---- Start screenshot translate ----
  const startScreenshotTranslate = useCallback(async () => {
    if (!isTauri) return;

    console.log('[OCR] startScreenshotTranslate called, startingRef:', startingRef.current);
    if (startingRef.current) return;
    startingRef.current = true;

    continuousRef.current = false;
    frameClosedRef.current = true;
    sessionIdRef.current += 1;
    pendingRegionRef.current = null;
    setContinuous(false);
    regionRef.current = null;
    lastOcrTextRef.current = '';
    hasOcrRef.current = false;

    try {
      console.log('[OCR] Closing existing windows...');
      // Close existing region frame and selector
      await safeInvoke('close_ocr_region_frame', undefined, { silent: true });
      await safeInvoke('close_ocr_screenshot_selector', undefined, { silent: true });

      console.log('[OCR] Hiding main window...');
      const appWindow = getCurrentWindow();
      await appWindow.hide();

      console.log('[OCR] Preparing screenshot snapshot...');
      const info = await withTimeout(
        prepareScreenshotSnapshot(),
        10000,
        t('ocr.captureTimeout') || '屏幕捕获超时：当前桌面会话可能不允许截图',
      );
      console.log('[OCR] Snapshot prepared:', info);
      snapshotInfoRef.current = info;

      console.log('[OCR] Creating screenshot selector...');
      await invokeOrThrow('create_ocr_screenshot_selector');
      console.log('[OCR] Selector created, status: selecting');
    } catch (err) {
      console.error('[OCR] Error in startScreenshotTranslate:', err);
      try {
        await getCurrentWindow().show();
      } catch {
        // ignore
      }
    } finally {
      startingRef.current = false;
    }
  }, [isTauri, t]);

  useEffect(() => {
    if (launchNonce <= 0) return;
    void startScreenshotTranslate();
  }, [launchNonce, startScreenshotTranslate]);

  return null;
}
