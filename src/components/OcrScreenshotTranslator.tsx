import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { ScanLine } from "lucide-react";
import {
  captureScreenshotRegion,
  ocrImagePreferNative,
  prepareScreenshotSnapshot,
  type ScreenshotSnapshotInfo,
  type ScreenshotRegion,
} from "../services/ocr";
import { calculateOcrResultOverlayRect } from "../services/ocrOverlayPosition";
import { useConfigStore } from "../stores/configStore";
import type { TranslateResponse } from "../types";

type OcrStatus = "idle" | "capturing" | "selecting" | "running" | "error";

interface RegionRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export default function OcrScreenshotTranslator() {
  const config = useConfigStore((state) => state.config);
  const [status, setStatus] = useState<OcrStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const [ocrSource, setOcrSource] = useState<string | null>(null);
  const [translation, setTranslation] = useState<string | null>(null);
  const [continuous, setContinuous] = useState(false);
  const [snapshotInfo, setSnapshotInfo] = useState<ScreenshotSnapshotInfo | null>(null);

  // Refs for stable access inside callbacks/intervals
  const regionRef = useRef<RegionRect | null>(null);
  const snapshotInfoRef = useRef<ScreenshotSnapshotInfo | null>(null);
  const busyRef = useRef(false);
  const continuousRef = useRef(false);

  // Keep refs in sync
  useEffect(() => {
    snapshotInfoRef.current = snapshotInfo;
  }, [snapshotInfo]);
  useEffect(() => {
    continuousRef.current = continuous;
  }, [continuous]);

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
        const sourceText = await ocrImagePreferNative(image, "auto");
        if (!sourceText.trim()) {
          throw new Error("OCR 没有识别到文本");
        }
        setOcrSource(sourceText.trim());

        const response = await invoke<TranslateResponse>("translate", {
          request: {
            text: sourceText.trim(),
            from: config.defaultFrom,
            to: config.defaultTo,
          },
        });

        const translatedText = response.results[0]?.text ?? "";
        setTranslation(translatedText);

        // Show/update overlay near the region frame
        if (translatedText) {
          const overlay = calculateOcrResultOverlayRect(
            sRegion,
            snapshotInfoRef.current,
          );
          await invoke("update_overlay", {
            x: overlay.x,
            y: overlay.y,
            width: overlay.width,
            height: overlay.height,
            text: translatedText,
            showControls: true,
            source: sourceText.trim(),
          });
        }
      } catch (err) {
        setError(String(err));
      } finally {
        busyRef.current = false;
      }
    },
    [config.defaultFrom, config.defaultTo],
  );

  // ---- Listen for events from the region frame window ----
  useEffect(() => {
    const unlisteners: (() => void)[] = [];

    const setup = async () => {
      // Position changed (drag)
      unlisteners.push(
        await listen<RegionRect>("ocr-region-position-changed", (event) => {
          const r = event.payload;
          regionRef.current = { x: r.x, y: r.y, width: r.width, height: r.height };
          // Reposition overlay
          const sRegion: ScreenshotRegion = {
            left: Math.round(r.x),
            top: Math.round(r.y),
            width: Math.round(r.width),
            height: Math.round(r.height),
          };
          const overlay = calculateOcrResultOverlayRect(sRegion, snapshotInfoRef.current);
          void invoke("update_overlay_position", { x: overlay.x, y: overlay.y });
        }),
      );

      // Size changed (resize)
      unlisteners.push(
        await listen<RegionRect>("ocr-region-size-changed", (event) => {
          const r = event.payload;
          regionRef.current = { x: r.x, y: r.y, width: r.width, height: r.height };
          // Re-run OCR with new size if we have previous results
          if (ocrSource !== null) {
            void captureAndTranslate({ x: r.x, y: r.y, width: r.width, height: r.height });
          }
        }),
      );

      // Manual refresh
      unlisteners.push(
        await listen("ocr-region-refresh", () => {
          if (regionRef.current) {
            void captureAndTranslate(regionRef.current);
          }
        }),
      );

      // Continuous toggle
      unlisteners.push(
        await listen<{ enabled: boolean }>("ocr-region-continuous", (event) => {
          setContinuous(event.payload.enabled);
        }),
      );

      // Region frame closed
      unlisteners.push(
        await listen("ocr-region-close", async () => {
          setContinuous(false);
          setStatus("idle");
          setError(null);
          setOcrSource(null);
          setTranslation(null);
          regionRef.current = null;
          try {
            await invoke("close_overlay");
          } catch {
            // overlay may not exist
          }
          await getCurrentWindow().show();
        }),
      );
    };

    void setup();
    return () => {
      unlisteners.forEach((fn) => fn());
    };
  }, [captureAndTranslate, ocrSource]);

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
    let unlisten: (() => void) | undefined;

    listen<ScreenshotRegion>("ocr-screenshot-selected", async (event) => {
      const sel = event.payload;
      // Convert image-pixel coordinates to screen coordinates
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
      try {
        const selector = await WebviewWindow.getByLabel("ocr-screenshot");
        if (selector) await selector.close();
      } catch {
        // may already be closed
      }

      // Show main window (but we'll show the region frame on top)
      const appWindow = getCurrentWindow();
      await appWindow.show();

      // Create region frame window at the selection position
      try {
        await invoke("create_ocr_region_frame", {
          x: screenX,
          y: screenY,
          width: screenW,
          height: screenH,
        });
      } catch (err) {
        setError(String(err));
        setStatus("error");
        return;
      }

      // Run first OCR + translate
      await captureAndTranslate(region);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, [captureAndTranslate]);

  // ---- Cancelled listener ----
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen("ocr-screenshot-cancelled", () => {
      void getCurrentWindow().show();
      setStatus("idle");
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  // ---- Start screenshot translate ----
  const startScreenshotTranslate = async () => {
    setStatus("capturing");
    setError(null);
    setOcrSource(null);
    setTranslation(null);
    setContinuous(false);
    regionRef.current = null;

    try {
      // Close existing region frame and overlay
      try {
        await invoke("close_ocr_region_frame");
        await invoke("close_overlay");
      } catch {
        // may not exist
      }

      const appWindow = getCurrentWindow();
      await appWindow.hide();
      await new Promise((resolve) => window.setTimeout(resolve, 180));

      const info = await prepareScreenshotSnapshot();
      setSnapshotInfo(info);
      snapshotInfoRef.current = info;

      // Close any existing selector
      const existing = await WebviewWindow.getByLabel("ocr-screenshot");
      if (existing) {
        await existing.close();
      }

      new WebviewWindow("ocr-screenshot", {
        url: "/?window=ocr-screenshot",
        title: "OCR Screenshot",
        fullscreen: true,
        decorations: false,
        alwaysOnTop: true,
        skipTaskbar: true,
        resizable: false,
        focus: true,
      });

      setStatus("selecting");
    } catch (err) {
      await getCurrentWindow().show();
      setError(String(err));
      setStatus("error");
    }
  };

  const busy = status === "capturing" || status === "selecting" || status === "running";

  return (
    <section className="rounded-2xl border border-border bg-bg-secondary p-5 shadow-sm">
      <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
        <div>
          <div className="flex items-center gap-2 text-lg font-semibold text-text-primary">
            <ScanLine size={20} />
            屏幕 OCR 翻译
          </div>
          <p className="mt-2 max-w-2xl text-sm leading-6 text-text-secondary">
            全屏截图选区后，在屏幕原位显示可拖拽/可调整的 OCR 区域框，翻译结果以浮窗显示在旁边。
            支持手动刷新和持续刷新（2 秒间隔）。优先使用 Windows 原生 OCR，失败时回退到 tesseract.js。
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
        </div>
      </div>

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
