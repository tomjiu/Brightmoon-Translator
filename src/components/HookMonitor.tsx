import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { safeInvoke, invokeOrThrow } from "../services/invoke";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "../i18n";
import { useConfigStore } from "../stores/configStore";
import {
  showOverlayAt,
  positionBelowText,
  positionAtWindowBottom,
} from "../services/overlayPosition";
import {
  Zap,
  Square,
  Trash2,
  Copy,
  Check,
  Monitor,
  Languages,
  Clock,
  Search,
  Download,
  ArrowDown,
  Volume2,
} from "lucide-react";

interface HookTranslatedItem {
  id: number;
  windowTitle: string;
  processName: string;
  original: string;
  translated: string;
  engine: string;
  timestamp: number;
  source: string;
  textRect?: [number, number, number, number]; // [x, y, w, h] screen coords
}

const formatTime = (timestamp: number) => {
  const date = new Date(timestamp);
  return date.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
};

function HookMonitor() {
  const { t } = useI18n();
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);
  const [isRunning, setIsRunning] = useState(false);
  const [results, setResults] = useState<HookTranslatedItem[]>([]);
  const [copiedId, setCopiedId] = useState<number | null>(null);
  const [speakingId, setSpeakingId] = useState<number | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [autoScroll, setAutoScroll] = useState(true);
  const hookConfig = config.hook;
  const [showOverlay, setShowOverlay] = useState(hookConfig?.showOverlay ?? true);
  const [autoCopy, setAutoCopy] = useState(hookConfig?.autoCopy ?? false);
  const [enabledSources, setEnabledSources] = useState<string[]>(hookConfig?.enabledSources ?? ["uia", "clipboard", "ocr", "hook"]);
  const listRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const idCounter = useRef(0);

  const ALL_SOURCES = [
    { key: "uia", label: "UIA", desc: t("hook.source.uia") },
    { key: "clipboard", label: "CB", desc: t("hook.source.clipboard") },
    { key: "ocr", label: "OCR", desc: t("hook.source.ocr") },
    { key: "hook", label: "HOOK", desc: t("hook.source.hook") },
  ];

  // Check initial status
  useEffect(() => {
    invokeOrThrow<boolean>("get_hook_monitor_status")
      .then((running) => {
        setIsRunning(running);
      })
      .catch(() => {});
  }, []);

  // Listen for hook-text-translated events
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;

    const setup = async () => {
      unlisten = await listen<{
        window_title: string;
        process_name: string;
        original: string;
        translated: string;
        engine: string;
        timestamp: number;
        source: string;
        text_rect?: [number, number, number, number];
      }>("hook-text-translated", (event) => {
        const item: HookTranslatedItem = {
          id: ++idCounter.current,
          windowTitle: event.payload.window_title,
          processName: event.payload.process_name,
          original: event.payload.original,
          translated: event.payload.translated,
          engine: event.payload.engine,
          timestamp: event.payload.timestamp,
          source: event.payload.source || "uia",
          textRect: event.payload.text_rect,
        };

        setResults((prev) => {
          const next = [item, ...prev];
          return next.length > 200 ? next.slice(0, 200) : next;
        });

        // Auto-copy if enabled
        if (autoCopy) {
          navigator.clipboard.writeText(item.translated).catch(() => {});
        }

        // Show in overlay positioned at target window bottom
        if (showOverlay) {
          if (item.textRect) {
            // Use precise text position: overlay below the text element
            const [tx, ty, tw, th] = item.textRect;
            const pos = positionBelowText(tx, ty, tw, th);
            showOverlayAt(pos, item.translated, item.original);
          } else {
            // Fallback: position at bottom of foreground window
            safeInvoke<[number, number, number, number]>("get_foreground_window_rect", undefined, { silent: true })
              .then(([rect]) => {
                if (rect) {
                  const [wx, wy, ww, wh] = rect;
                  const pos = positionAtWindowBottom(wx, wy, ww, wh);
                  showOverlayAt(pos, item.translated, item.original);
                }
              });
          }
        }
      });

      if (cancelled && unlisten) {
        unlisten();
        unlisten = null;
      }
    };

    setup();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [autoCopy, showOverlay]);

  // Auto-scroll to bottom when new results arrive
  useEffect(() => {
    if (autoScroll && bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [results.length, autoScroll]);

  // Detect manual scroll to disable auto-scroll
  const handleScroll = useCallback(() => {
    if (!listRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = listRef.current;
    const isAtBottom = scrollHeight - scrollTop - clientHeight < 50;
    setAutoScroll(isAtBottom);
  }, []);

  const handleToggle = useCallback(async () => {
    try {
      if (isRunning) {
        await invokeOrThrow("stop_hook_monitor");
        setIsRunning(false);
      } else {
        // Ensure config (source selection, intervals) is saved before starting
        await saveConfig();
        await invokeOrThrow("start_hook_monitor");
        setIsRunning(true);
      }
    } catch (err) {
      console.error("Hook monitor toggle failed:", err);
    }
  }, [isRunning, saveConfig]);

  const handleClear = useCallback(() => {
    setResults([]);
  }, []);

  const copyText = useCallback((text: string, id: number) => {
    navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1500);
  }, []);

  const speakText = useCallback(async (text: string, lang: string, id: number) => {
    try {
      setSpeakingId(id);
      const base64Audio = await invokeOrThrow<string>("text_to_speech", { text, lang });
      const audioBytes = Uint8Array.from(atob(base64Audio), c => c.charCodeAt(0));
      const audioBlob = new Blob([audioBytes], { type: "audio/mp3" });
      const audioUrl = URL.createObjectURL(audioBlob);
      const audio = new Audio(audioUrl);
      audio.onended = () => {
        setSpeakingId(null);
        URL.revokeObjectURL(audioUrl);
      };
      audio.onerror = () => {
        setSpeakingId(null);
        URL.revokeObjectURL(audioUrl);
      };
      await audio.play();
    } catch (err) {
      console.error("TTS failed:", err);
      setSpeakingId(null);
    }
  }, []);

  const handleExport = useCallback(() => {
    const lines = results.map(
      (r) =>
        `[${formatTime(r.timestamp)}] (${r.engine}) ${r.windowTitle}\n  ${r.original}\n  => ${r.translated}\n`
    );
    const blob = new Blob([lines.join("\n")], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `hook-translations-${new Date().toISOString().slice(0, 10)}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  }, [results]);

  const toggleOverlay = useCallback(() => {
    const next = !showOverlay;
    setShowOverlay(next);
    updateConfig((prev) => ({
      ...prev,
      hookShowOverlay: next,
      hook: { ...prev.hook, showOverlay: next },
    }));
  }, [showOverlay, updateConfig]);

  const toggleAutoCopy = useCallback(() => {
    const next = !autoCopy;
    setAutoCopy(next);
    updateConfig((prev) => ({
      ...prev,
      hookAutoCopy: next,
      hook: { ...prev.hook, autoCopy: next },
    }));
  }, [autoCopy, updateConfig]);

  const toggleSource = useCallback((source: string) => {
    setEnabledSources((prev) => {
      const next = prev.includes(source)
        ? prev.filter((s) => s !== source)
        : [...prev, source];
      updateConfig((cfg) => ({
        ...cfg,
        hook: { ...cfg.hook, enabledSources: next },
      }));
      // Persist source config since it's read at monitor start
      setTimeout(() => saveConfig(), 0);
      return next;
    });
  }, [updateConfig, saveConfig]);

  // Filter results by search query
  const filteredResults = useMemo(() => searchQuery
    ? results.filter(
        (r) =>
          r.original.toLowerCase().includes(searchQuery.toLowerCase()) ||
          r.translated.toLowerCase().includes(searchQuery.toLowerCase()) ||
          r.windowTitle.toLowerCase().includes(searchQuery.toLowerCase())
      )
    : results,
    [results, searchQuery]
  );

  return (
    <div className="flex flex-col h-full gap-3 p-4">
      {/* Header & Controls */}
      <div className="bg-bg-secondary border border-border rounded-xl p-4">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Zap size={18} className="text-primary" />
            <h3 className="text-sm font-semibold text-text-primary">
              {t("hook.title")}
            </h3>
          </div>
          <div className="flex items-center gap-2">
            {isRunning && (
              <div className="flex items-center gap-1.5">
                <div className="w-2 h-2 rounded-full bg-success animate-pulse" />
                <span className="text-xs text-success">
                  {t("hook.running")}
                </span>
              </div>
            )}
            <span className="text-xs text-text-secondary bg-bg-tertiary px-2 py-0.5 rounded-full">
              {results.length} {t("hook.items")}
            </span>
          </div>
        </div>

        <p className="text-xs text-text-secondary mb-3">
          {t("hook.description")}
        </p>

        {/* Source Toggles */}
        <div className="flex flex-wrap items-center gap-2 mb-3">
          <span className="text-xs text-text-secondary mr-1">{t("hook.sources")}:</span>
          {ALL_SOURCES.map((src) => (
            <label
              key={src.key}
              className={`flex items-center gap-1.5 cursor-pointer px-2 py-1 rounded-md text-xs transition-colors ${
                enabledSources.includes(src.key)
                  ? "bg-primary/15 text-primary border border-primary/30"
                  : "bg-bg-tertiary text-text-secondary border border-transparent opacity-60"
              }`}
              title={src.desc}
            >
              <input
                type="checkbox"
                checked={enabledSources.includes(src.key)}
                onChange={() => toggleSource(src.key)}
                className="accent-primary w-3 h-3"
                disabled={isRunning}
              />
              {src.label}
            </label>
          ))}
          {isRunning && (
            <span className="text-[10px] text-text-secondary/60 ml-1">{t("hook.sourcesStopToChange")}</span>
          )}
        </div>

        {/* Options */}
        <div className="flex items-center gap-3 mb-3">
          <label className="flex items-center gap-1.5 cursor-pointer">
            <input
              type="checkbox"
              checked={showOverlay}
              onChange={toggleOverlay}
              className="accent-primary w-3.5 h-3.5"
            />
            <span className="text-xs text-text-secondary">{t("hook.showOverlay")}</span>
          </label>
          <label className="flex items-center gap-1.5 cursor-pointer">
            <input
              type="checkbox"
              checked={autoCopy}
              onChange={toggleAutoCopy}
              className="accent-primary w-3.5 h-3.5"
            />
            <span className="text-xs text-text-secondary">{t("hook.autoCopy")}</span>
          </label>
        </div>

        {/* Action Buttons */}
        <div className="flex gap-2">
          <button
            className={`flex-1 flex items-center justify-center gap-2 px-4 py-2 rounded-lg text-sm font-semibold transition-colors ${
              isRunning
                ? "bg-error text-white hover:bg-error/90"
                : "bg-primary text-white hover:bg-primary-hover"
            }`}
            onClick={handleToggle}
          >
            {isRunning ? (
              <>
                <Square size={14} />
                {t("hook.stop")}
              </>
            ) : (
              <>
                <Zap size={14} />
                {t("hook.start")}
              </>
            )}
          </button>

          <button
            className="flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium bg-bg-tertiary text-text-secondary hover:text-text-primary hover:bg-bg-tertiary/80 transition-colors disabled:opacity-50"
            onClick={handleExport}
            disabled={results.length === 0}
            title={t("hook.export")}
          >
            <Download size={13} />
          </button>

          <button
            className="flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium bg-bg-tertiary text-text-secondary hover:text-text-primary hover:bg-bg-tertiary/80 transition-colors disabled:opacity-50"
            onClick={handleClear}
            disabled={results.length === 0}
          >
            <Trash2 size={13} />
            {t("hook.clear")}
          </button>
        </div>
      </div>

      {/* Search Bar */}
      {results.length > 3 && (
        <div className="relative">
          <Search
            size={14}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary"
          />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={t("hook.search")}
            className="w-full bg-bg-secondary border border-border rounded-lg pl-9 pr-4 py-2 text-xs text-text-primary outline-none focus:border-primary placeholder:text-text-secondary"
          />
        </div>
      )}

      {/* Results List */}
      <div
        ref={listRef}
        className="flex-1 overflow-y-auto space-y-2 min-h-0"
        onScroll={handleScroll}
      >
        {filteredResults.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-text-secondary">
            <Monitor size={48} className="mb-4 opacity-30" />
            <p className="text-sm">
              {searchQuery ? t("hook.noMatch") : t("hook.empty")}
            </p>
            {!searchQuery && (
              <p className="text-xs mt-1">{t("hook.emptyHint")}</p>
            )}
          </div>
        ) : (
          <>
            {filteredResults.map((item) => (
              <div
                key={item.id}
                className="bg-bg-secondary border border-border rounded-lg p-3 group hover:border-primary/30 transition-colors"
              >
                {/* Header: window + engine + time */}
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-1.5 min-w-0">
                    <Monitor
                      size={12}
                      className="text-text-secondary shrink-0"
                    />
                    <span className="text-xs text-text-secondary truncate max-w-[200px]">
                      {item.processName || item.windowTitle}
                    </span>
                    {item.windowTitle && item.processName && (
                      <span className="text-[10px] text-text-secondary/60 truncate max-w-[120px]">
                        — {item.windowTitle}
                      </span>
                    )}
                  </div>
                  <div className="flex items-center gap-1.5 shrink-0">
                    <span className={`text-[9px] px-1 py-0.5 rounded font-medium ${
                      item.source === "clipboard"
                        ? "bg-accent/20 text-accent"
                        : item.source === "ocr"
                        ? "bg-warning/20 text-warning"
                        : item.source === "hook"
                        ? "bg-success/20 text-success"
                        : "bg-primary/20 text-primary"
                    }`}>
                      {item.source === "clipboard" ? "CB" : item.source === "ocr" ? "OCR" : item.source === "hook" ? "HOOK" : "UIA"}
                    </span>
                    <Languages size={10} className="text-primary" />
                    <span className="text-[10px] text-primary font-medium">
                      {item.engine}
                    </span>
                    <Clock size={10} className="text-text-secondary ml-1" />
                    <span className="text-[10px] text-text-secondary">
                      {formatTime(item.timestamp)}
                    </span>
                  </div>
                </div>

                {/* Original text */}
                <div className="text-xs text-text-secondary mb-1.5 line-clamp-2 leading-relaxed select-text">
                  {item.original}
                </div>

                {/* Translated text */}
                <div className="flex items-start justify-between gap-2">
                  <div className="text-sm text-text-primary leading-relaxed flex-1 select-text">
                    {item.translated}
                  </div>
                  <div className="flex items-center gap-1 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
                    <button
                      className="p-1 rounded hover:bg-bg-tertiary text-text-secondary"
                      onClick={() => speakText(item.translated, "auto", item.id)}
                      title={t("hook.speak")}
                    >
                      <Volume2 size={14} className={speakingId === item.id ? "text-primary animate-pulse" : ""} />
                    </button>
                    <button
                      className="p-1 rounded hover:bg-bg-tertiary text-text-secondary"
                      onClick={() => copyText(item.translated, item.id)}
                      title={t("hook.copy")}
                    >
                      {copiedId === item.id ? (
                        <Check size={14} className="text-success" />
                      ) : (
                        <Copy size={14} />
                      )}
                    </button>
                  </div>
                </div>
              </div>
            ))}
            <div ref={bottomRef} />
          </>
        )}
      </div>

      {/* Scroll to bottom indicator */}
      {!autoScroll && results.length > 0 && (
        <button
          className="absolute bottom-20 right-6 bg-primary text-white rounded-full p-2 shadow-lg hover:bg-primary-hover transition-colors"
          onClick={() => {
            setAutoScroll(true);
            bottomRef.current?.scrollIntoView({ behavior: "smooth" });
          }}
        >
          <ArrowDown size={16} />
        </button>
      )}
    </div>
  );
}

export default HookMonitor;
