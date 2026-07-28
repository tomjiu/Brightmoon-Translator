import { useState, useEffect, useRef, useCallback, useMemo, memo } from 'react';
import { safeInvoke, invokeOrThrow } from '../services/invoke';
import { speakText as ttsSpeak } from '../services/tts';
import { listen } from '@tauri-apps/api/event';
import { useI18n } from '../i18n';
import { useConfigStore } from '../stores/configStore';
import { isTauriRuntime } from '../services/tauriRuntime';
import {
  showOverlayAt,
  positionBelowText,
  positionAtWindowBottom,
} from '../services/overlayPosition';
import ProcessPicker from './ProcessPicker';
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
  ChevronDown,
  ChevronUp,
  Syringe,
} from 'lucide-react';

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

interface HookStatus {
  injected: boolean;
  pid: number;
  processName: string;
  messagesRead: number;
}

interface CapturedText {
  text: string;
  codePage: number;
  x: number;
  y: number;
  timestamp: number;
}

const formatTime = (timestamp: number, browserLocale: string) => {
  const date = new Date(timestamp);
  return date.toLocaleTimeString(browserLocale, {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
};

interface HookResultItemProps {
  item: HookTranslatedItem;
  speakingId: number | null;
  copiedId: number | null;
  onSpeak: (text: string, lang: string, id: number) => void;
  onCopy: (text: string, id: number) => void;
  t: (key: string) => string;
  browserLocale: string;
}

const HookResultItem = memo(function HookResultItem({
  item,
  speakingId,
  copiedId,
  onSpeak,
  onCopy,
  t,
  browserLocale,
}: HookResultItemProps) {
  return (
    <div className="bg-bg-secondary border border-border rounded-lg p-3 group hover:border-primary/30 transition-colors">
      {/* Header: window + engine + time */}
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-1.5 min-w-0">
          <Monitor size={12} className="text-text-secondary shrink-0" />
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
          <span
            className={`text-[9px] px-1 py-0.5 rounded font-medium ${
              item.source === 'clipboard'
                ? 'bg-accent/20 text-accent'
                : item.source === 'ocr'
                  ? 'bg-warning/20 text-warning'
                  : item.source === 'hook'
                    ? 'bg-success/20 text-success'
                    : 'bg-primary/20 text-primary'
            }`}
          >
            {item.source === 'clipboard'
              ? 'CB'
              : item.source === 'ocr'
                ? 'OCR'
                : item.source === 'hook'
                  ? 'HOOK'
                  : 'UIA'}
          </span>
          <Languages size={10} className="text-primary" />
          <span className="text-[10px] text-primary font-medium">{item.engine}</span>
          <Clock size={10} className="text-text-secondary ml-1" />
          <span className="text-[10px] text-text-secondary">
            {formatTime(item.timestamp, browserLocale)}
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
            onClick={() => onSpeak(item.translated, 'auto', item.id)}
            title={t('hook.speak')}
          >
            <Volume2
              size={14}
              className={speakingId === item.id ? 'text-primary animate-pulse' : ''}
            />
          </button>
          <button
            className="p-1 rounded hover:bg-bg-tertiary text-text-secondary"
            onClick={() => onCopy(item.translated, item.id)}
            title={t('hook.copy')}
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
  );
});

function HookMonitor() {
  const { t, locale } = useI18n();
  const isTauri = isTauriRuntime();

  const localeMap: Record<string, string> = {
    zh: 'zh-CN',
    en: 'en-US',
    ja: 'ja-JP',
    ko: 'ko-KR',
  };
  const browserLocale = localeMap[locale] || locale;
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);
  const [isRunning, setIsRunning] = useState(false);
  const [results, setResults] = useState<HookTranslatedItem[]>([]);
  const [copiedId, setCopiedId] = useState<number | null>(null);
  const [speakingId, setSpeakingId] = useState<number | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [autoScroll, setAutoScroll] = useState(true);
  const hookConfig = config.hook;
  const [showOverlay, setShowOverlay] = useState(hookConfig?.showOverlay === true);
  const [autoCopy, setAutoCopy] = useState(hookConfig?.autoCopy ?? false);
  // Default sources: UIA + clipboard (product path).
  const [enabledSources, setEnabledSources] = useState<string[]>(
    hookConfig?.enabledSources?.length ? hookConfig.enabledSources : ['uia', 'clipboard'],
  );
  const listRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const idCounter = useRef(0);

  const ALL_SOURCES = useMemo(
    () => [
      { key: 'uia', label: 'UIA', desc: t('hook.source.uia') },
      { key: 'clipboard', label: 'CB', desc: t('hook.source.clipboard') },
      { key: 'ocr', label: 'OCR', desc: t('hook.source.ocr') },
      { key: 'hook', label: 'HOOK', desc: t('hook.source.hook') },
    ],
    [t],
  );

  // H-Code DLL Injection state
  const [hcodeExpanded, setHcodeExpanded] = useState(false);
  const [hcodePid, setHcodePid] = useState('');
  const [hcodeStatus, setHcodeStatus] = useState<HookStatus | null>(null);
  const [hcodeLoading, setHcodeLoading] = useState(false);
  const [hcodeMessages, setHcodeMessages] = useState<CapturedText[]>([]);
  const hcodePollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const [showProcessPicker, setShowProcessPicker] = useState(false);
  const [dllAvailable, setDllAvailable] = useState<boolean | null>(null);
  const [dllPath, setDllPath] = useState<string | null>(null);

  // Check initial status
  useEffect(() => {
    if (!isTauri) return;

    invokeOrThrow<boolean>('get_hook_monitor_status')
      .then((running) => {
        setIsRunning(running);
      })
      .catch((e: unknown) => {
        console.error('Failed to get hook monitor status:', e);
      });
  }, [isTauri]);

  // Listen for hook-text-translated events
  useEffect(() => {
    if (!isTauri) return;

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
      }>('hook-text-translated', (event) => {
        const item: HookTranslatedItem = {
          id: ++idCounter.current,
          windowTitle: event.payload.window_title,
          processName: event.payload.process_name,
          original: event.payload.original,
          translated: event.payload.translated,
          engine: event.payload.engine,
          timestamp: event.payload.timestamp,
          source: event.payload.source || 'uia',
          textRect: event.payload.text_rect,
        };

        setResults((prev) => {
          const next = [item, ...prev];
          return next.length > 200 ? next.slice(0, 200) : next;
        });

        // Auto-copy if enabled
        if (autoCopy) {
          navigator.clipboard.writeText(item.translated).catch((e: unknown) => {
            console.warn('Auto-copy failed:', e);
          });
        }

        // Auto-play TTS if enabled
        if (config.ttsAutoPlay && item.translated) {
          ttsSpeak(item.translated, 'auto').catch((e: unknown) => {
            console.warn('TTS auto-play failed:', e);
          });
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
            safeInvoke<[number, number, number, number]>('get_foreground_window_rect', undefined, {
              silent: true,
            }).then(([rect]) => {
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
  }, [autoCopy, isTauri, showOverlay]);

  // Auto-scroll to bottom when new results arrive
  useEffect(() => {
    if (autoScroll && bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [results.length, autoScroll]);

  // H-Code: DLL preflight + initial injection status
  useEffect(() => {
    if (!isTauri) return;

    void safeInvoke<boolean>('hook_dll_available', undefined, { silent: true }).then(([ok]) => {
      setDllAvailable(ok === true);
    });
    void safeInvoke<string | null>('hook_dll_path', undefined, { silent: true }).then(([p]) => {
      setDllPath(typeof p === 'string' ? p : null);
    });

    safeInvoke<HookStatus>('hook_status', undefined, { silent: true })
      .then(([status]) => {
        if (status) setHcodeStatus(status);
      })
      .catch((e: unknown) => {
        console.error('Failed to get H-Code injection status:', e);
      });
  }, [isTauri]);

  // H-Code: Poll for messages when injected
  useEffect(() => {
    if (hcodeStatus?.injected) {
      hcodePollRef.current = setInterval(async () => {
        try {
          const [msgs] = await safeInvoke<CapturedText[]>('hook_read_messages', undefined, {
            silent: true,
          });
          if (msgs && msgs.length > 0) {
            setHcodeMessages((prev) => {
              const next = [...prev, ...msgs];
              return next.length > 500 ? next.slice(-500) : next;
            });
            // Update status
            const [status] = await safeInvoke<HookStatus>('hook_status', undefined, {
              silent: true,
            });
            if (status) setHcodeStatus(status);
          }
        } catch (e) {
          console.error('H-Code message polling error:', e);
        }
      }, 200);
    }
    return () => {
      if (hcodePollRef.current) {
        clearInterval(hcodePollRef.current);
        hcodePollRef.current = null;
      }
    };
  }, [hcodeStatus?.injected]);

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
        await invokeOrThrow('stop_hook_monitor');
        setIsRunning(false);
      } else {
        // Ensure config (source selection, intervals) is saved before starting
        await saveConfig();
        await invokeOrThrow('start_hook_monitor');
        setIsRunning(true);
      }
    } catch (err) {
      console.error('Hook monitor toggle failed:', err);
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
      await ttsSpeak(text, lang);
    } catch (err) {
      console.error('TTS failed:', err);
    } finally {
      setSpeakingId(null);
    }
  }, []);

  const handleExport = useCallback(() => {
    const lines = results.map(
      (r) =>
        `[${formatTime(r.timestamp, browserLocale)}] (${r.engine}) ${r.windowTitle}\n  ${r.original}\n  => ${r.translated}\n`,
    );
    const blob = new Blob([lines.join('\n')], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `hook-translations-${new Date().toISOString().slice(0, 10)}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  }, [results, browserLocale]);

  // H-Code: Inject DLL
  const handleHcodeInject = useCallback(async () => {
    const pid = parseInt(hcodePid, 10);
    if (isNaN(pid) || pid <= 0) return;
    setHcodeLoading(true);
    try {
      await invokeOrThrow('hook_inject', { pid });
      const status = await invokeOrThrow<HookStatus>('hook_status');
      setHcodeStatus(status);
      setHcodeMessages([]);
    } catch (err) {
      console.error('H-Code inject failed:', err);
    } finally {
      setHcodeLoading(false);
    }
  }, [hcodePid]);

  // H-Code: Eject DLL
  const handleHcodeEject = useCallback(async () => {
    setHcodeLoading(true);
    try {
      await invokeOrThrow('hook_eject');
      setHcodeStatus(null);
      setHcodeMessages([]);
    } catch (err) {
      console.error('H-Code eject failed:', err);
    } finally {
      setHcodeLoading(false);
    }
  }, []);

  const toggleOverlay = useCallback(() => {
    const next = !showOverlay;
    setShowOverlay(next);
    updateConfig((prev) => ({
      ...prev,
      hook: { ...prev.hook, showOverlay: next },
    }));
  }, [showOverlay, updateConfig]);

  const toggleAutoCopy = useCallback(() => {
    const next = !autoCopy;
    setAutoCopy(next);
    updateConfig((prev) => ({
      ...prev,
      hook: { ...prev.hook, autoCopy: next },
    }));
  }, [autoCopy, updateConfig]);

  const toggleSource = useCallback(
    (source: string) => {
      setEnabledSources((prev) => {
        const next = prev.includes(source) ? prev.filter((s) => s !== source) : [...prev, source];
        updateConfig((cfg) => ({
          ...cfg,
          hook: { ...cfg.hook, enabledSources: next },
        }));
        // Persist source config since it's read at monitor start
        setTimeout(() => saveConfig(), 0);
        return next;
      });
    },
    [updateConfig, saveConfig],
  );

  // Filter results by search query
  const filteredResults = useMemo(
    () =>
      searchQuery
        ? results.filter(
            (r) =>
              r.original.toLowerCase().includes(searchQuery.toLowerCase()) ||
              r.translated.toLowerCase().includes(searchQuery.toLowerCase()) ||
              r.windowTitle.toLowerCase().includes(searchQuery.toLowerCase()),
          )
        : results,
    [results, searchQuery],
  );

  return (
    <div className="flex flex-col h-full gap-3 p-4">
      {/* Header & Controls */}
      <div className="bg-bg-secondary border border-border rounded-xl p-4">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Zap size={18} className="text-primary" />
            <h3 className="text-sm font-semibold text-text-primary">{t('hook.title')}</h3>
          </div>
          <div className="flex items-center gap-2">
            {isRunning && (
              <div className="flex items-center gap-1.5">
                <div className="w-2 h-2 rounded-full bg-success animate-pulse" />
                <span className="text-xs text-success">{t('hook.running')}</span>
              </div>
            )}
            <span className="text-xs text-text-secondary bg-bg-tertiary px-2 py-0.5 rounded-full">
              {results.length} {t('hook.items')}
            </span>
          </div>
        </div>

        <p className="text-xs text-text-secondary mb-3">{t('hook.description')}</p>

        {/* Source Toggles */}
        <div className="flex flex-wrap items-center gap-2 mb-3">
          <span className="text-xs text-text-secondary mr-1">{t('hook.sources')}:</span>
          {ALL_SOURCES.map((src) => (
            <label
              key={src.key}
              className={`flex items-center gap-1.5 cursor-pointer px-2 py-1 rounded-md text-xs transition-colors ${
                enabledSources.includes(src.key)
                  ? 'bg-primary/15 text-primary border border-primary/30'
                  : 'bg-bg-tertiary text-text-secondary border border-transparent opacity-60'
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
            <span className="text-[10px] text-text-secondary/60 ml-1">
              {t('hook.sourcesStopToChange')}
            </span>
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
            <span className="text-xs text-text-secondary">{t('hook.showOverlay')}</span>
          </label>
          <label className="flex items-center gap-1.5 cursor-pointer">
            <input
              type="checkbox"
              checked={autoCopy}
              onChange={toggleAutoCopy}
              className="accent-primary w-3.5 h-3.5"
            />
            <span className="text-xs text-text-secondary">{t('hook.autoCopy')}</span>
          </label>
        </div>

        {/* H-Code DLL Injection Section */}
        <div className="border border-border rounded-lg overflow-hidden">
          <button
            className="w-full flex items-center justify-between px-3 py-2 bg-bg-tertiary hover:bg-bg-tertiary/80 transition-colors"
            onClick={() => setHcodeExpanded(!hcodeExpanded)}
          >
            <div className="flex items-center gap-2">
              <Syringe size={14} className="text-accent" />
              <span className="text-xs font-medium text-text-primary">{t('hook.hcode.title')}</span>
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-400/15 text-amber-300 border border-amber-400/25">
                {t('hook.hcode.experimental')}
              </span>
              {hcodeStatus?.injected && (
                <span className="text-[10px] px-1.5 py-0.5 rounded bg-success/20 text-success">
                  {t('hook.hcode.injected')}
                </span>
              )}
            </div>
            {hcodeExpanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          </button>

          {hcodeExpanded && (
            <div className="p-3 space-y-3">
              <p className="text-[11px] text-text-secondary">{t('hook.hcode.description')}</p>

              {dllAvailable === false && (
                <div className="text-[11px] text-amber-300 bg-amber-400/10 border border-amber-400/20 rounded px-2 py-1.5">
                  未找到 moon_hook.dll，注入不可用。请编译 hook-dll 或放到 src-tauri/bin/。
                </div>
              )}
              {dllAvailable === true && dllPath && (
                <div className="text-[10px] text-text-secondary truncate" title={dllPath}>
                  DLL: {dllPath}
                </div>
              )}

              {/* PID Input */}
              <div className="flex gap-2">
                <input
                  type="number"
                  value={hcodePid}
                  onChange={(e) => setHcodePid(e.target.value)}
                  placeholder={t('hook.hcode.pidPlaceholder')}
                  className="flex-1 bg-bg-secondary border border-border rounded px-2 py-1.5 text-xs text-text-primary outline-none focus:border-primary"
                  disabled={hcodeStatus?.injected || dllAvailable !== true}
                />
                <button
                  className="flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-medium bg-bg-tertiary text-text-primary hover:bg-bg-secondary border border-border transition-colors disabled:opacity-50"
                  onClick={() => setShowProcessPicker(true)}
                  disabled={hcodeStatus?.injected || dllAvailable !== true}
                  title="选择进程"
                >
                  <Search size={14} />
                  选择
                </button>
                {hcodeStatus?.injected ? (
                  <button
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-medium bg-error text-white hover:bg-error/90 transition-colors disabled:opacity-50"
                    onClick={handleHcodeEject}
                    disabled={hcodeLoading}
                  >
                    {t('hook.hcode.eject')}
                  </button>
                ) : (
                  <button
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-medium bg-accent text-white hover:bg-accent/90 transition-colors disabled:opacity-50"
                    onClick={handleHcodeInject}
                    disabled={hcodeLoading || !hcodePid || dllAvailable !== true}
                    title={dllAvailable !== true ? '等待 DLL 检测或缺少 moon_hook.dll' : undefined}
                  >
                    {hcodeLoading ? t('hook.hcode.injecting') : t('hook.hcode.inject')}
                  </button>
                )}
              </div>

              {/* Status */}
              {hcodeStatus && (
                <div className="flex items-center gap-4 text-[11px]">
                  <div className="flex items-center gap-1.5">
                    <span className="text-text-secondary">{t('hook.hcode.status')}:</span>
                    <span className={hcodeStatus.injected ? 'text-success' : 'text-text-secondary'}>
                      {hcodeStatus.injected
                        ? t('hook.hcode.injected')
                        : t('hook.hcode.notInjected')}
                    </span>
                  </div>
                  {hcodeStatus.processName && (
                    <div className="flex items-center gap-1.5">
                      <span className="text-text-secondary">{t('hook.hcode.processName')}:</span>
                      <span className="text-text-primary">{hcodeStatus.processName}</span>
                    </div>
                  )}
                  <div className="flex items-center gap-1.5">
                    <span className="text-text-secondary">{t('hook.hcode.messagesRead')}:</span>
                    <span className="text-text-primary">{hcodeStatus.messagesRead}</span>
                  </div>
                </div>
              )}

              {/* Captured Messages */}
              {hcodeMessages.length > 0 && (
                <div className="max-h-40 overflow-y-auto bg-bg-primary border border-border rounded p-2 space-y-1">
                  {hcodeMessages.slice(-20).map((msg, i) => (
                    <div key={i} className="text-[11px] text-text-primary font-mono truncate">
                      {msg.text}
                    </div>
                  ))}
                  {hcodeMessages.length > 20 && (
                    <div className="text-[10px] text-text-secondary text-center">
                      ... {hcodeMessages.length} {t('hook.items')}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}
        </div>

        {/* Action Buttons */}
        <div className="flex gap-2">
          <button
            className={`flex-1 flex items-center justify-center gap-2 px-4 py-2 rounded-lg text-sm font-semibold transition-colors ${
              isRunning
                ? 'bg-error text-white hover:bg-error/90'
                : 'bg-primary text-primary-fg hover:bg-primary-hover'
            }`}
            onClick={handleToggle}
          >
            {isRunning ? (
              <>
                <Square size={14} />
                {t('hook.stop')}
              </>
            ) : (
              <>
                <Zap size={14} />
                {t('hook.start')}
              </>
            )}
          </button>

          <button
            className="flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium bg-bg-tertiary text-text-secondary hover:text-text-primary hover:bg-bg-tertiary/80 transition-colors disabled:opacity-50"
            onClick={handleExport}
            disabled={results.length === 0}
            title={t('hook.export')}
          >
            <Download size={13} />
          </button>

          <button
            className="flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium bg-bg-tertiary text-text-secondary hover:text-text-primary hover:bg-bg-tertiary/80 transition-colors disabled:opacity-50"
            onClick={handleClear}
            disabled={results.length === 0}
          >
            <Trash2 size={13} />
            {t('hook.clear')}
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
            placeholder={t('hook.search')}
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
            <p className="text-sm">{searchQuery ? t('hook.noMatch') : t('hook.empty')}</p>
            {!searchQuery && <p className="text-xs mt-1">{t('hook.emptyHint')}</p>}
          </div>
        ) : (
          <>
            {filteredResults.map((item) => (
              <HookResultItem
                key={item.id}
                item={item}
                speakingId={speakingId}
                copiedId={copiedId}
                onSpeak={speakText}
                onCopy={copyText}
                t={t}
                browserLocale={browserLocale}
              />
            ))}
            <div ref={bottomRef} />
          </>
        )}
      </div>

      {/* Scroll to bottom indicator */}
      {!autoScroll && results.length > 0 && (
        <button
          className="absolute bottom-20 right-6 bg-primary text-primary-fg rounded-full p-2 shadow-lg hover:bg-primary-hover transition-colors"
          onClick={() => {
            setAutoScroll(true);
            bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
          }}
        >
          <ArrowDown size={16} />
        </button>
      )}

      {/* Process Picker Dialog */}
      <ProcessPicker
        isOpen={showProcessPicker}
        onClose={() => setShowProcessPicker(false)}
        onSelect={(pid) => setHcodePid(pid.toString())}
      />
    </div>
  );
}

export default HookMonitor;
