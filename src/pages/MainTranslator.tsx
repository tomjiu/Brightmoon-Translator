import { useEffect, useRef, useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslateStore } from "../stores/translateStore";
import { useConfigStore } from "../stores/configStore";
import { useI18n } from "../i18n";
import { LANGUAGES } from "../types";
import { useSpeechRecognition } from "../hooks/useSpeechRecognition";
import {
  ArrowLeftRight,
  Copy,
  Check,
  X,
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
} from "lucide-react";
import OcrSelector from "../components/OcrSelector";

function MainTranslator() {
  const {
    sourceText,
    results,
    dictionaryResults,
    backTranslation,
    fromLang,
    toLang,
    loading,
    detectedLang,
    error,
    streamingText,
    isStreaming,
    incrementalMode,
    incrementalEntries,
    translationHistory,
    historyIndex,
    setSourceText,
    setFromLang,
    setToLang,
    swapLanguages,
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

  const { config } = useConfigStore();
  const { t } = useI18n();

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
        setSourceText(currentText ? currentText + " " + text : text);
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

  const handleInput = useCallback(
    (value: string) => {
      setSourceText(value);
      setMaskRevealed(false);
      if (debounceTimer.current) {
        clearTimeout(debounceTimer.current);
      }
      debounceTimer.current = setTimeout(() => {
        if (value.trim()) {
          detectLanguage(value);
          if (useTranslateStore.getState().embeddedMode) {
            translateEmbedded();
          } else {
            translateStream();
          }
          lookupDictionary();
        }
      }, 500);
    },
    [setSourceText, translateStream, lookupDictionary, detectLanguage]
  );

  const copyResult = (text: string, index: number) => {
    navigator.clipboard.writeText(text);
    setCopiedIndex(index);
    setTimeout(() => setCopiedIndex(null), 1500);
  };

  const speakText = async (text: string, lang: string, index: number) => {
    try {
      setSpeakingIndex(index);
      const base64Audio = await invoke<string>("text_to_speech", { text, lang });
      const audioBytes = Uint8Array.from(atob(base64Audio), c => c.charCodeAt(0));
      const audioBlob = new Blob([audioBytes], { type: "audio/mp3" });
      const audioUrl = URL.createObjectURL(audioBlob);
      const audio = new Audio(audioUrl);
      audio.onended = () => {
        setSpeakingIndex(null);
        URL.revokeObjectURL(audioUrl);
      };
      audio.onerror = () => {
        setSpeakingIndex(null);
        URL.revokeObjectURL(audioUrl);
      };
      await audio.play();
    } catch (err) {
      console.error("TTS failed:", err);
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
      if (e.altKey && e.key === "ArrowLeft") {
        e.preventDefault();
        goToPreviousTranslation();
      } else if (e.altKey && e.key === "ArrowRight") {
        e.preventDefault();
        goToNextTranslation();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [goToPreviousTranslation, goToNextTranslation]);

  // Window follow mode: move window to cursor on selection translation
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setup = async () => {
      unlisten = await listen("trigger-translate-selection", () => {
        if (config.windowFollowMode === "cursor") {
          moveWindowToCursor();
        }
      });
    };
    setup();

    return () => {
      if (unlisten) unlisten();
    };
  }, [config.windowFollowMode, moveWindowToCursor]);

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  };

  return (
    <div className="flex flex-col h-full p-4 gap-3">
      {/* Language Bar */}
      <div className="flex items-center justify-center gap-3">
        <select
          value={fromLang}
          onChange={(e) => setFromLang(e.target.value)}
          className="bg-bg-secondary text-text-primary border border-border rounded-lg px-4 py-2 text-sm cursor-pointer min-w-[120px] focus:border-primary"
        >
          {LANGUAGES.map((l) => (
            <option key={l.code} value={l.code}>
              {l.name}
            </option>
          ))}
        </select>

        {detectedLang && fromLang === "auto" && (
          <span className="text-xs text-text-secondary">
            {t("translator.detected")}: {detectedLang}
          </span>
        )}

        <button
          className="bg-bg-tertiary border border-border text-text-primary rounded-lg px-4 py-2 text-lg hover:bg-primary hover:text-white transition-colors"
          onClick={swapLanguages}
          title={t("translator.swapLang")}
        >
          <ArrowLeftRight size={18} />
        </button>

        <select
          value={toLang}
          onChange={(e) => setToLang(e.target.value)}
          className="bg-bg-secondary text-text-primary border border-border rounded-lg px-4 py-2 text-sm cursor-pointer min-w-[120px] focus:border-primary"
        >
          {LANGUAGES.filter((l) => l.code !== "auto").map((l) => (
            <option key={l.code} value={l.code}>
              {l.name}
            </option>
          ))}
        </select>

        <div className="ml-2 flex items-center gap-2">
          <OcrSelector />

          {/* Incremental Mode Toggle */}
          <button
            className={`w-9 h-9 rounded-lg flex items-center justify-center transition-colors ${
              incrementalMode
                ? "bg-accent text-white"
                : "bg-bg-tertiary text-text-secondary hover:bg-bg-tertiary/80"
            }`}
            onClick={toggleIncrementalMode}
            title={t(incrementalMode ? "translator.incrementalModeOn" : "translator.incrementalModeOff")}
          >
            <Layers size={16} />
          </button>

          {/* Delete Newlines Toggle */}
          <button
            className={`w-9 h-9 rounded-lg flex items-center justify-center transition-colors ${
              deleteNewlines
                ? "bg-warning text-white"
                : "bg-bg-tertiary text-text-secondary hover:bg-bg-tertiary/80"
            }`}
            onClick={() => setDeleteNewlines(!deleteNewlines)}
            title={t(deleteNewlines ? "translator.keepNewlines" : "translator.deleteNewlines")}
          >
            <Eraser size={16} />
          </button>

          {/* Clipboard Monitor Toggle */}
          <button
            className={`w-9 h-9 rounded-lg flex items-center justify-center transition-colors ${
              clipboardMonitorEnabled
                ? "bg-primary text-white"
                : "bg-bg-tertiary text-text-secondary hover:bg-bg-tertiary/80"
            }`}
            onClick={toggleClipboardMonitor}
            title={t(clipboardMonitorEnabled ? "translator.stopClipboardMonitor" : "translator.startClipboardMonitor")}
          >
            <Clipboard size={16} />
          </button>

          {/* Bilingual Mode Toggle */}
          <button
            className={`w-9 h-9 rounded-lg flex items-center justify-center transition-colors ${
              bilingualMode
                ? "bg-accent text-white"
                : "bg-bg-tertiary text-text-secondary hover:bg-bg-tertiary/80"
            }`}
            onClick={() => setBilingualMode(!bilingualMode)}
            title={t(bilingualMode ? "translator.bilingualOff" : "translator.bilingualOn")}
          >
            <Columns size={16} />
          </button>

          {/* Embedded Translation Mode Toggle */}
          <button
            className={`w-9 h-9 rounded-lg flex items-center justify-center transition-colors ${
              embeddedMode
                ? "bg-primary text-white"
                : "bg-bg-tertiary text-text-secondary hover:bg-bg-tertiary/80"
            }`}
            onClick={toggleEmbeddedMode}
            title={t(embeddedMode ? "translator.embeddedOff" : "translator.embeddedOn")}
          >
            <BookOpen size={16} />
          </button>
        </div>
      </div>

      {/* Translation Area */}
      <div className="flex gap-3 flex-1 min-h-0">
        {/* Source Panel */}
        <div className="flex-1 flex flex-col bg-bg-secondary border border-border rounded-xl overflow-hidden">
          <div className="flex-1 relative">
            <textarea
              value={sourceText}
              onChange={(e) => handleInput(e.target.value)}
              placeholder={t("translator.placeholder")}
              className="w-full h-full bg-transparent text-text-primary p-4 text-sm leading-relaxed resize-none outline-none placeholder:text-text-secondary"
              autoFocus
            />
            {isListening && interimTranscript && (
              <div className="absolute bottom-2 left-4 right-4 text-xs text-primary bg-primary/10 rounded-lg px-3 py-1.5">
                {interimTranscript}...
              </div>
            )}
          </div>
          <div className="flex justify-between items-center px-4 py-2 border-t border-border">
            <div className="flex items-center gap-2">
              <span className="text-xs text-text-secondary">
                {sourceText.length} {t("translator.chars")}
                {incrementalMode && (
                  <span className="ml-2 text-accent">{t("translator.incrementalMode")}</span>
                )}
              </span>
              {speechError && (
                <span className="text-xs text-error">{speechError}</span>
              )}
            </div>
            <div className="flex items-center gap-2">
              {/* Speech Recognition Button */}
              {isSpeechSupported && (
                <button
                  className={`p-1.5 rounded-lg transition-colors flex items-center gap-1 ${
                    isListening
                      ? "bg-error text-white animate-pulse"
                      : "text-text-secondary hover:text-primary hover:bg-bg-tertiary"
                  }`}
                  onClick={() => {
                    if (isListening) {
                      // Consume any remaining transcript before stopping
                      const remaining = consumeTranscript();
                      if (remaining) {
                        const currentText = useTranslateStore.getState().sourceText;
                        setSourceText(currentText ? currentText + " " + remaining : remaining);
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
                  title={isListening ? t("translator.stopListening") : t("translator.startListening")}
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
                  {t("translator.clear")}
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Result Panel */}
        <div className="flex-1 flex flex-col bg-bg-secondary border border-border rounded-xl overflow-hidden">
          <div className="flex-1 overflow-y-auto">
            {/* Incremental Entries */}
            {incrementalMode && incrementalEntries.length > 0 && (
              <div className="p-2">
                <div className="flex items-center justify-between mb-2 px-2">
                  <span className="text-xs text-accent font-semibold">
                    {t("translator.appendRecords")} ({incrementalEntries.length})
                  </span>
                  <button
                    className="text-xs text-text-secondary hover:text-error transition-colors flex items-center gap-1"
                    onClick={clearIncremental}
                  >
                    <X size={12} />
                    {t("translator.emptyAppendRecords")}
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
                      {entry.sourceText.length > 50 ? "..." : ""}
                    </div>
                    <div className="text-sm text-primary">
                      {entry.results[0]?.text || ""}
                    </div>
                    <div className="text-xs text-text-secondary mt-1">
                      {formatTime(entry.timestamp)}
                    </div>
                  </div>
                ))}
              </div>
            )}

            {/* Embedded Translation Mode */}
            {embeddedMode && embeddedLines.length > 0 && (
              <div className="p-4">
                <div className="flex items-center gap-2 mb-3">
                  <BookOpen size={14} className="text-primary" />
                  <span className="text-xs text-primary font-semibold uppercase">
                    {t("translator.embeddedTitle")}
                  </span>
                </div>
                <div className="space-y-3">
                  {embeddedLines.map((line) => (
                    <div
                      key={line.lineNumber}
                      className="border-l-2 border-primary/30 pl-3"
                    >
                      <p className="text-sm text-text-secondary leading-relaxed select-text">
                        {line.original}
                      </p>
                      <p className="text-sm text-text-primary leading-relaxed select-text mt-1">
                        {line.translated}
                      </p>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Current Results */}
            {!embeddedMode && (
              loading || isStreaming ? (
                isStreaming && streamingText ? (
                  <div className="p-4">
                    <div className="flex justify-between items-center mb-2">
                      <span className="text-xs text-primary font-semibold uppercase">
                        {t("translator.streaming")}
                      </span>
                    </div>
                    <div className="text-sm leading-relaxed text-text-primary select-text">
                      {streamingText}
                      <span className="animate-pulse text-primary">|</span>
                    </div>
                  </div>
                ) : (
                  <div className="flex items-center justify-center h-full text-text-secondary">
                    <div className="animate-pulse">{t("translator.translating")}</div>
                  </div>
                )
              ) : error ? (
                <div className="p-4 text-error text-sm">{error}</div>
              ) : results.length > 0 ? (
                results.map((r, i) => (
                <div
                  key={i}
                  className="p-4 border-b border-border last:border-b-0"
                >
                  {/* Bilingual: show source text above translation */}
                  {bilingualMode && (
                    <div className="mb-3 pb-2 border-b border-border/50">
                      <div className="flex items-center gap-1.5 mb-1">
                        <AlignLeft size={10} className="text-text-secondary" />
                        <span className="text-xs text-text-secondary">{t("translator.sourceText")}</span>
                      </div>
                      <p className="text-sm text-text-secondary leading-relaxed select-text">
                        {sourceText}
                      </p>
                    </div>
                  )}
                  <div className="flex justify-between items-center mb-2">
                    <span className="text-xs text-primary font-semibold uppercase">
                      {r.engine}
                    </span>
                    <div className="flex items-center gap-2">
                      {config.translationMask && (
                        <button
                          className={`border border-border rounded-md px-2 py-1 text-xs transition-colors flex items-center gap-1 ${
                            maskRevealed
                              ? "bg-bg-tertiary text-text-secondary"
                              : "bg-warning/20 text-warning border-warning"
                          }`}
                          onClick={() => setMaskRevealed(!maskRevealed)}
                          title={t(maskRevealed ? "translator.hideOriginal" : "translator.showOriginal")}
                        >
                          {maskRevealed ? (
                            <>
                              <EyeOff size={12} />
                              {t("translator.hideOriginal")}
                            </>
                          ) : (
                            <>
                              <Eye size={12} />
                              {t("translator.showOriginal")}
                            </>
                          )}
                        </button>
                      )}
                      <button
                        className={`border border-border rounded-md px-2 py-1 text-xs transition-colors flex items-center gap-1 ${
                          speakingIndex === i
                            ? "bg-primary text-white border-primary"
                            : "bg-bg-tertiary text-text-secondary hover:bg-primary hover:text-white hover:border-primary"
                        }`}
                        onClick={() => speakText(r.text, toLang, i)}
                        title={t("translator.speak")}
                      >
                        <Volume2 size={12} />
                        {speakingIndex === i ? t("translator.speaking") : t("translator.speak")}
                      </button>
                      <button
                        className="bg-bg-tertiary border border-border text-text-secondary rounded-md px-2 py-1 text-xs hover:bg-primary hover:text-white hover:border-primary transition-colors flex items-center gap-1"
                        onClick={() => copyResult(r.text, i)}
                      >
                        {copiedIndex === i ? (
                          <>
                            <Check size={12} />
                            {t("translator.copied")}
                          </>
                        ) : (
                          <>
                            <Copy size={12} />
                            {t("translator.copy")}
                          </>
                        )}
                      </button>
                      {i === 0 && (
                        <>
                          <button
                            className="bg-bg-tertiary border border-border text-text-secondary rounded-md px-2 py-1 text-xs hover:bg-accent hover:text-white hover:border-accent transition-colors flex items-center gap-1"
                            onClick={() => backTranslate(r.text)}
                            title={t("translator.backTranslate")}
                          >
                            <Repeat size={12} />
                            {t("translator.backTranslate")}
                          </button>
                          <button
                            className="bg-bg-tertiary border border-border text-text-secondary rounded-md px-2 py-1 text-xs hover:bg-accent hover:text-white hover:border-accent transition-colors flex items-center gap-1 disabled:opacity-50"
                            onClick={polishTranslation}
                            disabled={polishing}
                            title={t("translator.polish")}
                          >
                            <Sparkles size={12} />
                            {polishing ? t("translator.polishing") : t("translator.polish")}
                          </button>
                        </>
                      )}
                    </div>
                  </div>
                  {config.translationMask && !maskRevealed ? (
                    <div
                      className="text-sm leading-relaxed text-text-primary select-text cursor-pointer bg-bg-tertiary/50 rounded-lg p-3 text-center hover:bg-bg-tertiary transition-colors"
                      onClick={() => setMaskRevealed(true)}
                    >
                      <Eye size={16} className="inline mr-2 text-text-secondary" />
                      <span className="text-text-secondary">
                        {t("translator.clickToShow")}
                      </span>
                    </div>
                  ) : (
                    <div className="text-sm leading-relaxed text-text-primary select-text">
                      {r.text}
                    </div>
                  )}
                </div>
              ))
              ) : (
                <div className="flex items-center justify-center h-full text-text-secondary text-sm">
                  {incrementalMode && incrementalEntries.length > 0
                    ? t("translator.continueInput")
                    : t("translator.resultPlaceholder")}
                </div>
              )
            )}

            {/* Back Translation Result */}
            {backTranslation && (
              <div className="border-t border-border">
                <div className="p-4">
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2">
                      <Repeat size={14} className="text-accent" />
                      <span className="text-xs text-accent font-semibold uppercase">
                        {t("translator.backTranslation")}
                      </span>
                    </div>
                    <button
                      className="text-xs text-text-secondary hover:text-error transition-colors"
                      onClick={() => useTranslateStore.setState({ backTranslation: null })}
                    >
                      <X size={14} />
                    </button>
                  </div>
                  <p className="text-sm text-text-secondary italic">
                    {backTranslation}
                  </p>
                  <p className="text-xs text-text-secondary mt-2">
                    {t("translator.backTranslateHint")}
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
                      {t("translator.dictionary")}
                    </span>
                  </div>
                  {dictionaryResults.map((entry, idx) => (
                    <div key={idx} className="mb-4 last:mb-0">
                      <div className="flex items-baseline gap-2 mb-2">
                        <span className="text-lg font-bold text-text-primary">
                          {entry.word}
                        </span>
                        {entry.phonetic && (
                          <span className="text-sm text-text-secondary">
                            {entry.phonetic}
                          </span>
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
                                <span className="text-text-primary">
                                  {def.definition}
                                </span>
                                {def.example && (
                                  <p className="text-xs text-text-secondary mt-0.5 italic">
                                    "{def.example}"
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
                    ? "text-text-secondary hover:bg-bg-tertiary hover:text-text-primary"
                    : "text-text-secondary/30 cursor-not-allowed"
                }`}
                onClick={goToPreviousTranslation}
                disabled={historyIndex <= 0}
                title={`${t("translator.previousTranslation")} (Alt+Left)`}
              >
                <ChevronLeft size={16} />
              </button>
              <span className="text-xs text-text-secondary">
                {t("translator.historyPosition", {
                  current: String(historyIndex + 1),
                  total: String(translationHistory.length),
                })}
              </span>
              <button
                className={`p-1.5 rounded-lg transition-colors ${
                  historyIndex < translationHistory.length - 1
                    ? "text-text-secondary hover:bg-bg-tertiary hover:text-text-primary"
                    : "text-text-secondary/30 cursor-not-allowed"
                }`}
                onClick={goToNextTranslation}
                disabled={historyIndex >= translationHistory.length - 1}
                title={`${t("translator.nextTranslation")} (Alt+Right)`}
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
