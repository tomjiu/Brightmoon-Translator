import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { TranslationResult, TranslateResponse, DictionaryResult, DetectionResult, EmbeddedLine } from "../types";

export interface IncrementalEntry {
  id: string;
  sourceText: string;
  results: TranslationResult[];
  timestamp: number;
}

interface HistoryEntry {
  sourceText: string;
  results: TranslationResult[];
  fromLang: string;
  toLang: string;
}

interface TranslateState {
  sourceText: string;
  results: TranslationResult[];
  dictionaryResults: DictionaryResult[];
  backTranslation: string | null;
  fromLang: string;
  toLang: string;
  loading: boolean;
  detectedLang: string;
  error: string | null;
  streamingText: string;
  isStreaming: boolean;
  clipboardMonitorEnabled: boolean;
  clipboardUnlisten: (() => void) | null;
  incrementalMode: boolean;
  incrementalEntries: IncrementalEntry[];
  translationHistory: HistoryEntry[];
  historyIndex: number;
  embeddedLines: EmbeddedLine[];
  embeddedMode: boolean;
  polishing: boolean;

  setSourceText: (text: string) => void;
  setFromLang: (lang: string) => void;
  setToLang: (lang: string) => void;
  swapLanguages: () => void;
  translate: () => Promise<void>;
  translateStream: () => Promise<void>;
  lookupDictionary: () => Promise<void>;
  backTranslate: (text: string) => Promise<void>;
  polishTranslation: () => Promise<void>;
  detectLanguage: (text: string) => Promise<void>;
  clear: () => void;
  clearIncremental: () => void;
  removeIncrementalEntry: (id: string) => void;
  toggleIncrementalMode: () => void;
  toggleClipboardMonitor: () => void;
  startClipboardMonitor: () => Promise<void>;
  stopClipboardMonitor: () => Promise<void>;
  goToPreviousTranslation: () => void;
  goToNextTranslation: () => void;
  moveWindowToCursor: () => Promise<void>;
  translateEmbedded: () => Promise<void>;
  toggleEmbeddedMode: () => void;
}

export const useTranslateStore = create<TranslateState>((set, get) => ({
  sourceText: "",
  results: [],
  dictionaryResults: [],
  backTranslation: null,
  fromLang: "auto",
  toLang: "zh",
  loading: false,
  detectedLang: "",
  error: null,
  streamingText: "",
  isStreaming: false,
  clipboardMonitorEnabled: false,
  clipboardUnlisten: null,
  incrementalMode: false,
  incrementalEntries: [],
  translationHistory: [],
  historyIndex: -1,
  embeddedLines: [],
  embeddedMode: false,
  polishing: false,

  setSourceText: (text) => set({ sourceText: text }),

  setFromLang: (lang) => set({ fromLang: lang }),

  setToLang: (lang) => set({ toLang: lang }),

  swapLanguages: () => {
    const { fromLang, toLang, results } = get();
    if (fromLang === "auto") return;
    const newFrom = toLang;
    const newTo = fromLang;
    set({ fromLang: newFrom, toLang: newTo });
    // If we have results, swap the text
    if (results.length > 0) {
      set({ sourceText: results[0].text });
      // Trigger new translation after swap
      setTimeout(() => get().translate(), 0);
    }
  },

  translate: async () => {
    const { sourceText, fromLang, toLang, incrementalMode, incrementalEntries, translationHistory } = get();
    if (!sourceText.trim()) {
      if (!incrementalMode) {
        set({ results: [], error: null });
      }
      return;
    }

    set({ loading: true, error: null });

    try {
      const response = await invoke<TranslateResponse>("translate", {
        request: {
          text: sourceText.trim(),
          from: fromLang,
          to: toLang,
        },
      });

      // Add to translation history
      const historyEntry: HistoryEntry = {
        sourceText: sourceText.trim(),
        results: response.results,
        fromLang,
        toLang,
      };
      const newHistory = [...translationHistory, historyEntry];
      // Keep only last 100 entries
      if (newHistory.length > 100) newHistory.shift();

      if (incrementalMode) {
        // Append to incremental entries
        const entry: IncrementalEntry = {
          id: Date.now().toString(),
          sourceText: sourceText.trim(),
          results: response.results,
          timestamp: Date.now(),
        };
        set({
          results: response.results,
          incrementalEntries: [...incrementalEntries, entry],
          detectedLang: response.detectedLanguage || "",
          loading: false,
          translationHistory: newHistory,
          historyIndex: newHistory.length - 1,
        });
      } else {
        set({
          results: response.results,
          detectedLang: response.detectedLanguage || "",
          loading: false,
          translationHistory: newHistory,
          historyIndex: newHistory.length - 1,
        });
      }
    } catch (err) {
      set({
        error: String(err),
        loading: false,
      });
    }
  },

  translateStream: async () => {
    const { sourceText, fromLang, toLang, incrementalMode, incrementalEntries, translationHistory } = get();
    if (!sourceText.trim()) {
      if (!incrementalMode) {
        set({ results: [], error: null });
      }
      return;
    }

    set({ loading: true, error: null, isStreaming: true, streamingText: "" });

    // Listen for streaming chunks
    let fullText = "";
    const unlisten = await listen<{ chunk: string; done: boolean }>("stream-chunk", (event) => {
      if (event.payload.done) {
        // Streaming complete
        const newResults = [{ engine: "LLM", text: fullText }];
        const historyEntry: HistoryEntry = {
          sourceText: sourceText.trim(),
          results: newResults,
          fromLang,
          toLang,
        };
        const newHistory = [...translationHistory, historyEntry];
        if (newHistory.length > 100) newHistory.shift();

        if (incrementalMode) {
          const entry: IncrementalEntry = {
            id: Date.now().toString(),
            sourceText: sourceText.trim(),
            results: newResults,
            timestamp: Date.now(),
          };
          set({
            results: newResults,
            incrementalEntries: [...incrementalEntries, entry],
            loading: false,
            isStreaming: false,
            streamingText: "",
            translationHistory: newHistory,
            historyIndex: newHistory.length - 1,
          });
        } else {
          set({
            results: newResults,
            loading: false,
            isStreaming: false,
            streamingText: "",
            translationHistory: newHistory,
            historyIndex: newHistory.length - 1,
          });
        }
      } else {
        // Accumulate chunks
        fullText += event.payload.chunk;
        set({ streamingText: fullText });
      }
    });

    try {
      // Invoke backend streaming (returns when complete)
      await invoke<string>("translate_stream", {
        request: {
          text: sourceText.trim(),
          from: fromLang,
          to: toLang,
        },
      });
    } catch (err) {
      set({
        error: String(err),
        loading: false,
        isStreaming: false,
      });
    } finally {
      unlisten();
    }
  },

  lookupDictionary: async () => {
    const { sourceText } = get();
    if (!sourceText.trim()) {
      set({ dictionaryResults: [] });
      return;
    }

    try {
      const results = await invoke<DictionaryResult[]>("lookup_dictionary", {
        text: sourceText.trim(),
      });
      set({ dictionaryResults: results });
    } catch (err) {
      // Dictionary lookup is optional, don't show error
      console.error("Dictionary lookup failed:", err);
      set({ dictionaryResults: [] });
    }
  },

  backTranslate: async (text: string) => {
    const { fromLang, toLang } = get();
    if (!text.trim()) {
      set({ backTranslation: null });
      return;
    }

    try {
      const result = await invoke<string>("back_translate", {
        text: text.trim(),
        from: fromLang,
        to: toLang,
      });
      set({ backTranslation: result });
    } catch (err) {
      console.error("Back-translation failed:", err);
      set({ backTranslation: null });
    }
  },

  polishTranslation: async () => {
    const { sourceText, results, fromLang, toLang } = get();
    if (!sourceText.trim() || results.length === 0) {
      return;
    }

    set({ polishing: true });

    try {
      const translatedText = results[0].text;
      const polished = await invoke<string>("polish_translation", {
        sourceText: sourceText.trim(),
        translatedText,
        fromLang,
        toLang,
      });

      // Update the first result with polished text
      const newResults = [...results];
      newResults[0] = { ...newResults[0], text: polished };
      set({ results: newResults });
    } catch (err) {
      console.error("Polish failed:", err);
    } finally {
      set({ polishing: false });
    }
  },

  detectLanguage: async (text: string) => {
    if (!text.trim()) {
      set({ detectedLang: "" });
      return;
    }

    try {
      const result = await invoke<DetectionResult>("detect_language", {
        text: text.trim(),
      });
      if (result.language !== "auto") {
        set({ detectedLang: result.name });
      } else {
        set({ detectedLang: "" });
      }
    } catch (err) {
      console.error("Language detection failed:", err);
      set({ detectedLang: "" });
    }
  },

  clear: () =>
    set({
      sourceText: "",
      results: [],
      dictionaryResults: [],
      backTranslation: null,
      error: null,
      detectedLang: "",
      streamingText: "",
    }),

  clearIncremental: () =>
    set({
      incrementalEntries: [],
      results: [],
      sourceText: "",
    }),

  removeIncrementalEntry: (id: string) => {
    set((state) => ({
      incrementalEntries: state.incrementalEntries.filter((e) => e.id !== id),
    }));
  },

  toggleIncrementalMode: () => {
    set((state) => ({ incrementalMode: !state.incrementalMode }));
  },

  toggleClipboardMonitor: () => {
    const { clipboardMonitorEnabled } = get();
    if (clipboardMonitorEnabled) {
      get().stopClipboardMonitor();
    } else {
      get().startClipboardMonitor();
    }
  },

  startClipboardMonitor: async () => {
    try {
      // Clean up existing listener if any
      const { clipboardUnlisten } = get();
      if (clipboardUnlisten) {
        clipboardUnlisten();
      }

      await invoke("start_clipboard_monitor");

      // Listen for clipboard read events and save unlisten function
      const unlisten = await listen("read-clipboard", async () => {
        try {
          const text = await navigator.clipboard.readText();
          const { sourceText } = get();
          if (text && text !== sourceText) {
            set({ sourceText: text });
            // Auto-translate clipboard content
            setTimeout(() => get().translate(), 100);
          }
        } catch (err) {
          // Clipboard read failed silently
        }
      });

      set({ clipboardMonitorEnabled: true, clipboardUnlisten: unlisten });
    } catch (err) {
      console.error("Failed to start clipboard monitor:", err);
    }
  },

  stopClipboardMonitor: async () => {
    try {
      await invoke("stop_clipboard_monitor");

      // Clean up event listener
      const { clipboardUnlisten } = get();
      if (clipboardUnlisten) {
        clipboardUnlisten();
      }

      set({ clipboardMonitorEnabled: false, clipboardUnlisten: null });
    } catch (err) {
      console.error("Failed to stop clipboard monitor:", err);
    }
  },

  goToPreviousTranslation: () => {
    const { translationHistory, historyIndex } = get();
    if (translationHistory.length === 0 || historyIndex <= 0) return;

    const newIndex = historyIndex - 1;
    const entry = translationHistory[newIndex];
    set({
      historyIndex: newIndex,
      sourceText: entry.sourceText,
      results: entry.results,
      fromLang: entry.fromLang,
      toLang: entry.toLang,
    });
  },

  goToNextTranslation: () => {
    const { translationHistory, historyIndex } = get();
    if (historyIndex >= translationHistory.length - 1) return;

    const newIndex = historyIndex + 1;
    const entry = translationHistory[newIndex];
    set({
      historyIndex: newIndex,
      sourceText: entry.sourceText,
      results: entry.results,
      fromLang: entry.fromLang,
      toLang: entry.toLang,
    });
  },

  moveWindowToCursor: async () => {
    try {
      await invoke("move_window_to_cursor");
    } catch (err) {
      console.error("Failed to move window to cursor:", err);
    }
  },

  translateEmbedded: async () => {
    const { sourceText, fromLang, toLang } = get();
    if (!sourceText.trim()) {
      set({ embeddedLines: [] });
      return;
    }

    set({ loading: true, error: null });

    try {
      const results = await invoke<EmbeddedLine[]>("translate_embedded", {
        text: sourceText.trim(),
        from: fromLang,
        to: toLang,
      });
      set({ embeddedLines: results, loading: false });
    } catch (err) {
      set({ error: String(err), loading: false });
    }
  },

  toggleEmbeddedMode: () => {
    const { embeddedMode } = get();
    set({ embeddedMode: !embeddedMode });
    // If switching to embedded mode and we have source text, translate
    if (!embeddedMode && get().sourceText.trim()) {
      get().translateEmbedded();
    }
  },
}));
