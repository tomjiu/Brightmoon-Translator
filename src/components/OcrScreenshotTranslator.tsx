import { useCallback, useEffect, useRef, useState } from "react";
import { listen, emitTo } from "@tauri-apps/api/event";
import { safeInvoke, invokeOrThrow } from "../services/invoke";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ScanLine, Search } from "lucide-react";
import {
  captureScreenshotRegion,
  ocrImagePreferNativeDetailed,
  prepareScreenshotSnapshot,
  type ScreenshotSnapshotInfo,
  type ScreenshotRegion,
  type OcrResultDetailed,
} from "../services/ocr";
import { useConfigStore } from "../stores/configStore";
import type { TranslateResponse, DetectionResult, TextRegion } from "../types";

type OcrStatus = "idle" | "capturing" | "selecting" | "running" | "error";

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
    const timer = window.setTimeout(() => reject(new Error(message)), timeoutMs);
    promise.then(
      (value) => {
        window.clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        window.clearTimeout(timer);
        reject(error);
      },
    );
  });
}

export default function OcrScreenshotTranslator({ launchNonce = 0 }: OcrScreenshotTranslatorProps) {
  const config = useConfigStore((state) => state.config);
  const [status, setStatus] = useState<OcrStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const [ocrSource, setOcrSource] = useState<string | null>(null);
  const [translation, setTranslation] = useState<string | null>(null);
  const [detectedLang, setDetectedLang] = useState<string | null>(null);
  const [continuous, setContinuous] = useState(false);
  const [snapshotInfo, setSnapshotInfo] = useState<ScreenshotSnapshotInfo | null>(null);
  const [detectedRegions, setDetectedRegions] = useState<TextRegion[]>([]);
  const [detectingRegions, setDetectingRegions] = useState(false);

  // Refs for stable access inside callbacks/intervals
  const regionRef = useRef<RegionRect | null>(null);
  const snapshotInfoRef = useRef<ScreenshotSnapshotInfo | null>(null);
  const busyRef = useRef(false);
  const continuousRef = useRef(false);
  const lastOcrTextRef = useRef<string>("");
  const hasOcrRef = useRef(false); // Track whether OCR has been performed (avoids ocrSource dependency)
  const sourceLangRef = useRef(config.defaultFrom);
  const targetLangRef = useRef(config.defaultTo);

  // Keep refs in sync
  useEffect(() => {
    snapshotInfoRef.current = snapshotInfo;
  }, [snapshotInfo]);
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
      try {
        await emitTo("ocr-region-frame", "ocr-region-update-data", {
          screenshot,
          sourceText: ocrResult.text,
          translatedText,
          ocrLines: ocrResult.lines,
          lineTranslations,
          sourceLang: sourceLangRef.current,
          targetLang: targetLangRef.current,
          detectedLang: detectedLangName,
        });
      } catch {
        // region frame may not be ready yet
      }
    },
    [],
  );

  // ---- Core OCR + Translate pipeline ----
  const captureAndTranslate = useCallback(
    async (region: RegionRect) => {
      if (busyRef.current) return;
      busyRef.current = true;
      setStatus("running");
      setError(null);

      try {
        const sRegion: ScreenshotRegion = {
          left: Math.round(region.x),
          top: Math.round(region.y),
          width: Math.round(region.width),
          height: Math.round(region.height),
        };

        const image = await captureScreenshotRegion(sRegion);
        const ocrResult = await ocrImagePreferNativeDetailed(image, "auto");
        if (!ocrResult.text.trim()) {
          throw new Error("OCR 没有识别到文本");
        }

        const sourceTextTrimmed = ocrResult.text.trim();
        setOcrSource(sourceTextTrimmed);
        hasOcrRef.current = true;

        // Auto-detect language from full OCR text
        let effectiveSourceLang = sourceLangRef.current;
        if (sourceLangRef.current === "auto" && sourceTextTrimmed.length >= 2) {
          try {
            const detected = await invokeOrThrow<DetectionResult>("detect_language", {
              text: sourceTextTrimmed,
            });
            if (detected.language !== "auto") {
              effectiveSourceLang = detected.language;
              setDetectedLang(detected.name);
            } else {
              setDetectedLang(null);
            }
          } catch {
            // Language detection failure is non-fatal, continue with "auto"
            setDetectedLang(null);
          }
        } else {
          setDetectedLang(null);
        }

        // Only translate if the OCR text actually changed
        const textChanged = sourceTextTrimmed !== lastOcrTextRef.current;
        let translatedText = "";
        let lineTranslations: string[] = [];

        if (textChanged || !lastOcrTextRef.current) {
          lastOcrTextRef.current = sourceTextTrimmed;

          // Translate each line separately for immersive replacement
          const lines = ocrResult.lines.filter(l => l.text.trim().length > 0);
          const translatePromises = lines.map(async (line) => {
            try {
              const response = await invokeOrThrow<TranslateResponse>("translate", {
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
          translatedText = lineTranslations.join("\n");
          setTranslation(translatedText);
        }

        // Send screenshot + OCR + translation to the merged region frame
        await sendToRegionFrame(image, ocrResult, translatedText, lineTranslations, detectedLang ?? undefined);
      } catch (err) {
        setError(String(err));
        throw err; // Re-throw so caller can handle (e.g., close region frame on failure)
      } finally {
        busyRef.current = false;
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
    let cancelled = false;
    const unlisteners: (() => void)[] = [];

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
    void registerListener<RegionRect>("ocr-region-position-changed", (event) => {
      const r = event.payload;
      regionRef.current = { x: r.x, y: r.y, width: r.width, height: r.height };
    });

    // Size changed (resize) — re-run OCR
    void registerListener<RegionRect>("ocr-region-size-changed", (event) => {
      const r = event.payload;
      regionRef.current = { x: r.x, y: r.y, width: r.width, height: r.height };
      if (hasOcrRef.current) {
        lastOcrTextRef.current = "";
        void captureAndTranslate({ x: r.x, y: r.y, width: r.width, height: r.height });
      }
    });

    // Manual refresh
    void registerListener<unknown>("ocr-region-refresh", () => {
      if (regionRef.current) {
        void captureAndTranslate(regionRef.current);
      }
    });

    // Continuous toggle
    void registerListener<{ enabled: boolean }>("ocr-region-continuous", (event) => {
      setContinuous(event.payload.enabled);
    });

    // Language change from region frame
    void registerListener<{ sourceLang: string; targetLang: string }>("ocr-region-lang-change", (event) => {
      const p = event.payload;
      sourceLangRef.current = p.sourceLang;
      targetLangRef.current = p.targetLang;
      lastOcrTextRef.current = "";
      if (regionRef.current) {
        void captureAndTranslate(regionRef.current);
      }
    });

    // Region frame closed
    void registerListener<unknown>("ocr-region-close", async () => {
      setContinuous(false);
      setStatus("idle");
      setError(null);
      setOcrSource(null);
      setTranslation(null);
      setDetectedLang(null);
      regionRef.current = null;
      lastOcrTextRef.current = "";
      hasOcrRef.current = false;
      await safeInvoke("close_ocr_region_frame", undefined, { silent: true });
      await getCurrentWindow().show();
    });

    return () => {
      cancelled = true;
      unlisteners.forEach((fn) => fn());
    };
  }, [captureAndTranslate]);

  // ---- Continuous refresh timer ----
  useEffect(() => {
    if (!continuous || !regionRef.current) return;

    const id = window.setInterval(() => {
      const r = regionRef.current;
      if (r) {
        void captureAndTranslate(r);
      }
    }, 2000);

    return () => window.clearInterval(id);
  }, [continuous, captureAndTranslate]);

  // ---- Screenshot selection listener ----
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<ScreenshotRegion>("ocr-screenshot-selected", async (event) => {
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

      // Close selector window
      await safeInvoke("close_ocr_screenshot_selector", undefined, { silent: true });

      // Create merged region frame window at the selection position
      try {
        await invokeOrThrow("create_ocr_region_frame", {
          x: screenX,
          y: screenY,
          width: screenW,
          height: screenH,
        });
      } catch (err) {
        if (cancelled) return;
        await safeInvoke("close_ocr_region_frame", undefined, { silent: true });
        await getCurrentWindow().show();
        setError(String(err));
        setStatus("error");
        return;
      }

      // Wait briefly for region frame to initialize, then run first OCR + translate
      await new Promise((resolve) => window.setTimeout(resolve, 50));

      lastOcrTextRef.current = "";
      try {
        await captureAndTranslate(region);
      } catch (err) {
        if (cancelled) return;
        console.error("[OCR] captureAndTranslate failed after selection:", err);
        setError(String(err));
        setStatus("error");
        await safeInvoke("close_ocr_region_frame", undefined, { silent: true });
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
  }, [captureAndTranslate]);

  // ---- Cancelled listener ----
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen("ocr-screenshot-cancelled", () => {
      if (cancelled) return;
      void getCurrentWindow().show();
      setStatus("idle");
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
  }, []);

  // Guard ref to prevent concurrent startScreenshotTranslate calls
  const startingRef = useRef(false);

  // ---- Start screenshot translate ----
  const startScreenshotTranslate = useCallback(async () => {
    if (startingRef.current) return;
    startingRef.current = true;

    setStatus("capturing");
    setError(null);
    setOcrSource(null);
    setTranslation(null);
    setContinuous(false);
    regionRef.current = null;
    lastOcrTextRef.current = "";
    hasOcrRef.current = false;

    try {
      // Close existing region frame and selector
      await safeInvoke("close_ocr_region_frame", undefined, { silent: true });
      await safeInvoke("close_ocr_screenshot_selector", undefined, { silent: true });

      const appWindow = getCurrentWindow();
      await appWindow.hide();

      const info = await withTimeout(
        prepareScreenshotSnapshot(),
        10000,
        "屏幕捕获超时：当前桌面会话可能不允许截图",
      );
      setSnapshotInfo(info);
      snapshotInfoRef.current = info;

      await invokeOrThrow("create_ocr_screenshot_selector");
      setStatus("selecting");
    } catch (err) {
      try {
        await getCurrentWindow().show();
      } catch {
        // ignore
      }
      setError(String(err));
      setStatus("error");
    } finally {
      startingRef.current = false;
    }
  }, []);

  useEffect(() => {
    if (launchNonce <= 0) return;
    void startScreenshotTranslate();
  }, [launchNonce, startScreenshotTranslate]);

  const busy = status === "capturing" || status === "selecting" || status === "running";

  // Auto-detect text regions in foreground window
  const autoDetectRegions = useCallback(async () => {
    setDetectingRegions(true);
    setError(null);
    try {
      const regions = await invokeOrThrow<TextRegion[]>("detect_text_regions", { hwnd: null });
      setDetectedRegions(regions);
      if (regions.length === 0) {
        setError("未检测到文本区域");
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setDetectingRegions(false);
    }
  }, []);

  // Select a detected region and start OCR monitoring
  const selectDetectedRegion = useCallback(async (region: TextRegion) => {
    const regionRect: RegionRect = {
      x: region.x,
      y: region.y,
      width: region.width,
      height: region.height,
    };
    regionRef.current = regionRect;
    setDetectedRegions([]);
    lastOcrTextRef.current = "";
    hasOcrRef.current = false;

    // Create region frame
    try {
      await invokeOrThrow("create_ocr_region_frame", {
        x: region.x,
        y: region.y,
        width: region.width,
        height: region.height,
      });
    } catch (err) {
      setError(String(err));
      setStatus("error");
      return;
    }

    await new Promise((resolve) => window.setTimeout(resolve, 50));

    try {
      await captureAndTranslate(regionRect);
    } catch (err) {
      console.error("[OCR] captureAndTranslate failed:", err);
      setError(String(err));
      setStatus("error");
      await safeInvoke("close_ocr_region_frame", undefined, { silent: true });
    }
  }, [captureAndTranslate]);

  return (
    <section className="rounded-2xl border border-border bg-bg-secondary p-5 shadow-sm">
      <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
        <div>
          <div className="flex items-center gap-2 text-lg font-semibold text-text-primary">
            <ScanLine size={20} />
            屏幕 OCR 翻译
          </div>
          <p className="mt-2 max-w-2xl text-sm leading-6 text-text-secondary">
            选区后直接在屏幕原位显示翻译结果，支持自动刷新、原文/译文切换、语言切换和截图复制。
            优先使用 Windows 原生 OCR，失败时回退到 tesseract.js。
          </p>
        </div>

        <div className="flex flex-wrap gap-2">
          <button
            className="rounded-xl bg-primary px-4 py-2 text-sm font-medium text-white shadow-md shadow-primary/20 disabled:cursor-not-allowed disabled:opacity-60"
            onClick={startScreenshotTranslate}
            disabled={busy}
          >
            {busy ? "处理中..." : "开始截图翻译"}
          </button>
          <button
            className="rounded-xl bg-emerald-600 px-4 py-2 text-sm font-medium text-white shadow-md shadow-emerald-600/20 disabled:cursor-not-allowed disabled:opacity-60 flex items-center gap-1"
            onClick={autoDetectRegions}
            disabled={busy || detectingRegions}
          >
            <Search size={16} />
            {detectingRegions ? "检测中..." : "自动检测文本"}
          </button>
        </div>
      </div>

      {/* Detected regions selection */}
      {detectedRegions.length > 0 && (
        <div className="mt-4 rounded-xl border border-emerald-500/30 bg-emerald-500/5 p-4">
          <div className="mb-2 text-sm font-medium text-emerald-400">
            检测到 {detectedRegions.length} 个文本区域，点击选择：
          </div>
          <div className="flex flex-col gap-2">
            {detectedRegions.map((region, idx) => (
              <button
                key={idx}
                className="rounded-lg border border-border bg-bg-primary px-3 py-2 text-left text-sm hover:border-emerald-500/50 hover:bg-emerald-500/10 transition-colors"
                onClick={() => selectDetectedRegion(region)}
              >
                <div className="flex items-center justify-between">
                  <span className="text-text-primary">
                    区域 {idx + 1} ({region.lineCount} 行)
                  </span>
                  <span className="text-text-secondary text-xs">
                    {region.width}×{region.height} @ ({region.x}, {region.y})
                  </span>
                </div>
                {region.textPreview && (
                  <p className="mt-1 text-xs text-text-secondary truncate">
                    {region.textPreview}
                  </p>
                )}
              </button>
            ))}
          </div>
          <button
            className="mt-2 text-xs text-text-secondary hover:text-text-primary"
            onClick={() => setDetectedRegions([])}
          >
            清除检测结果
          </button>
        </div>
      )}

      <div className="mt-4 rounded-xl bg-bg-tertiary px-3 py-2 text-sm text-text-secondary">
        当前状态：
        <span className="ml-1 text-text-primary">
          {status === "idle" && "等待截图"}
          {status === "capturing" && "正在捕获屏幕"}
          {status === "selecting" && "等待选区"}
          {status === "running" && "正在 OCR 与翻译"}
          {status === "error" && "出错"}
        </span>
        {continuous && (
          <span className="ml-2 text-sky-500">持续刷新中 (2s)</span>
        )}
        {detectedLang && (
          <span className="ml-2 text-emerald-500">检测到: {detectedLang}</span>
        )}
      </div>

      {error && (
        <div className="mt-4 rounded-xl border border-red-500/40 bg-red-500/10 p-3 text-sm text-red-500">
          {error}
        </div>
      )}

      {ocrSource && (
        <div className="mt-4 rounded-xl border border-border bg-bg-primary p-4">
          <div className="mb-2 text-sm font-medium text-text-primary">OCR 原文</div>
          <p className="whitespace-pre-wrap text-sm leading-6 text-text-secondary max-h-40 overflow-auto">
            {ocrSource}
          </p>
        </div>
      )}

      {translation && (
        <div className="mt-3 rounded-xl border border-border bg-bg-primary p-4">
          <div className="mb-2 text-sm font-medium text-text-primary">翻译结果</div>
          <p className="whitespace-pre-wrap text-sm leading-6 text-text-primary max-h-40 overflow-auto">
            {translation}
          </p>
        </div>
      )}
    </section>
  );
}
