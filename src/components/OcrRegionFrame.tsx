import { useEffect, useRef, useState, useCallback } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PhysicalSize } from "@tauri-apps/api/dpi";
import {
  RefreshCw,
  Play,
  Pause,
  X,
  Copy,
  Image,
  Languages,
} from "lucide-react";
import type { OcrLineResult } from "../services/ocr";

interface OcrRegionData {
  screenshot: string;
  sourceText: string;
  translatedText: string;
  ocrLines: OcrLineResult[];
  lineTranslations: string[];
  sourceLang: string;
  targetLang: string;
}

type DisplayMode = "translation" | "source";

const LANGUAGE_MAP: Record<string, string> = {
  auto: "自动检测",
  "zh-CN": "中文",
  "en": "英语",
  "ja": "日语",
  "ko": "韩语",
  "fr": "法语",
  "de": "德语",
  "ru": "俄语",
  "es": "西班牙语",
};

const SUPPORTED_LANGS = ["auto", "zh-CN", "en", "ja", "ko", "fr", "de", "ru", "es"];

export default function OcrRegionFrame() {
  const win = getCurrentWindow();
  const [data, setData] = useState<OcrRegionData | null>(null);
  const [continuous, setContinuous] = useState(false);
  const [displayMode, setDisplayMode] = useState<DisplayMode>("translation");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const resizeStart = useRef({ x: 0, y: 0, width: 0, height: 0 });
  const [sourceLang, setSourceLang] = useState("auto");
  const [targetLang, setTargetLang] = useState("en");
  const sourceLangRef = useRef(sourceLang);
  const targetLangRef = useRef(targetLang);
  useEffect(() => { sourceLangRef.current = sourceLang; }, [sourceLang]);
  useEffect(() => { targetLangRef.current = targetLang; }, [targetLang]);

  // Solid dark background (no transparency to prevent ghost frames on move/resize)
  useEffect(() => {
    document.body.style.backgroundColor = "#0a0a0a";
    const root = document.getElementById("root");
    if (root) root.style.backgroundColor = "#0a0a0a";
  }, []);

  // Listen for data updates from main window
  // Uses `cancelled` flag to handle React StrictMode double-mount
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    // Timeout: show error if no data received within 15 seconds
    // OCR + translation pipeline can take several seconds on first run
    const timeout = window.setTimeout(() => {
      if (cancelled) return;
      if (!data) {
        setError("等待数据超时，请点击重试");
        setLoading(false);
      }
    }, 15000);

    listen<OcrRegionData>("ocr-region-update-data", (event) => {
      if (cancelled) return;
      const d = event.payload;
      setData(d);
      setLoading(false);
      setError(null);
      setSourceLang(d.sourceLang);
      setTargetLang(d.targetLang);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      window.clearTimeout(timeout);
      unlisten?.();
    };
  }, []);

  // Emit initial position
  useEffect(() => {
    const timer = setTimeout(async () => {
      try {
        const pos = await win.outerPosition();
        const size = await win.outerSize();
        await emit("ocr-region-position-changed", {
          x: pos.x,
          y: pos.y,
          width: size.width,
          height: size.height,
        });
      } catch {
        // window may already be closed
      }
    }, 200);
    return () => clearTimeout(timer);
  }, []);

  // ---- Native OS drag (replaces manual setPosition to prevent ghost windows) ----
  const onMouseDown = async (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button") || (e.target as HTMLElement).closest("select")) return;
    e.preventDefault();
    try {
      await win.startDragging();
      // After drag completes, notify the main window of new position
      const pos = await win.outerPosition();
      const size = await win.outerSize();
      await emit("ocr-region-position-changed", { x: pos.x, y: pos.y, width: size.width, height: size.height });
    } catch { /* ignore */ }
  };

  // ---- Resize from corner handle ----
  const resizing = useRef(false);
  const onResizeStart = async (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    resizing.current = true;
    const size = await win.outerSize();
    resizeStart.current = { x: e.screenX, y: e.screenY, width: size.width, height: size.height };
    const onMove = (ev: MouseEvent) => {
      if (!resizing.current) return;
      const dx = ev.screenX - resizeStart.current.x;
      const dy = ev.screenY - resizeStart.current.y;
      void win.setSize(new PhysicalSize(Math.max(80, resizeStart.current.width + dx), Math.max(60, resizeStart.current.height + dy)));
    };
    const onUp = async () => {
      resizing.current = false;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      try {
        const pos = await win.outerPosition();
        const size = await win.outerSize();
        await emit("ocr-region-size-changed", { x: pos.x, y: pos.y, width: size.width, height: size.height });
      } catch { /* ignore */ }
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  };

  // ---- Button handlers ----
  const handleRefresh = useCallback(() => {
    void emit("ocr-region-refresh", null);
  }, []);

  const handleToggleContinuous = useCallback(() => {
    const next = !continuous;
    setContinuous(next);
    void emit("ocr-region-continuous", { enabled: next });
  }, [continuous]);

  const handleClose = useCallback(() => {
    // Just emit the close event — the main window will close this window
    // via the Rust `close_ocr_region_frame` command, then show itself.
    // This avoids both windows being visible at the same time.
    void emit("ocr-region-close", null);
  }, []);

  const copyToClipboard = useCallback(async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // fallback
    }
  }, []);

  const handleCopyScreenshot = useCallback(async () => {
    if (!data?.screenshot) return;
    try {
      const blob = await (await fetch(data.screenshot)).blob();
      await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
    } catch {
      // fallback — copy as text
      await copyToClipboard(data.sourceText);
    }
  }, [data, copyToClipboard]);

  const handleLangChange = useCallback((type: "source" | "target", value: string) => {
    if (type === "source") {
      setSourceLang(value);
      sourceLangRef.current = value;
    } else {
      setTargetLang(value);
      targetLangRef.current = value;
    }
    void emit("ocr-region-lang-change", {
      sourceLang: type === "source" ? value : sourceLangRef.current,
      targetLang: type === "target" ? value : targetLangRef.current,
    });
  }, []);

  // ---- Compute text area bounds from OCR lines ----
  const textAreaBounds = useCallback(() => {
    if (!data?.ocrLines?.length) return null;
    const validLines = data.ocrLines.filter((l) => l.width > 0 && l.height > 0);
    if (!validLines.length) return null;
    let minX = Infinity, minY = Infinity, maxR = -Infinity, maxB = -Infinity;
    for (const l of validLines) {
      if (l.x < minX) minX = l.x;
      if (l.y < minY) minY = l.y;
      if (l.x + l.width > maxR) maxR = l.x + l.width;
      if (l.y + l.height > maxB) maxB = l.y + l.height;
    }
    return { x: minX, y: minY, width: maxR - minX, height: maxB - minY };
  }, [data]);

  const bounds = textAreaBounds();

  return (
    <div
      className="fixed inset-0 select-none"
      onMouseDown={onMouseDown}
    >
      {/* Loading state */}
      {loading && !error && (
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
          <div className="text-gray-400 text-sm animate-pulse">
            正在识别文本...
          </div>
        </div>
      )}

      {/* Error state */}
      {error && (
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 pointer-events-auto">
          <div className="text-red-400 text-sm">
            {error}
          </div>
          <button
            className="px-3 py-1.5 bg-sky-500/20 text-sky-300 rounded text-xs hover:bg-sky-500/30 transition-colors"
            onClick={() => {
              setError(null);
              setLoading(true);
              handleRefresh();
            }}
          >
            重试
          </button>
        </div>
      )}

      {/* Captured screenshot as dimmed background */}
      {data?.screenshot && (
        <img
          src={data.screenshot}
          className="absolute inset-0 w-full h-full pointer-events-none select-none"
          style={{ opacity: 0.12 }}
          alt=""
          draggable={false}
        />
      )}

      {/* Translation overlay at source text position — immersive line-by-line replacement */}
      {data && displayMode === "translation" && data.ocrLines.length > 0 && (
        <div className="absolute inset-0 pointer-events-none">
          {data.ocrLines.map((line, i) =>
            line.width > 0 && line.height > 0 ? (
              <div
                key={i}
                className="absolute group"
                style={{
                  left: line.x - 2,
                  top: line.y - 1,
                  minWidth: line.width + 4,
                }}
              >
                {/* Background that matches original text area */}
                <div
                  className="absolute inset-0 bg-black/60 backdrop-blur-[2px] rounded-sm"
                  style={{ minWidth: line.width + 4 }}
                />
                {/* Translation text overlay with better styling */}
                <div
                  className="relative text-xs leading-normal text-white font-medium drop-shadow-[0_1px_2px_rgba(0,0,0,0.95)] whitespace-nowrap px-1.5 py-0.5"
                  style={{
                    minWidth: line.width,
                    fontSize: `${Math.max(10, Math.min(14, line.height * 0.6))}px`,
                    lineHeight: `${line.height}px`,
                  }}
                >
                  {data.lineTranslations?.[i] || line.text}
                </div>
              </div>
            ) : null,
          )}
        </div>
      )}

      {/* Fallback: show full translation if no line translations available */}
      {data && displayMode === "translation" && data.translatedText && (!data.lineTranslations || data.lineTranslations.length === 0) && bounds && (
        <div
          className="absolute pointer-events-none"
          style={{
            left: bounds.x - 4,
            top: bounds.y - 2,
            maxWidth: Math.min(bounds.width + 8, window.innerWidth - bounds.x - 8),
          }}
        >
          <div className="bg-black/60 backdrop-blur-[2px] rounded px-2 py-1">
            <div className="text-xs leading-normal text-white font-medium drop-shadow-[0_1px_2px_rgba(0,0,0,0.95)]">
              {data.translatedText}
            </div>
          </div>
        </div>
      )}

      {/* Source text overlay */}
      {data && displayMode === "source" && data.ocrLines.length > 0 && (
        <div className="absolute inset-0 pointer-events-none">
          {data.ocrLines.map((line, i) =>
            line.width > 0 && line.height > 0 ? (
              <div
                key={i}
                className="absolute bg-gray-900/60 rounded px-1 text-xs leading-tight text-gray-300 whitespace-nowrap"
                style={{ left: line.x, top: line.y - 2, maxWidth: line.width + 4 }}
              >
                {line.text}
              </div>
            ) : null,
          )}
        </div>
      )}

      {/* ---- Bottom Control Bar ---- */}
      <div className="absolute bottom-0 left-0 right-0 pointer-events-none">
        <div className="bg-gray-900/85 backdrop-blur-sm border-t border-sky-400/30 px-2 py-1.5 flex items-center gap-1.5 text-xs pointer-events-auto flex-wrap">
          {/* Display mode toggle */}
          <button
            className={`px-2 py-0.5 rounded font-medium transition-colors ${
              displayMode === "translation"
                ? "bg-sky-500/30 text-sky-300"
                : "text-gray-400 hover:text-gray-200 hover:bg-white/10"
            }`}
            onClick={() => setDisplayMode(displayMode === "translation" ? "source" : "translation")}
            title="切换原文/译文显示"
          >
            {displayMode === "translation" ? "译文" : "原文"}
          </button>

          <span className="w-px h-4 bg-gray-700" />

          {/* Copy source */}
          <button
            className="flex items-center gap-1 px-1.5 py-0.5 rounded text-gray-400 hover:text-gray-200 hover:bg-white/10 transition-colors"
            onClick={() => data && copyToClipboard(data.sourceText)}
            title="复制原文"
          >
            <Copy size={11} />
            原文
          </button>

          {/* Copy translation */}
          <button
            className="flex items-center gap-1 px-1.5 py-0.5 rounded text-gray-400 hover:text-gray-200 hover:bg-white/10 transition-colors"
            onClick={() => data && copyToClipboard(data.translatedText)}
            title="复制译文"
          >
            <Copy size={11} />
            译文
          </button>

          {/* Copy screenshot */}
          <button
            className="flex items-center gap-1 px-1.5 py-0.5 rounded text-gray-400 hover:text-gray-200 hover:bg-white/10 transition-colors"
            onClick={handleCopyScreenshot}
            title="复制截图"
          >
            <Image size={11} />
            截图
          </button>

          <span className="w-px h-4 bg-gray-700" />

          {/* Language selectors */}
          <Languages size={11} className="text-gray-500" />
          <select
            className="bg-gray-800 text-gray-300 rounded border border-gray-700 px-1 py-0.5 text-[11px] cursor-pointer"
            value={sourceLang}
            onChange={(e) => handleLangChange("source", e.target.value)}
          >
            {SUPPORTED_LANGS.map((l) => (
              <option key={l} value={l}>
                {LANGUAGE_MAP[l] || l}
              </option>
            ))}
          </select>
          <span className="text-gray-500 text-[10px]">→</span>
          <select
            className="bg-gray-800 text-gray-300 rounded border border-gray-700 px-1 py-0.5 text-[11px] cursor-pointer"
            value={targetLang}
            onChange={(e) => handleLangChange("target", e.target.value)}
          >
            {SUPPORTED_LANGS.filter((l) => l !== "auto").map((l) => (
              <option key={l} value={l}>
                {LANGUAGE_MAP[l] || l}
              </option>
            ))}
          </select>

          <span className="w-px h-4 bg-gray-700" />

          {/* Auto refresh toggle */}
          <button
            className={`flex items-center gap-1 px-1.5 py-0.5 rounded transition-colors ${
              continuous
                ? "text-sky-400 bg-sky-400/15"
                : "text-gray-400 hover:text-gray-200 hover:bg-white/10"
            }`}
            onClick={handleToggleContinuous}
            title={continuous ? "暂停自动刷新" : "开启自动刷新"}
          >
            {continuous ? <Pause size={11} /> : <Play size={11} />}
            自动
          </button>

          {/* Manual refresh */}
          <button
            className="flex items-center justify-center w-5 h-5 rounded text-gray-400 hover:text-gray-200 hover:bg-white/10 transition-colors"
            onClick={handleRefresh}
            title="立即刷新"
          >
            <RefreshCw size={11} />
          </button>

          {/* Close */}
          <button
            className="flex items-center justify-center w-5 h-5 rounded text-gray-400 hover:text-red-400 hover:bg-red-400/15 transition-colors ml-auto"
            onClick={handleClose}
            title="关闭"
          >
            <X size={12} />
          </button>
        </div>
      </div>

      {/* Bottom-right resize handle */}
      <div
        className="absolute bottom-0 right-0 w-4 h-4 cursor-se-resize pointer-events-auto z-10"
        onMouseDown={onResizeStart}
      >
        <svg
          className="absolute bottom-0.5 right-0.5 text-sky-400/60"
          width="12"
          height="12"
          viewBox="0 0 12 12"
        >
          <path d="M11 1L1 11M11 5L5 11M11 9L9 11" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      </div>
    </div>
  );
}
