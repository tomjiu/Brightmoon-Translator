import { useEffect, useRef, useCallback, useState } from 'react';
import { speakText as ttsSpeak } from '../services/tts';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { useTranslateStore } from '../stores/translateStore';
import { useConfigStore } from '../stores/configStore';
import { useI18n } from '../i18n';
import { isTauriRuntime } from '../services/tauriRuntime';
import { safeInvoke } from '../services/invoke';
import { LANGUAGES } from '../types';
import { useSpeechRecognition } from '../hooks/useSpeechRecognition';
import { normalizeTranslatorInput } from '../services/translatorText';
import { RubyText } from '../components/RubyText';
import {
  ArrowLeftRight,
  Copy,
  Check,
  X,
  Scan,
  Volume2,
  Clipboard,
  Eye,
  EyeOff,
  Book,
  Repeat,
  Layers,
  Eraser,
  ChevronLeft,
  ChevronRight,
  Columns,
  AlignLeft,
  BookOpen,
  Mic,
  MicOff,
  Sparkles,
  Bookmark,
  Pin,
} from 'lucide-react';
import { saveAndCollect, summarizeReport } from '../hooks/useCollectionPush';

interface MainTranslatorProps {
  onOcrScreenshot: () => void;
}

function MainTranslator({ onOcrScreenshot }: MainTranslatorProps) {
  // Split high-frequency streaming state from stable state to reduce re-renders
  const streamingText = useTranslateStore((s) => s.streamingText);
  const isStreaming = useTranslateStore((s) => s.isStreaming);
  const loading = useTranslateStore((s) => s.loading);
  const sourceText = useTranslateStore((s) => s.sourceText);
  const results = useTranslateStore((s) => s.results);
  const {
    dictionaryResults,
    backTranslation,
    fromLang,
    toLang,
    detectedLang,
    error,
    incrementalMode,
    incrementalEntries,
    translationHistory,
    historyIndex,
    setSourceText,
    setFromLang,
    setToLang,
    swapLanguages,
    translate,
    translateStream,
    lookupDictionary,
    backTranslate,
    polishTranslation,
    polishing,
    detectLanguage,
    clear,
    clearIncremental,
    removeIncrementalEntry,
    toggleIncrementalMode,
    toggleClipboardMonitor,
    clipboardMonitorEnabled,
    goToPreviousTranslation,
    goToNextTranslation,
    moveWindowToCursor,
    embeddedLines,
    embeddedMode,
    translateEmbedded,
    toggleEmbeddedMode,
  } = useTranslateStore();

  const config = useConfigStore((s) => s.config);
  const { t, locale } = useI18n();
  const isTauri = isTauriRuntime();

  const localeMap: Record<string, string> = {
    zh: 'zh-CN',
    en: 'en-US',
    ja: 'ja-JP',
    ko: 'ko-KR',
  };
  const browserLocale = localeMap[locale] || locale;

  // Speech recognition
  const {
    isListening,
    interimTranscript,
    error: speechError,
    startListening,
    stopListening,
    isSupported: isSpeechSupported,
    consumeTranscript,
  } = useSpeechRecognition();

  // Periodically consume speech transcript and append to source text
  useEffect(() => {
    if (!isListening) return;

    const timer = setInterval(() => {
      const text = consumeTranscript();
      if (text) {
        const currentText = useTranslateStore.getState().sourceText;
        setSourceText(currentText ? `${currentText} ${text}` : text);
      }
    }, 300);

    return () => clearInterval(timer);
  }, [isListening, consumeTranscript, setSourceText]);

  const debounceTimer = useRef<ReturnType<typeof setTimeout>>();
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);
  const [speakingIndex, setSpeakingIndex] = useState<number | null>(null);
  const [maskRevealed, setMaskRevealed] = useState(false);
  const [deleteNewlines, setDeleteNewlines] = useState(false);
  const [bilingualMode, setBilingualMode] = useState(false);
  const [collectedIndex, setCollectedIndex] = useState<number | null>(null);
  const [collectHint, setCollectHint] = useState<string | null>(null);
  const [activeResultIndex, setActiveResultIndex] = useState(0);

  const handleCollect = useCallback(
    async (text: string, index: number) => {
      const word = sourceText.trim();
      if (!word) return;
      try {
        const { report } = await saveAndCollect({
          word,
          translation: text,
          fromLang: fromLang === 'auto' ? detectedLang || 'en' : fromLang,
          toLang,
        });
        setCollectedIndex(index);
        setCollectHint(summarizeReport(report));
        window.setTimeout(() => setCollectHint(null), 4000);
      } catch (err) {
        setCollectHint(err instanceof Error ? err.message : String(err));
      }
    },
    [sourceText, fromLang, toLang, detectedLang],
  );

  const handleInput = useCallback(
    (value: string) => {
      const normalizedValue = normalizeTranslatorInput(value, deleteNewlines);
      setSourceText(normalizedValue);
      setMaskRevealed(false);
      if (debounceTimer.current) {
        clearTimeout(debounceTimer.current);
      }
      debounceTimer.current = setTimeout(() => {
        if (normalizedValue.trim()) {
          detectLanguage(normalizedValue);
          // Auto-enable embedded mode for multi-line text
          const isMultiLine = normalizedValue.includes('\n');
          const store = useTranslateStore.getState();
          if (store.embeddedMode || isMultiLine) {
            translateEmbedded();
          } else {
            translate();
          }
          lookupDictionary();
        }
      }, 500);
    },
    [
      deleteNewlines,
      setSourceText,
      translate,
      translateEmbedded,
      lookupDictionary,
      detectLanguage,
    ],
  );

  const copyResult = (text: string, index: number) => {
    navigator.clipboard.writeText(text);
    setCopiedIndex(index);
    setTimeout(() => setCopiedIndex(null), 1500);
  };

  // O1-O4: Pin translation card to screen as a persistent always-on-top window.
  // The backend PinWindowManager maintains a retain pool of reusable webviews
  // and stacks multiple pins with a +24/+24 cascade offset.
  const [pinnedIndex, setPinnedIndex] = useState<number | null>(null);
  const pinResult = async (text: string, index: number) => {
    if (!isTauri) return;
    // Place the pin near the current window's bottom-right corner so it
    // doesn't cover the result panel. The backend applies stacked cascade.
    let x = 100;
    let y = 100;
    try {
      const win = getCurrentWebviewWindow();
      const pos = await win.outerPosition();
      const size = await win.outerSize();
      x = pos.x + size.width + 8;
      y = pos.y + 40;
    } catch {
      // Fallback to default position if window info is unavailable.
    }
    const [label, err] = await safeInvoke<string>(
      'pin_translation_card',
      {
        source: sourceText,
        translated: text,
        x,
        y,
        width: 360,
        height: 160,
        sourceApp: null,
        windowTitle: null,
      },
      { silent: true },
    );
    if (err || !label) {
      console.warn('[Pin] failed to pin translation:', err);
      return;
    }
    setPinnedIndex(index);
    setTimeout(() => setPinnedIndex(null), 1500);
  };

  const speakText = async (text: string, lang: string, index: number) => {
    try {
      setSpeakingIndex(index);
      await ttsSpeak(text, lang);
    } catch (err) {
      console.error('TTS failed:', err);
    } finally {
      setSpeakingIndex(null);
    }
  };

  useEffect(() => {
    return () => {
      if (debounceTimer.current) {
        clearTimeout(debounceTimer.current);
      }
    };
  }, []);

  // Keyboard shortcuts for history navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.altKey && e.key === 'ArrowLeft') {
        e.preventDefault();
        goToPreviousTranslation();
      } else if (e.altKey && e.key === 'ArrowRight') {
        e.preventDefault();
        goToNextTranslation();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [goToPreviousTranslation, goToNextTranslation]);

  // Window follow mode: move window to cursor on selection translation
  useEffect(() => {
    if (!isTauri) return;

    let unlisten: (() => void) | undefined;

    const setup = async () => {
      unlisten = await listen('trigger-translate-selection', () => {
        if (config.windowFollowMode === 'cursor') {
          moveWindowToCursor();
        }
      });
    };
    setup();

    return () => {
      if (unlisten) unlisten();
    };
  }, [config.windowFollowMode, isTauri, moveWindowToCursor]);

  // Auto-play TTS after translation completes
  useEffect(() => {
    if (config.ttsAutoPlay && results.length > 0 && !loading) {
      const lastResult = results[results.length - 1];
      if (lastResult.text) {
        ttsSpeak(lastResult.text, toLang).catch((e: unknown) => {
          console.warn('TTS auto-play failed:', e);
        });
      }
    }
  }, [results, loading, config.ttsAutoPlay, toLang]);

  // Reset active engine tab when a new translation arrives
  useEffect(() => {
    setActiveResultIndex(0);
    setMaskRevealed(false);
  }, [results]);

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleString(browserLocale, {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  };

  return (
    <div className="flex flex-col h-full">
      {/* Chrome bar — same surface as Documents / Vocabulary tab strips */}
      <div className="ui-chrome flex flex-wrap items-center justify-between gap-2 px-4 py-2.5 border-b border-border">
        <div className="flex min-w-0 flex-1 basis-[280px] items-center gap-1.5">
          <select
            value={fromLang}
            onChange={(e) => setFromLang(e.target.value)}
            className="h-8 min-w-0 w-[clamp(96px,22vw,132px)] bg-bg-tertiary text-text-primary border border-border rounded-lg px-2 py-1 text-xs cursor-pointer"
          >
            {LANGUAGES.map((l) => (
              <option key={l.code} value={l.code}>
                {l.name}
              </option>
            ))}
          </select>

          {detectedLang && fromLang === 'auto' && (
            <span
              className="max-w-[112px] truncate text-xs text-text-secondary"
              title={`${t('translator.detected')}: ${detectedLang}`}
            >
              {t('translator.detected')}: {detectedLang}
            </span>
          )}

          <button
            className="h-8 w-8 shrink-0 bg-bg-tertiary border border-border text-text-primary rounded-lg hover:bg-primary hover:text-primary-fg transition-colors duration-150 flex items-center justify-center"
            onClick={swapLanguages}
            title={t('translator.swapLang')}
          >
            <ArrowLeftRight size={16} />
          </button>

          <select
            value={toLang}
            onChange={(e) => setToLang(e.target.value)}
            className="h-8 min-w-0 w-[clamp(96px,22vw,132px)] bg-bg-tertiary text-text-primary border border-border rounded-lg px-2 py-1 text-xs cursor-pointer"
          >
            {LANGUAGES.filter((l) => l.code !== 'auto').map((l) => (
              <option key={l.code} value={l.code}>
                {l.name}
              </option>
            ))}
          </select>
        </div>

        <div className="flex shrink-0 items-center gap-1.5">
          <button
            className="h-8 w-8 shrink-0 bg-primary text-primary-fg rounded-lg hover:bg-primary-hover transition-colors duration-150 flex items-center justify-center"
            onClick={onOcrScreenshot}
            title={t('ocr.screenshotTranslate') || 'OCR截图翻译'}
          >
            <Scan size={16} />
          </button>

          <div className="flex items-center gap-0.5 bg-bg-tertiary rounded-lg p-0.5 border border-border">
            <button
              className={`w-7 h-7 rounded-md flex items-center justify-center transition-colors duration-150 ${
                incrementalMode
                  ? 'bg-primary text-primary-fg'
                  : 'text-text-secondary hover:text-text-primary'
              }`}
              onClick={toggleIncrementalMode}
              title={t(
                incrementalMode ? 'translator.incrementalModeOn' : 'translator.incrementalModeOff',
              )}
            >
              <Layers size={14} />
            </button>

            <button
              className={`w-7 h-7 rounded-md flex items-center justify-center transition-colors duration-150 ${
                deleteNewlines
                  ? 'bg-primary text-primary-fg'
                  : 'text-text-secondary hover:text-text-primary'
              }`}
              onClick={() => setDeleteNewlines(!deleteNewlines)}
              title={t(deleteNewlines ? 'translator.keepNewlines' : 'translator.deleteNewlines')}
            >
              <Eraser size={14} />
            </button>

            <button
              className={`w-7 h-7 rounded-md flex items-center justify-center transition-colors duration-150 ${
                clipboardMonitorEnabled
                  ? 'bg-primary text-primary-fg'
                  : 'text-text-secondary hover:text-text-primary'
              }`}
              onClick={toggleClipboardMonitor}
              title={t(
                clipboardMonitorEnabled
                  ? 'translator.stopClipboardMonitor'
                  : 'translator.startClipboardMonitor',
              )}
            >
              <Clipboard size={14} />
            </button>

            <button
              className={`w-7 h-7 rounded-md flex items-center justify-center transition-colors duration-150 ${
                bilingualMode
                  ? 'bg-primary text-primary-fg'
                  : 'text-text-secondary hover:text-text-primary'
              }`}
              onClick={() => setBilingualMode(!bilingualMode)}
              title={t(bilingualMode ? 'translator.bilingualOff' : 'translator.bilingualOn')}
            >
              <Columns size={14} />
            </button>

            <button
              className={`w-7 h-7 rounded-md flex items-center justify-center transition-colors duration-150 ${
                embeddedMode
                  ? 'bg-primary text-primary-fg'
                  : 'text-text-secondary hover:text-text-primary'
              }`}
              onClick={() => {
                toggleEmbeddedMode();
              }}
              title={t(embeddedMode ? 'translator.embeddedOff' : 'translator.embeddedOn')}
            >
              <BookOpen size={14} />
            </button>
          </div>
        </div>
      </div>

      {/* Translation Area */}
      <div className="flex gap-3 flex-1 min-h-0 p-3">
        {/* Source Panel */}
        <div className="flex-1 flex flex-col bg-bg-secondary border border-border rounded-xl overflow-hidden shadow-sm">
          <div className="flex-1 relative">
            <textarea
              value={sourceText}
              onChange={(e) => handleInput(e.target.value)}
              placeholder={t('translator.placeholder')}
              className="w-full h-full bg-transparent text-text-primary p-3 text-sm leading-relaxed resize-none outline-none placeholder:text-text-secondary"
              autoFocus
            />
            {isListening && interimTranscript && (
              <div className="absolute bottom-2 left-4 right-4 text-xs text-primary bg-primary/10 rounded-lg px-3 py-1.5">
                {interimTranscript}...
              </div>
            )}
          </div>
          <div className="flex justify-between items-center px-3 py-1.5 border-t border-border">
            <div className="flex items-center gap-2">
              <span className="text-xs text-text-secondary">
                {sourceText.length} {t('translator.chars')}
                {incrementalMode && (
                  <span className="ml-2 text-accent">{t('translator.incrementalMode')}</span>
                )}
              </span>
              {speechError && <span className="text-xs text-error">{speechError}</span>}
            </div>
            <div className="flex items-center gap-2">
              {/* Speech Recognition Button */}
              {isSpeechSupported && (
                <button
                  className={`p-1.5 rounded-lg transition-colors flex items-center gap-1 ${
                    isListening
                      ? 'bg-primary text-primary-fg animate-pulse'
                      : 'text-text-secondary hover:text-text-primary hover:bg-bg-tertiary'
                  }`}
                  onClick={() => {
                    if (isListening) {
                      // Consume any remaining transcript before stopping
                      const remaining = consumeTranscript();
                      if (remaining) {
                        const currentText = useTranslateStore.getState().sourceText;
                        setSourceText(currentText ? `${currentText} ${remaining}` : remaining);
                      }
                      stopListening();
                      // Trigger translation after stopping
                      setTimeout(() => {
                        const text = useTranslateStore.getState().sourceText;
                        if (text.trim()) {
                          translateStream();
                        }
                      }, 100);
                    } else {
                      startListening(fromLang);
                    }
                  }}
                  title={
                    isListening ? t('translator.stopListening') : t('translator.startListening')
                  }
                >
                  {isListening ? <MicOff size={14} /> : <Mic size={14} />}
                </button>
              )}
              {sourceText && (
                <button
                  className="text-xs text-text-secondary hover:text-error transition-colors flex items-center gap-1"
                  onClick={() => {
                    clear();
                    if (isListening) stopListening();
                  }}
                >
                  <X size={14} />
                  {t('translator.clear')}
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Result Panel */}
        <div className="flex-1 flex flex-col bg-bg-secondary border border-border rounded-xl overflow-hidden shadow-sm">
          <div className="flex-1 overflow-y-auto">
            {/* Incremental Entries */}
            {incrementalMode && incrementalEntries.length > 0 && (
              <div className="p-2">
                <div className="flex items-center justify-between mb-2 px-2">
                  <span className="text-xs text-accent font-semibold">
                    {t('translator.appendRecords')} ({incrementalEntries.length})
                  </span>
                  <button
                    className="text-xs text-text-secondary hover:text-error transition-colors flex items-center gap-1"
                    onClick={clearIncremental}
                  >
                    <X size={12} />
                    {t('translator.emptyAppendRecords')}
                  </button>
                </div>
                {incrementalEntries.map((entry) => (
                  <div
                    key={entry.id}
                    className="bg-bg-tertiary/50 rounded-lg p-3 mb-2 group relative"
                  >
                    <button
                      className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity p-1 rounded-md hover:bg-error/20 text-text-secondary hover:text-error"
                      onClick={() => removeIncrementalEntry(entry.id)}
                    >
                      <X size={12} />
                    </button>
                    <div className="text-xs text-text-secondary mb-1">
                      {entry.sourceText.slice(0, 50)}
                      {entry.sourceText.length > 50 ? '...' : ''}
                    </div>
                    <div className="text-sm text-primary">{entry.results[0]?.text || ''}</div>
                    <div className="text-xs text-text-secondary mt-1">
                      {formatTime(entry.timestamp)}
                    </div>
                  </div>
                ))}
              </div>
            )}

            {/* Embedded Translation Mode */}
            {embeddedMode && (
              <div className="p-4">
                {loading ? (
                  <div className="flex items-center justify-center py-8">
                    <div className="animate-pulse text-sm text-text-secondary">
                      {t('translator.translating')}
                    </div>
                  </div>
                ) : embeddedLines.length > 0 ? (
                  <>
                    <div className="flex items-center justify-between mb-3">
                      <div className="flex items-center gap-2">
                        <BookOpen size={14} className="text-primary" />
                        <span className="text-xs text-primary font-semibold uppercase">
                          {t('translator.embeddedTitle')}
                        </span>
                        <span className="text-xs text-text-secondary">
                          {embeddedLines.length} {t('translator.chars')}
                        </span>
                      </div>
                      <button
                        className="flex items-center gap-1 px-2 py-1 text-xs text-text-secondary hover:text-text-primary hover:bg-bg-tertiary rounded transition-colors"
                        onClick={() => {
                          const text = embeddedLines
                            .map((l) => `${l.lineNumber}. ${l.original}\n   ${l.translated}`)
                            .join('\n');
                          navigator.clipboard.writeText(text);
                        }}
                        title={t('translator.copy')}
                      >
                        <Copy size={12} />
                        {t('translator.copy')}
                      </button>
                    </div>
                    <div className="space-y-2 max-h-[60vh] overflow-y-auto">
                      {embeddedLines.map((line) => (
                        <div
                          key={line.lineNumber}
                          className="group flex gap-3 py-2 px-3 rounded-lg border-l-2 border-primary/30 hover:bg-bg-tertiary/50 transition-colors"
                        >
                          <span className="text-xs text-text-secondary font-mono w-6 shrink-0 text-right pt-0.5">
                            {line.lineNumber}
                          </span>
                          <div className="flex-1 min-w-0">
                            <p className="text-sm text-text-secondary leading-relaxed select-text">
                              {line.original}
                            </p>
                            <p className="text-sm text-text-primary leading-relaxed select-text mt-1">
                              {line.translated}
                            </p>
                          </div>
                          <button
                            className="opacity-0 group-hover:opacity-100 shrink-0 p-1 text-text-secondary hover:text-primary transition-all"
                            onClick={() =>
                              navigator.clipboard.writeText(`${line.original}\n${line.translated}`)
                            }
                            title={t('translator.copy')}
                          >
                            <Copy size={12} />
                          </button>
                        </div>
                      ))}
                    </div>
                  </>
                ) : (
                  <div className="flex flex-col items-center justify-center py-8 text-text-secondary">
                    <BookOpen size={32} className="mb-3 opacity-30" />
                    <p className="text-sm">{t('translator.embeddedEmpty')}</p>
                    <p className="text-xs mt-1">{t('translator.embeddedEmptyHint')}</p>
                  </div>
                )}
              </div>
            )}

            {/* Current Results */}
            {!embeddedMode &&
              (loading || isStreaming ? (
                isStreaming && streamingText ? (
                  <div className="p-4">
                    <div className="flex justify-between items-center mb-2">
                      <span className="text-xs text-primary font-semibold uppercase">
                        {t('translator.streaming')}
                      </span>
                    </div>
                    <div className="text-sm leading-relaxed text-text-primary select-text">
                      {streamingText}
                      <span className="animate-pulse text-primary">|</span>
                    </div>
                  </div>
                ) : (
                  <div className="flex items-center justify-center h-full text-text-secondary">
                    <div className="animate-pulse">{t('translator.translating')}</div>
                  </div>
                )
              ) : error ? (
                <div className="p-4 text-error text-sm">{error}</div>
              ) : results.length > 0 ? (
                (() => {
                  const safeIndex = Math.min(activeResultIndex, results.length - 1);
                  const r = results[safeIndex];
                  return (
                    <div className="p-4">
                      {/* Bilingual: show source text above translation */}
                      {bilingualMode && (
                        <div className="mb-3 pb-2 border-b border-border/50">
                          <div className="flex items-center gap-1.5 mb-1">
                            <AlignLeft size={10} className="text-text-secondary" />
                            <span className="text-xs text-text-secondary">
                              {t('translator.sourceText')}
                            </span>
                          </div>
                          <p className="text-sm text-text-secondary leading-relaxed select-text">
                            {sourceText}
                          </p>
                        </div>
                      )}
                      <div className="flex items-center justify-between gap-2 mb-2">
                        {/* Engine switcher */}
                        {results.length > 1 ? (
                          <div className="flex items-center gap-0.5 bg-bg-tertiary rounded-lg p-0.5 border border-border">
                            {results.map((res, i) => (
                              <button
                                key={i}
                                onClick={() => setActiveResultIndex(i)}
                                className={`px-2 py-1 text-xs font-medium rounded-md transition-colors ${
                                  i === safeIndex
                                    ? 'bg-primary text-primary-fg'
                                    : 'text-text-secondary hover:text-text-primary'
                                }`}
                              >
                                {res.engine}
                              </button>
                            ))}
                          </div>
                        ) : (
                          <span className="text-xs text-primary font-semibold uppercase">
                            {r.engine}
                          </span>
                        )}

                        <div className="flex items-center gap-0.5">
                          {config.translationMask && (
                            <button
                              className={`h-7 w-7 rounded-md flex items-center justify-center transition-colors ${
                                maskRevealed
                                  ? 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
                                  : 'text-warning bg-warning/15 hover:bg-warning/20'
                              }`}
                              onClick={() => setMaskRevealed(!maskRevealed)}
                              title={t(
                                maskRevealed ? 'translator.hideOriginal' : 'translator.showOriginal',
                              )}
                            >
                              {maskRevealed ? <EyeOff size={13} /> : <Eye size={13} />}
                            </button>
                          )}
                          <button
                            className={`h-7 w-7 rounded-md flex items-center justify-center transition-colors ${
                              speakingIndex === safeIndex
                                ? 'text-primary bg-primary/15'
                                : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
                            }`}
                            onClick={() => speakText(r.text, toLang, safeIndex)}
                            title={t('translator.speak')}
                          >
                            <Volume2 size={13} />
                          </button>
                          <button
                            className={`h-7 w-7 rounded-md flex items-center justify-center transition-colors ${
                              collectedIndex === safeIndex
                                ? 'text-primary bg-primary/15'
                                : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
                            }`}
                            onClick={() => void handleCollect(r.text, safeIndex)}
                            title={t('translator.collectToWordbook')}
                          >
                            <Bookmark size={13} />
                          </button>
                          <button
                            className={`h-7 w-7 rounded-md flex items-center justify-center transition-colors ${
                              copiedIndex === safeIndex
                                ? 'text-primary bg-primary/15'
                                : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
                            }`}
                            onClick={() => copyResult(r.text, safeIndex)}
                            title={t('translator.copy')}
                          >
                            {copiedIndex === safeIndex ? <Check size={13} /> : <Copy size={13} />}
                          </button>
                          {isTauri && (
                            <button
                              className={`h-7 w-7 rounded-md flex items-center justify-center transition-colors ${
                                pinnedIndex === safeIndex
                                  ? 'text-primary bg-primary/15'
                                  : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
                              }`}
                              onClick={() => void pinResult(r.text, safeIndex)}
                              title={t('translator.pin')}
                            >
                              <Pin size={13} />
                            </button>
                          )}
                          <button
                            className="h-7 w-7 rounded-md flex items-center justify-center transition-colors text-text-secondary hover:bg-bg-tertiary hover:text-text-primary"
                            onClick={() => backTranslate(r.text)}
                            title={t('translator.backTranslate')}
                          >
                            <Repeat size={13} />
                          </button>
                          <button
                            className="h-7 w-7 rounded-md flex items-center justify-center transition-colors text-text-secondary hover:bg-bg-tertiary hover:text-text-primary disabled:opacity-50"
                            onClick={() => polishTranslation(safeIndex)}
                            disabled={polishing}
                            title={t('translator.polish')}
                          >
                            <Sparkles size={13} />
                          </button>
                        </div>
                      </div>
                      {config.translationMask && !maskRevealed ? (
                        <div
                          className="text-sm leading-relaxed text-text-primary select-text cursor-pointer bg-bg-tertiary/50 rounded-lg p-3 text-center hover:bg-bg-tertiary transition-colors"
                          onClick={() => setMaskRevealed(true)}
                        >
                          <Eye size={16} className="inline mr-2 text-text-secondary" />
                          <span className="text-text-secondary">{t('translator.clickToShow')}</span>
                        </div>
                      ) : (
                        <div className="text-sm leading-relaxed text-text-primary select-text">
                          <RubyText
                            text={r.text}
                            enabled={config.furiganaEnabled && toLang === 'ja'}
                          />
                        </div>
                      )}
                      {collectHint && collectedIndex === safeIndex && (
                        <p className="mt-2 text-xs text-text-secondary whitespace-pre-wrap">
                          {collectHint}
                        </p>
                      )}
                    </div>
                  );
                })()
              ) : (
                <div className="flex items-center justify-center h-full text-text-secondary text-sm">
                  {incrementalMode && incrementalEntries.length > 0
                    ? t('translator.continueInput')
                    : t('translator.resultPlaceholder')}
                </div>
              ))}

            {/* Back Translation Result */}
            {backTranslation && (
              <div className="border-t border-border">
                <div className="p-4">
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2">
                      <Repeat size={14} className="text-accent" />
                      <span className="text-xs text-accent font-semibold uppercase">
                        {t('translator.backTranslation')}
                      </span>
                    </div>
                    <button
                      className="text-xs text-text-secondary hover:text-error transition-colors"
                      onClick={() => useTranslateStore.setState({ backTranslation: null })}
                    >
                      <X size={14} />
                    </button>
                  </div>
                  <p className="text-sm text-text-secondary italic">{backTranslation}</p>
                  <p className="text-xs text-text-secondary mt-2">
                    {t('translator.backTranslateHint')}
                  </p>
                </div>
              </div>
            )}

            {/* Dictionary Results */}
            {dictionaryResults.length > 0 && (
              <div className="border-t border-border">
                <div className="p-4">
                  <div className="flex items-center gap-2 mb-3">
                    <Book size={14} className="text-accent" />
                    <span className="text-xs text-accent font-semibold uppercase">
                      {t('translator.dictionary')}
                    </span>
                  </div>
                  {dictionaryResults.map((entry, idx) => (
                    <div key={idx} className="mb-4 last:mb-0">
                      <div className="flex items-baseline gap-2 mb-2">
                        <span className="text-lg font-bold text-text-primary">{entry.word}</span>
                        {entry.phonetic && (
                          <span className="text-sm text-text-secondary">{entry.phonetic}</span>
                        )}
                      </div>
                      {entry.meanings.map((meaning, mIdx) => (
                        <div key={mIdx} className="ml-2 mb-3">
                          <span className="text-xs text-primary font-medium italic">
                            {meaning.partOfSpeech}
                          </span>
                          <ul className="mt-1 space-y-1.5">
                            {meaning.definitions.slice(0, 3).map((def, dIdx) => (
                              <li key={dIdx} className="text-sm">
                                <span className="text-text-primary">{def.definition}</span>
                                {def.example && (
                                  <p className="text-xs text-text-secondary mt-0.5 italic">
                                    &ldquo;{def.example}&rdquo;
                                  </p>
                                )}
                              </li>
                            ))}
                          </ul>
                        </div>
                      ))}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
          {/* History Navigation Footer */}
          {translationHistory.length > 0 && (
            <div className="flex items-center justify-center gap-3 px-4 py-2 border-t border-border">
              <button
                className={`p-1.5 rounded-lg transition-colors ${
                  historyIndex > 0
                    ? 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
                    : 'text-text-secondary/30 cursor-not-allowed'
                }`}
                onClick={goToPreviousTranslation}
                disabled={historyIndex <= 0}
                title={`${t('translator.previousTranslation')} (Alt+Left)`}
              >
                <ChevronLeft size={16} />
              </button>
              <span className="text-xs text-text-secondary">
                {t('translator.historyPosition', {
                  current: String(historyIndex + 1),
                  total: String(translationHistory.length),
                })}
              </span>
              <button
                className={`p-1.5 rounded-lg transition-colors ${
                  historyIndex < translationHistory.length - 1
                    ? 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
                    : 'text-text-secondary/30 cursor-not-allowed'
                }`}
                onClick={goToNextTranslation}
                disabled={historyIndex >= translationHistory.length - 1}
                title={`${t('translator.nextTranslation')} (Alt+Right)`}
              >
                <ChevronRight size={16} />
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default MainTranslator;
