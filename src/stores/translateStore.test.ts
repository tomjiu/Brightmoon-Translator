import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useTranslateStore } from './translateStore';
import { useIncrementalStore } from './incrementalStore';

import { useEmbeddedStore } from './embeddedStore';
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

    useIncrementalStore.setState({
      incrementalMode: false,
      incrementalEntries: [],
    });

    useTranslateStore.setState({
      translationHistory: [],
      historyIndex: -1,
    } as unknown);

    useEmbeddedStore.setState({
      embeddedMode: false,
      embeddedLines: [],
    });

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
        request: { text: 'Hello', from: 'auto', to: 'zh' },
      });
    });

    it('should handle incremental mode', async () => {
      useTranslateStore.setState({
        sourceText: 'Hello',
        incrementalMode: true,
        incrementalEntries: [],
      });
      useIncrementalStore.setState({ incrementalMode: true, incrementalEntries: [] });

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

  describe('incrementalStore', () => {
    it('should clear incremental entries', () => {
      useIncrementalStore.setState({
        incrementalEntries: [{ id: '1', sourceText: 'Hello', results: [], timestamp: Date.now() }],
      });

      useIncrementalStore.getState().clearIncremental();

      expect(useIncrementalStore.getState().incrementalEntries).toEqual([]);
    });

    it('should remove specific entry by id', () => {
      useIncrementalStore.setState({
        incrementalEntries: [
          { id: '1', sourceText: 'Hello', results: [], timestamp: 1 },
          { id: '2', sourceText: 'World', results: [], timestamp: 2 },
        ],
      });

      useIncrementalStore.getState().removeIncrementalEntry('1');

      expect(useIncrementalStore.getState().incrementalEntries).toHaveLength(1);
      expect(useIncrementalStore.getState().incrementalEntries[0].id).toBe('2');
    });

    it('should toggle incremental mode', () => {
      expect(useIncrementalStore.getState().incrementalMode).toBe(false);

      useIncrementalStore.getState().toggleIncrementalMode();
      expect(useIncrementalStore.getState().incrementalMode).toBe(true);

      useIncrementalStore.getState().toggleIncrementalMode();
      expect(useIncrementalStore.getState().incrementalMode).toBe(false);
    });
  });

  describe('historyStore', () => {
    beforeEach(() => {
      useTranslateStore.setState({
        translationHistory: [
          {
            sourceText: 'First',
            results: [{ engine: 'google', text: '第一个' }],
            fromLang: 'en',
            toLang: 'zh',
          },
          {
            sourceText: 'Second',
            results: [{ engine: 'google', text: '第二个' }],
            fromLang: 'en',
            toLang: 'zh',
          },
          {
            sourceText: 'Third',
            results: [{ engine: 'google', text: '第三个' }],
            fromLang: 'en',
            toLang: 'zh',
          },
        ],
        historyIndex: 2,
      } as unknown);
    });

    it('should go to previous translation', () => {
      useTranslateStore.getState().goToPreviousTranslation();

      const state = useTranslateStore.getState();
      expect(state.historyIndex).toBe(1);
      expect(state.sourceText).toBe('Second');
      expect(state.results).toEqual([{ engine: 'google', text: '第二个' }]);
    });

    it('should not go before first translation', () => {
      useTranslateStore.setState({ historyIndex: 0 } as unknown);

      useTranslateStore.getState().goToPreviousTranslation();

      expect(useTranslateStore.getState().historyIndex).toBe(0);
    });

    it('should go to next translation', () => {
      useTranslateStore.setState({ historyIndex: 1 } as unknown);

      useTranslateStore.getState().goToNextTranslation();

      const state = useTranslateStore.getState();
      expect(state.historyIndex).toBe(2);
      expect(state.sourceText).toBe('Third');
    });

    it('should not go past last translation', () => {
      useTranslateStore.setState({ historyIndex: 2 } as unknown);

      useTranslateStore.getState().goToNextTranslation();

      expect(useTranslateStore.getState().historyIndex).toBe(2);
    });

    it('should do nothing with empty history', () => {
      useTranslateStore.setState({
        translationHistory: [],
        historyIndex: -1,
      } as unknown);

      useTranslateStore.getState().goToPreviousTranslation();
      useTranslateStore.getState().goToNextTranslation();

      expect(useTranslateStore.getState().historyIndex).toBe(-1);
    });
  });

  describe('embeddedStore', () => {
    it('should clear embedded lines for empty text', async () => {
      useTranslateStore.setState({ sourceText: '' });
      useEmbeddedStore.setState({
        embeddedLines: [{ lineNumber: 1, original: 'old', translated: '旧' }],
      });

      await useEmbeddedStore.getState().translateEmbedded();

      expect(useEmbeddedStore.getState().embeddedLines).toEqual([]);
    });

    it('should call safeInvoke with correct parameters', async () => {
      useTranslateStore.setState({
        sourceText: 'Hello\nWorld',
        fromLang: 'en',
        toLang: 'zh',
      });

      vi.mocked(safeInvoke).mockResolvedValue([
        [
          { lineNumber: 1, original: 'Hello', translated: '你好' },
          { lineNumber: 2, original: 'World', translated: '世界' },
        ],
        null,
      ]);

      await useEmbeddedStore.getState().translateEmbedded();

      expect(safeInvoke).toHaveBeenCalledWith('translate_embedded', {
        text: 'Hello\nWorld',
        from: 'en',
        to: 'zh',
      });
    });

    it('should update embedded lines on success', async () => {
      useTranslateStore.setState({ sourceText: 'Hello' });

      const mockResults = [{ lineNumber: 1, original: 'Hello', translated: '你好' }];
      vi.mocked(safeInvoke).mockResolvedValue([mockResults, null]);

      await useEmbeddedStore.getState().translateEmbedded();

      expect(useEmbeddedStore.getState().embeddedLines).toEqual(mockResults);
      expect(useTranslateStore.getState().loading).toBe(false);
    });

    it('should handle error', async () => {
      useTranslateStore.setState({ sourceText: 'Hello' });

      vi.mocked(safeInvoke).mockResolvedValue([null, { code: 'ERR', message: 'Embedded failed' }]);

      await useEmbeddedStore.getState().translateEmbedded();

      expect(useTranslateStore.getState().error).toBe('Embedded failed');
      expect(useTranslateStore.getState().loading).toBe(false);
    });

    it('should toggle embedded mode', () => {
      expect(useEmbeddedStore.getState().embeddedMode).toBe(false);

      useEmbeddedStore.getState().toggleEmbeddedMode();
      expect(useEmbeddedStore.getState().embeddedMode).toBe(true);

      useEmbeddedStore.getState().toggleEmbeddedMode();
      expect(useEmbeddedStore.getState().embeddedMode).toBe(false);
    });

    it('should trigger translate when switching to embedded mode with text', async () => {
      useTranslateStore.setState({ sourceText: 'Hello' });

      vi.mocked(safeInvoke).mockResolvedValue([
        [{ lineNumber: 1, original: 'Hello', translated: '你好' }],
        null,
      ]);

      useEmbeddedStore.getState().toggleEmbeddedMode();

      // Wait for async operation
      await vi.waitFor(() => {
        expect(safeInvoke).toHaveBeenCalledWith('translate_embedded', expect.any(Object));
      });
    });
  });

  describe('lookupDictionary', () => {
    it('should clear dictionary for empty text', async () => {
      useTranslateStore.setState({
        sourceText: '',
        dictionaryResults: [{ word: 'old', meanings: [], sourceUrls: [] }],
      });

      await useTranslateStore.getState().lookupDictionary();

      expect(useTranslateStore.getState().dictionaryResults).toEqual([]);
    });

    it('should update dictionary results on success', async () => {
      useTranslateStore.setState({ sourceText: 'hello' });

      const mockResults = [
        {
          word: 'hello',
          phonetic: '/həˈloʊ/',
          meanings: [
            {
              partOfSpeech: 'noun',
              definitions: [{ definition: 'A greeting', synonyms: [], antonyms: [] }],
            },
          ],
          sourceUrls: ['https://dictionary.com'],
        },
      ];
      vi.mocked(safeInvoke).mockResolvedValue([mockResults, null]);

      await useTranslateStore.getState().lookupDictionary();

      expect(useTranslateStore.getState().dictionaryResults).toEqual(mockResults);
    });

    it('should handle lookup error silently', async () => {
      useTranslateStore.setState({ sourceText: 'hello' });

      vi.mocked(safeInvoke).mockResolvedValue([null, { code: 'ERR', message: 'Not found' }]);

      await useTranslateStore.getState().lookupDictionary();

      expect(useTranslateStore.getState().dictionaryResults).toEqual([]);
    });
  });

  describe('backTranslate', () => {
    it('should clear back translation for empty text', async () => {
      useTranslateStore.setState({ backTranslation: 'old' });

      await useTranslateStore.getState().backTranslate('');

      expect(useTranslateStore.getState().backTranslation).toBeNull();
    });

    it('should update back translation on success', async () => {
      vi.mocked(safeInvoke).mockResolvedValue(['Hello back', null]);

      await useTranslateStore.getState().backTranslate('你好');

      expect(useTranslateStore.getState().backTranslation).toBe('Hello back');
    });

    it('should handle error silently', async () => {
      vi.mocked(safeInvoke).mockResolvedValue([null, { code: 'ERR', message: 'Failed' }]);

      await useTranslateStore.getState().backTranslate('你好');

      expect(useTranslateStore.getState().backTranslation).toBeNull();
    });
  });

  describe('detectLanguage', () => {
    it('should clear detected lang for empty text', async () => {
      useTranslateStore.setState({ detectedLang: 'en' });

      await useTranslateStore.getState().detectLanguage('');

      expect(useTranslateStore.getState().detectedLang).toBe('');
    });

    it('should update detected language on success', async () => {
      vi.mocked(safeInvoke).mockResolvedValue([
        { language: 'en', confidence: 0.95, name: 'English' },
        null,
      ]);

      await useTranslateStore.getState().detectLanguage('Hello');

      expect(useTranslateStore.getState().detectedLang).toBe('English');
    });

    it('should clear detected lang for auto detection', async () => {
      vi.mocked(safeInvoke).mockResolvedValue([
        { language: 'auto', confidence: 0, name: 'Auto' },
        null,
      ]);

      await useTranslateStore.getState().detectLanguage('Hello');

      expect(useTranslateStore.getState().detectedLang).toBe('');
    });

    it('should handle error silently', async () => {
      vi.mocked(safeInvoke).mockResolvedValue([null, { code: 'ERR', message: 'Failed' }]);

      await useTranslateStore.getState().detectLanguage('Hello');

      expect(useTranslateStore.getState().detectedLang).toBe('');
    });
  });

  describe('polishTranslation', () => {
    it('should not polish without source text', async () => {
      useTranslateStore.setState({
        sourceText: '',
        results: [{ engine: 'google', text: '你好' }],
      });

      await useTranslateStore.getState().polishTranslation();

      expect(safeInvoke).not.toHaveBeenCalled();
    });

    it('should not polish without results', async () => {
      useTranslateStore.setState({
        sourceText: 'Hello',
        results: [],
      });

      await useTranslateStore.getState().polishTranslation();

      expect(safeInvoke).not.toHaveBeenCalled();
    });

    it('should update first result with polished text', async () => {
      useTranslateStore.setState({
        sourceText: 'Hello',
        results: [{ engine: 'google', text: '你好' }],
        fromLang: 'en',
        toLang: 'zh',
      });

      vi.mocked(safeInvoke).mockResolvedValue(['您好', null]);

      await useTranslateStore.getState().polishTranslation();

      expect(useTranslateStore.getState().results[0].text).toBe('您好');
      expect(useTranslateStore.getState().polishing).toBe(false);
    });

    it('should handle polish error', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      useTranslateStore.setState({
        sourceText: 'Hello',
        results: [{ engine: 'google', text: '你好' }],
      });

      vi.mocked(safeInvoke).mockResolvedValue([null, { code: 'ERR', message: 'Failed' }]);

      await useTranslateStore.getState().polishTranslation();

      expect(useTranslateStore.getState().results[0].text).toBe('你好');
      expect(useTranslateStore.getState().polishing).toBe(false);
      consoleSpy.mockRestore();
    });
  });
});
