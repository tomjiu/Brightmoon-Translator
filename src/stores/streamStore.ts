import { create } from "zustand";

interface StreamState {
  isStreaming: boolean;
  streamingText: string;
  streamError: string | null;

  setStreaming: (streaming: boolean) => void;
  setStreamingText: (text: string) => void;
  setStreamError: (error: string | null) => void;
  resetStream: () => void;
}

export const useStreamStore = create<StreamState>((set) => ({
  isStreaming: false,
  streamingText: "",
  streamError: null,

  setStreaming: (streaming) => set({ isStreaming: streaming }),

  setStreamingText: (text) => set({ streamingText: text }),

  setStreamError: (error) => set({ streamError: error }),

  resetStream: () => set({ isStreaming: false, streamingText: "", streamError: null }),
}));
