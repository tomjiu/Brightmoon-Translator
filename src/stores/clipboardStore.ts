import { create } from "zustand";
import { safeInvoke } from "../services/invoke";
import { listen } from "@tauri-apps/api/event";
import { useTranslateStore } from "./translateStore";

interface ClipboardState {
  clipboardMonitorEnabled: boolean;
  clipboardUnlisten: (() => void) | null;
  lastClipboardText: string;

  toggleClipboardMonitor: () => void;
  startClipboardMonitor: () => Promise<void>;
  stopClipboardMonitor: () => Promise<void>;
}

export const useClipboardStore = create<ClipboardState>((set, get) => ({
  clipboardMonitorEnabled: false,
  clipboardUnlisten: null,
  lastClipboardText: "",

  toggleClipboardMonitor: () => {
    const { clipboardMonitorEnabled } = get();
    if (clipboardMonitorEnabled) {
      get().stopClipboardMonitor();
    } else {
      get().startClipboardMonitor();
    }
  },

  startClipboardMonitor: async () => {
    // Clean up existing listener if any
    const { clipboardUnlisten } = get();
    if (clipboardUnlisten) {
      clipboardUnlisten();
    }

    const [, error] = await safeInvoke("start_clipboard_monitor");
    if (error) {
      console.error("Failed to start clipboard monitor:", error);
      return;
    }

    // Listen for clipboard read events and save unlisten function
    const unlisten = await listen("read-clipboard", async () => {
      try {
        const text = await navigator.clipboard.readText();
        const { sourceText } = useTranslateStore.getState();
        if (text && text !== sourceText) {
          useTranslateStore.getState().setSourceText(text);
          set({ lastClipboardText: text });
          // Auto-translate clipboard content
          setTimeout(() => useTranslateStore.getState().translate(), 100);
        }
      } catch {
        // Clipboard read failed silently
      }
    });

    set({ clipboardMonitorEnabled: true, clipboardUnlisten: unlisten });
  },

  stopClipboardMonitor: async () => {
    const [, error] = await safeInvoke("stop_clipboard_monitor");
    if (error) {
      console.error("Failed to stop clipboard monitor:", error);
      return;
    }

    // Clean up event listener
    const { clipboardUnlisten } = get();
    if (clipboardUnlisten) {
      clipboardUnlisten();
    }

    set({ clipboardMonitorEnabled: false, clipboardUnlisten: null });
  },
}));
