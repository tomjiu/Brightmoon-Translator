import { create } from 'zustand';
import { safeInvoke } from '../services/invoke';
import { useTranslateStore } from './translateStore';
import type { EmbeddedLine } from '../types';

interface EmbeddedState {
  embeddedMode: boolean;
  embeddedLines: EmbeddedLine[];

  translateEmbedded: () => Promise<void>;
  toggleEmbeddedMode: () => void;
}

export const useEmbeddedStore = create<EmbeddedState>((set, get) => ({
  embeddedMode: false,
  embeddedLines: [],

  translateEmbedded: async () => {
    const { sourceText, fromLang, toLang } = useTranslateStore.getState();
    if (!sourceText.trim()) {
      set({ embeddedLines: [] });
      return;
    }

    useTranslateStore.setState({ loading: true, error: null });

    const [results, error] = await safeInvoke<EmbeddedLine[]>('translate_embedded', {
      text: sourceText.trim(),
      from: fromLang,
      to: toLang,
    });

    if (error || !results) {
      useTranslateStore.setState({
        error: error?.message || 'Embedded translation failed',
        loading: false,
      });
      return;
    }
    set({ embeddedLines: results });
    useTranslateStore.setState({ loading: false });
  },

  toggleEmbeddedMode: () => {
    const { embeddedMode } = get();
    const { sourceText } = useTranslateStore.getState();
    set({ embeddedMode: !embeddedMode });
    // If switching to embedded mode and we have source text, translate
    if (!embeddedMode && sourceText.trim()) {
      get().translateEmbedded();
    }
  },
}));
