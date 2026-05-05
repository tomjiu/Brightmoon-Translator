import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Copy, Pin, RefreshCw, ScanLine, X } from "lucide-react";
import {
  captureScreenshotRegion,
  cropScreenshotSnapshot,
  ocrImagePreferNative,
  prepareScreenshotSnapshot,
  type ScreenshotRegion,
} from "../services/ocr";
import { useConfigStore } from "../stores/configStore";
import type { TranslateResponse, TranslationResult } from "../types";

type OcrStatus = "idle" | "capturing" | "selecting" | "recognizing" | "refreshing" | "done" | "error";

interface OcrResultState {
  region: ScreenshotRegion;
  image: string;
  sourceText: string;
  translations: TranslationResult[];
  updatedAt: number;
}

function primaryText(results: TranslationResult[]) {
  return results[0]?.text ?? "";
}

export default function OcrScreenshotTranslator() {
  const config = useConfigStore((state) => state.config);
  const [status, setStatus] = useState<OcrStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<OcrResultState | null>(null);
  const [pinned, setPinned] = useState(false);

  const runOcr = useCallback(async (region: ScreenshotRegion, freshCapture: boolean) => {
    setStatus(freshCapture ? "refreshing" : "recognizing");
    setError(null);

    try {
      const image = freshCapture
        ? await captureScreenshotRegion(region)
        : await cropScreenshotSnapshot(region);
      const sourceText = await ocrImagePreferNative(image, "auto");
      if (!sourceText.trim()) {
        throw new Error("OCR 没有识别到文本");
      }

      const response = await invoke<TranslateResponse>("translate", {
        request: {
          text: sourceText.trim(),
          from: config.defaultFrom,
          to: config.defaultTo,
        },
      });

      setResult({
        region,
        image,
        sourceText: sourceText.trim(),
        translations: response.results,
        updatedAt: Date.now(),
      });
      await getCurrentWindow().show();
      await getCurrentWindow().setFocus();
      setStatus("done");
    } catch (err) {
      await getCurrentWindow().show();
      await getCurrentWindow().setFocus();
      setError(String(err));
      setStatus("error");
    }
  }, [config.defaultFrom, config.defaultTo]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<ScreenshotRegion>("ocr-screenshot-selected", (event) => {
      void runOcr(event.payload, false);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, [runOcr]);

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

  const startScreenshotTranslate = async () => {
    setStatus("capturing");
    setError(null);

    try {
      const appWindow = getCurrentWindow();
      await appWindow.hide();
      await new Promise((resolve) => window.setTimeout(resolve, 180));
      await prepareScreenshotSnapshot();
      const existing = await WebviewWindow.getByLabel("ocr-screenshot");
      if (existing) {
        await existing.close();
      }

      const selector = new WebviewWindow("ocr-screenshot", {
        url: "/?window=ocr-screenshot",
        title: "OCR Screenshot",
        fullscreen: true,
        decorations: false,
        alwaysOnTop: true,
        skipTaskbar: true,
        resizable: false,
        focus: true,
      });

      void selector.once("tauri://error", (event) => {
        setError(String(event.payload));
        setStatus("error");
      });
      setStatus("selecting");
    } catch (err) {
      await getCurrentWindow().show();
      setError(String(err));
      setStatus("error");
    }
  };

  const copyText = async (text: string) => {
    await navigator.clipboard.writeText(text);
  };

  const togglePin = async () => {
    const next = !pinned;
    await getCurrentWindow().setAlwaysOnTop(next);
    setPinned(next);
  };

  const busy = status === "capturing" || status === "selecting" || status === "recognizing" || status === "refreshing";

  return (
    <section className="rounded-2xl border border-border bg-bg-secondary p-5 shadow-sm">
      <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
        <div>
          <div className="flex items-center gap-2 text-lg font-semibold text-text-primary">
            <ScanLine size={20} />
            截图翻译 MVP
          </div>
          <p className="mt-2 max-w-2xl text-sm leading-6 text-text-secondary">
            默认走全屏截图选区：先截屏，再拖选区域，优先调用 Windows 原生 OCR，失败时回退到 tesseract.js。
            旧版连续 OCR 监控保留在下方，暂不作为主流程。
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
            className={`rounded-xl border px-3 py-2 text-sm transition-colors ${
              pinned
                ? "border-primary bg-primary/10 text-primary"
                : "border-border text-text-secondary hover:text-text-primary"
            }`}
            onClick={togglePin}
            title="置顶主窗口"
          >
            <Pin size={16} />
          </button>
        </div>
      </div>

      <div className="mt-4 rounded-xl bg-bg-tertiary px-3 py-2 text-sm text-text-secondary">
        当前状态：
        <span className="ml-1 text-text-primary">
          {status === "idle" && "等待截图"}
          {status === "capturing" && "正在捕获屏幕"}
          {status === "selecting" && "等待选区"}
          {status === "recognizing" && "正在 OCR 与翻译"}
          {status === "refreshing" && "正在刷新同一区域"}
          {status === "done" && "已完成"}
          {status === "error" && "出错"}
        </span>
      </div>

      {error && (
        <div className="mt-4 rounded-xl border border-red-500/40 bg-red-500/10 p-3 text-sm text-red-500">
          {error}
        </div>
      )}

      {result && (
        <div className="mt-5 grid gap-4 xl:grid-cols-[320px,1fr]">
          <div className="overflow-hidden rounded-xl border border-border bg-bg-primary">
            <img src={result.image} alt="OCR selected region" className="max-h-80 w-full object-contain bg-black/80" />
            <div className="flex items-center justify-between border-t border-border px-3 py-2 text-xs text-text-secondary">
              <span>
                {result.region.width} x {result.region.height}px
              </span>
              <span>{new Date(result.updatedAt).toLocaleTimeString()}</span>
            </div>
          </div>

          <div className="grid gap-3">
            <div className="rounded-xl border border-border bg-bg-primary p-4">
              <div className="mb-2 flex items-center justify-between">
                <h3 className="text-sm font-medium text-text-primary">OCR 原文</h3>
                <button className="text-text-secondary hover:text-text-primary" onClick={() => copyText(result.sourceText)}>
                  <Copy size={15} />
                </button>
              </div>
              <p className="whitespace-pre-wrap text-sm leading-6 text-text-primary">{result.sourceText}</p>
            </div>

            <div className="rounded-xl border border-border bg-bg-primary p-4">
              <div className="mb-2 flex items-center justify-between">
                <h3 className="text-sm font-medium text-text-primary">翻译结果</h3>
                <button className="text-text-secondary hover:text-text-primary" onClick={() => copyText(primaryText(result.translations))}>
                  <Copy size={15} />
                </button>
              </div>
              <p className="whitespace-pre-wrap text-sm leading-6 text-text-primary">
                {primaryText(result.translations) || "没有翻译结果"}
              </p>
            </div>

            <div className="flex flex-wrap gap-2">
              <button
                className="inline-flex items-center gap-2 rounded-xl border border-border px-3 py-2 text-sm text-text-primary hover:bg-bg-tertiary disabled:opacity-60"
                disabled={busy}
                onClick={() => runOcr(result.region, true)}
              >
                <RefreshCw size={15} />
                刷新同一区域
              </button>
              <button
                className="inline-flex items-center gap-2 rounded-xl border border-border px-3 py-2 text-sm text-text-secondary hover:bg-bg-tertiary hover:text-text-primary"
                onClick={() => {
                  setResult(null);
                  setStatus("idle");
                }}
              >
                <X size={15} />
                清空结果
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
