import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useTranslateStore } from './translateStore';
import { safeInvoke } from '../services/invoke';

// Mock modules
vi.mock('../services/invoke', () => ({
  safeInvoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

describe('translateStore', () => {
  beforeEach(() => {
    // Reset all store states
    useTranslateStore.setState({
      sourceText: '',
      results: [],
      dictionaryResults: [],
      backTranslation: null,
      fromLang: 'auto',
      toLang: 'zh',
      loading: false,
      detectedLang: '',
      error: null,
      incrementalMode: false,
      incrementalEntries: [],
      polishing: false,
      realtimeTranslate: true,
      realtimeDelayMs: 500,
    });

    useTranslateStore.setState({
      translationHistory: [],
      historyIndex: -1,
    } as unknown);

    vi.clearAllMocks();
  });

  describe('setSourceText', () => {
    it('should update source text', () => {
      useTranslateStore.getState().setSourceText('Hello World');
      expect(useTranslateStore.getState().sourceText).toBe('Hello World');
    });

    it('should handle empty string', () => {
      useTranslateStore.setState({ sourceText: 'existing' });
      useTranslateStore.getState().setSourceText('');
      expect(useTranslateStore.getState().sourceText).toBe('');
    });
  });

  describe('setFromLang / setToLang', () => {
    it('should update from language', () => {
      useTranslateStore.getState().setFromLang('en');
      expect(useTranslateStore.getState().fromLang).toBe('en');
    });

    it('should update to language', () => {
      useTranslateStore.getState().setToLang('ja');
      expect(useTranslateStore.getState().toLang).toBe('ja');
    });
  });

  describe('swapLanguages', () => {
    it('should swap from and to languages', () => {
      useTranslateStore.setState({ fromLang: 'en', toLang: 'zh' });

      useTranslateStore.getState().swapLanguages();

      expect(useTranslateStore.getState().fromLang).toBe('zh');
      expect(useTranslateStore.getState().toLang).toBe('en');
    });

    it('should not swap when from is auto', () => {
      useTranslateStore.setState({ fromLang: 'auto', toLang: 'zh' });

      useTranslateStore.getState().swapLanguages();

      expect(useTranslateStore.getState().fromLang).toBe('auto');
      expect(useTranslateStore.getState().toLang).toBe('zh');
    });

    it('should swap source text with first result if results exist', () => {
      useTranslateStore.setState({
        fromLang: 'en',
        toLang: 'zh',
        results: [{ engine: 'google', text: '你好' }],
        sourceText: 'Hello',
      });

      // Mock translate to avoid actual call
      vi.mocked(safeInvoke).mockResolvedValue([[{ engine: 'google', text: 'Hello' }], null]);

      useTranslateStore.getState().swapLanguages();

      expect(useTranslateStore.getState().sourceText).toBe('你好');
    });
  });

  describe('translate', () => {
    it('should clear results for empty text', async () => {
      useTranslateStore.setState({
        sourceText: '',
        results: [{ engine: 'google', text: 'old' }],
      });

      await useTranslateStore.getState().translate();

      expect(useTranslateStore.getState().results).toEqual([]);
      expect(useTranslateStore.getState().error).toBeNull();
    });

    it('should call safeInvoke with correct parameters', async () => {
      useTranslateStore.setState({
        sourceText: 'Hello',
        fromLang: 'en',
        toLang: 'zh',
      });

      vi.mocked(safeInvoke).mockResolvedValue([
        { results: [{ engine: 'google', text: '你好' }], detectedLanguage: 'en' },
        null,
      ]);

      await useTranslateStore.getState().translate();

      expect(safeInvoke).toHaveBeenCalledWith('translate', {
        request: {
          text: 'Hello',
          from: 'en',
          to: 'zh',
          channel: 'ui',
        },
      });
    });

    it('should update results on success', async () => {
      useTranslateStore.setState({ sourceText: 'Hello' });

      vi.mocked(safeInvoke).mockResolvedValue([
        { results: [{ engine: 'google', text: '你好' }], detectedLanguage: 'en' },
        null,
      ]);

      await useTranslateStore.getState().translate();

      expect(useTranslateStore.getState().results).toEqual([{ engine: 'google', text: '你好' }]);
      expect(useTranslateStore.getState().detectedLang).toBe('en');
      expect(useTranslateStore.getState().loading).toBe(false);
    });

    it('should handle translation error', async () => {
      useTranslateStore.setState({ sourceText: 'Hello' });

      vi.mocked(safeInvoke).mockResolvedValue([
        null,
        { code: 'ERR', message: 'Translation failed' },
      ]);

      await useTranslateStore.getState().translate();

      expect(useTranslateStore.getState().error).toBe('Translation failed');
      expect(useTranslateStore.getState().loading).toBe(false);
    });

    it('should add to translation history', async () => {
      useTranslateStore.setState({ sourceText: 'Hello' });

      vi.mocked(safeInvoke).mockResolvedValue([
        { results: [{ engine: 'google', text: '你好' }], detectedLanguage: 'en' },
        null,
      ]);

      await useTranslateStore.getState().translate();

      const { translationHistory, historyIndex } = useTranslateStore.getState();
      expect(translationHistory).toHaveLength(1);
      expect(historyIndex).toBe(0);
      expect(translationHistory[0].sourceText).toBe('Hello');
    });

    it('should trim source text before translation', async () => {
      useTranslateStore.setState({ sourceText: '  Hello  ' });

      vi.mocked(safeInvoke).mockResolvedValue([
        { results: [{ engine: 'google', text: '你好' }], detectedLanguage: 'en' },
        null,
      ]);

      await useTranslateStore.getState().translate();

      expect(safeInvoke).toHaveBeenCalledWith('translate', {
        request: { text: 'Hello', from: 'auto', to: 'zh', channel: 'ui' },
      });
    });

    it('should handle incremental mode', async () => {
      useTranslateStore.setState({
        sourceText: 'Hello',
        incrementalMode: true,
        incrementalEntries: [],
      });

      vi.mocked(safeInvoke).mockResolvedValue([
        { results: [{ engine: 'google', text: '你好' }], detectedLanguage: 'en' },
        null,
      ]);

      await useTranslateStore.getState().translate();

      const { incrementalEntries } = useTranslateStore.getState();
      expect(incrementalEntries).toHaveLength(1);
      expect(incrementalEntries[0].sourceText).toBe('Hello');
    });
  });

  describe('clear', () => {
    it('should clear all translation state', () => {
      useTranslateStore.setState({
        sourceText: 'Hello',
        results: [{ engine: 'google', text: '你好' }],
        dictionaryResults: [{ word: 'hello', meanings: [], sourceUrls: [] }],
        backTranslation: 'Hello back',
        error: 'Some error',
        detectedLang: 'en',
      });

      useTranslateStore.getState().clear();

      const state = useTranslateStore.getState();
      expect(state.sourceText).toBe('');
      expect(state.results).toEqual([]);
      expect(state.dictionaryResults).toEqual([]);
      expect(state.backTranslation).toBeNull();
      expect(state.error).toBeNull();
      expect(state.detectedLang).toBe('');
    });
  });
});
