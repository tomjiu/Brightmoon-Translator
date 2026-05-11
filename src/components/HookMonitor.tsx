import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "../i18n";
import { useConfigStore } from "../stores/configStore";
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
}

function HookMonitor() {
  const { t } = useI18n();
  const { config, updateConfig } = useConfigStore();
  const [isRunning, setIsRunning] = useState(false);
  const [results, setResults] = useState<HookTranslatedItem[]>([]);
  const [copiedId, setCopiedId] = useState<number | null>(null);
  const [speakingId, setSpeakingId] = useState<number | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [autoScroll, setAutoScroll] = useState(true);
  const [showOverlay, setShowOverlay] = useState(config.hookShowOverlay ?? true);
  const [autoCopy, setAutoCopy] = useState(config.hookAutoCopy ?? false);
  const listRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const idCounter = useRef(0);

  // Check initial status
  useEffect(() => {
    invoke<boolean>("get_hook_monitor_status").then((running) => {
      setIsRunning(running);
    });
  }, []);

  // Listen for hook-text-translated events
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setup = async () => {
      unlisten = await listen<{
        window_title: string;
        process_name: string;
        original: string;
        translated: string;
        engine: string;
        timestamp: number;
        source: string;
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
          invoke<[number, number, number, number]>("get_foreground_window_rect")
            .then(([wx, wy, ww, _wh]) => {
              // Position overlay at bottom-center of the target window
              const overlayW = Math.min(500, ww - 40);
              const overlayH = 180;
              const ox = wx + (ww - overlayW) / 2;
              const oy = wy + _wh - overlayH - 20;
              invoke("update_overlay", {
                x: Math.round(ox),
                y: Math.round(oy),
                width: Math.round(overlayW),
                height: overlayH,
                text: item.translated,
                source: item.original,
                showControls: false,
              }).catch(() => {});
            })
            .catch(() => {});
        }
      });
    };

    setup();
    return () => {
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
        await invoke("stop_hook_monitor");
        setIsRunning(false);
      } else {
        await invoke("start_hook_monitor");
        setIsRunning(true);
      }
    } catch (err) {
      console.error("Hook monitor toggle failed:", err);
    }
  }, [isRunning]);

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
      const base64Audio = await invoke<string>("text_to_speech", { text, lang });
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
    updateConfig((prev) => ({ ...prev, hookShowOverlay: next }));
  }, [showOverlay, updateConfig]);

  const toggleAutoCopy = useCallback(() => {
    const next = !autoCopy;
    setAutoCopy(next);
    updateConfig((prev) => ({ ...prev, hookAutoCopy: next }));
  }, [autoCopy, updateConfig]);

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  };

  // Filter results by search query
  const filteredResults = searchQuery
    ? results.filter(
        (r) =>
          r.original.toLowerCase().includes(searchQuery.toLowerCase()) ||
          r.translated.toLowerCase().includes(searchQuery.toLowerCase()) ||
          r.windowTitle.toLowerCase().includes(searchQuery.toLowerCase())
      )
    : results;

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
                        : "bg-primary/20 text-primary"
                    }`}>
                      {item.source === "clipboard" ? "CB" : "UIA"}
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
