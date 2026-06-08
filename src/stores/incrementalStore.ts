import { create } from "zustand";
import type { TranslationResult } from "../types";

export interface IncrementalEntry {
  id: string;
  sourceText: string;
  results: TranslationResult[];
  timestamp: number;
}

interface IncrementalState {
  incrementalMode: boolean;
  incrementalEntries: IncrementalEntry[];

  toggleIncrementalMode: () => void;
  addIncrementalEntry: (entry: IncrementalEntry) => void;
  clearIncremental: () => void;
  removeIncrementalEntry: (id: string) => void;
}

export const useIncrementalStore = create<IncrementalState>((set) => ({
  incrementalMode: false,
  incrementalEntries: [],

  toggleIncrementalMode: () => {
    set((state) => ({ incrementalMode: !state.incrementalMode }));
  },

  addIncrementalEntry: (entry) => {
    set((state) => ({
      incrementalEntries: [...state.incrementalEntries, entry],
    }));
  },

  clearIncremental: () => {
    set({ incrementalEntries: [] });
  },

  removeIncrementalEntry: (id) => {
    set((state) => ({
      incrementalEntries: state.incrementalEntries.filter((e) => e.id !== id),
    }));
  },
}));
