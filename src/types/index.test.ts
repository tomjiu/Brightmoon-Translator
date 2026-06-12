import { describe, it, expect } from 'vitest';
import {
  LANGUAGES,
  VARIABLE_FORMATS,
  type TranslationResult,
  type TranslateResponse,
  type HistoryItem,
  type AppConfig,
  type DetectionResult,
  type TextRegion,
  type EmbeddedLine,
  type DictionaryResult,
  type BatchTaskStatus,
  type BatchJobStatus,
  type BatchConfig,
  type BatchTask,
  type BatchProgress,
  type TmExportEntry,
  type TmExportData,
  type TmStats,
} from './index';

describe('Type definitions', () => {
  describe('TranslationResult', () => {
    it('should accept valid translation result', () => {
      const result: TranslationResult = {
        engine: 'google',
        text: '你好',
      };

      expect(result.engine).toBe('google');
      expect(result.text).toBe('你好');
    });

    it('should accept optional latencyMs', () => {
      const result: TranslationResult = {
        engine: 'deepl',
        text: 'Hello',
        latencyMs: 150,
      };

      expect(result.latencyMs).toBe(150);
    });
  });

  describe('TranslateResponse', () => {
    it('should accept valid translate response', () => {
      const response: TranslateResponse = {
        results: [
          { engine: 'google', text: '你好' },
          { engine: 'deepl', text: '您好' },
        ],
        detectedLanguage: 'en',
      };

      expect(response.results).toHaveLength(2);
      expect(response.detectedLanguage).toBe('en');
    });

    it('should accept response without detectedLanguage', () => {
      const response: TranslateResponse = {
        results: [{ engine: 'google', text: '你好' }],
      };

      expect(response.detectedLanguage).toBeUndefined();
    });
  });

  describe('HistoryItem', () => {
    it('should accept valid history item', () => {
      const item: HistoryItem = {
        id: '123',
        sourceText: 'Hello',
        translatedText: '你好',
        from: 'en',
        to: 'zh',
        engine: 'google',
        timestamp: Date.now(),
      };

      expect(item.id).toBe('123');
      expect(item.sourceText).toBe('Hello');
    });
  });

  describe('AppConfig', () => {
    it('should accept valid app config', () => {
      const config: AppConfig = {
        llm: {
          provider: 'deepseek',
          apiKey: 'test-key',
          apiKeys: [],
          baseUrl: 'https://api.deepseek.com',
          model: 'deepseek-chat',
        },
        engines: {
          google: { enabled: true },
          baidu: { enabled: false, appId: '', secret: '' },
          youdao: { enabled: false, useAi: false },
          deepl: { enabled: false, apiKey: '', pro: false },
          deeplx: { enabled: false, pro: false },
          microsoft: { enabled: false },
          yandex: { enabled: false },
        },
        defaultFrom: 'auto',
        defaultTo: 'zh',
        customPrompt: '',
        promptTemplates: [],
        clipboardMonitor: false,
        autoCopyResult: false,
        autoCopyMode: 'translated',
        translationMask: false,
        apiServerEnabled: false,
        apiServerPort: 60828,
        hotkeys: {
          ocrTranslate: 'Alt+Q',
          showWindow: 'Alt+W',
          translateSelection: 'Alt+E',
        },
        proxy: {
          enabled: false,
          proxyType: 'http',
          host: '',
          port: 7890,
          username: '',
          password: '',
        },
        windowFollowMode: 'none',
        translationBlacklist: [],
      };

      expect(config.llm.provider).toBe('deepseek');
      expect(config.engines.google.enabled).toBe(true);
      expect(config.hotkeys.ocrTranslate).toBe('Alt+Q');
    });
  });

  describe('DetectionResult', () => {
    it('should accept valid detection result', () => {
      const result: DetectionResult = {
        language: 'en',
        confidence: 0.95,
        name: 'English',
      };

      expect(result.language).toBe('en');
      expect(result.confidence).toBe(0.95);
      expect(result.name).toBe('English');
    });
  });

  describe('TextRegion', () => {
    it('should accept valid text region', () => {
      const region: TextRegion = {
        x: 100,
        y: 200,
        width: 300,
        height: 50,
        lineCount: 3,
        textPreview: 'Hello World...',
      };

      expect(region.x).toBe(100);
      expect(region.lineCount).toBe(3);
    });
  });

  describe('EmbeddedLine', () => {
    it('should accept valid embedded line', () => {
      const line: EmbeddedLine = {
        lineNumber: 1,
        original: 'Hello',
        translated: '你好',
      };

      expect(line.lineNumber).toBe(1);
      expect(line.original).toBe('Hello');
      expect(line.translated).toBe('你好');
    });
  });

  describe('DictionaryResult', () => {
    it('should accept valid dictionary result', () => {
      const result: DictionaryResult = {
        word: 'hello',
        phonetic: '/həˈloʊ/',
        meanings: [
          {
            partOfSpeech: 'noun',
            definitions: [
              {
                definition: 'A greeting',
                example: 'Hello, how are you?',
                synonyms: ['greeting', 'hi'],
                antonyms: [],
              },
            ],
          },
        ],
        sourceUrls: ['https://dictionary.com'],
      };

      expect(result.word).toBe('hello');
      expect(result.meanings).toHaveLength(1);
      expect(result.meanings[0].definitions[0].synonyms).toContain('hi');
    });

    it('should accept result without phonetic', () => {
      const result: DictionaryResult = {
        word: 'test',
        meanings: [],
        sourceUrls: [],
      };

      expect(result.phonetic).toBeUndefined();
    });
  });

  describe('Batch types', () => {
    it('should accept valid BatchConfig', () => {
      const config: BatchConfig = {
        concurrency: 3,
        fromLang: 'en',
        toLang: 'zh',
        engine: 'google',
        continueOnError: true,
      };

      expect(config.concurrency).toBe(3);
    });

    it('should accept valid BatchTask', () => {
      const task: BatchTask = {
        id: 'task-1',
        index: 0,
        text: 'Hello',
        fromLang: 'en',
        toLang: 'zh',
        status: 'completed',
        result: '你好',
      };

      expect(task.status).toBe('completed');
      expect(task.result).toBe('你好');
    });

    it('should accept valid BatchProgress', () => {
      const progress: BatchProgress = {
        jobId: 'job-1',
        total: 10,
        completed: 5,
        failed: 1,
        currentIndex: 5,
        status: 'running',
      };

      expect(progress.total).toBe(10);
      expect(progress.completed).toBe(5);
    });

    it('should accept all BatchTaskStatus values', () => {
      const statuses: BatchTaskStatus[] = [
        'pending',
        'running',
        'completed',
        'failed',
        'cancelled',
      ];

      statuses.forEach((status) => {
        const task: BatchTask = {
          id: '1',
          index: 0,
          text: '',
          fromLang: '',
          toLang: '',
          status,
        };
        expect(task.status).toBe(status);
      });
    });

    it('should accept all BatchJobStatus values', () => {
      const statuses: BatchJobStatus[] = [
        'idle',
        'running',
        'paused',
        'completed',
        'cancelled',
        'failed',
      ];

      statuses.forEach((status) => {
        const progress: BatchProgress = {
          jobId: '1',
          total: 0,
          completed: 0,
          failed: 0,
          status,
        };
        expect(progress.status).toBe(status);
      });
    });
  });

  describe('TM types', () => {
    it('should accept valid TmExportEntry', () => {
      const entry: TmExportEntry = {
        source: 'Hello',
        target: '你好',
        fromLang: 'en',
        toLang: 'zh',
        engine: 'google',
        timestamp: Date.now(),
      };

      expect(entry.source).toBe('Hello');
    });

    it('should accept valid TmExportData', () => {
      const data: TmExportData = {
        version: 1,
        entries: [],
        exportedAt: Date.now(),
      };

      expect(data.version).toBe(1);
    });

    it('should accept valid TmStats', () => {
      const stats: TmStats = {
        total: 100,
        langPairs: [
          ['en', 'zh', 50],
          ['ja', 'zh', 30],
          ['en', 'ja', 20],
        ],
      };

      expect(stats.total).toBe(100);
      expect(stats.langPairs).toHaveLength(3);
    });
  });
});

describe('Constants', () => {
  describe('LANGUAGES', () => {
    it('should contain expected languages', () => {
      expect(LANGUAGES).toBeDefined();
      expect(LANGUAGES.length).toBeGreaterThan(0);
    });

    it('should have auto as first language', () => {
      expect(LANGUAGES[0].code).toBe('auto');
      expect(LANGUAGES[0].name).toBe('自动检测');
    });

    it('should contain common languages', () => {
      const codes = LANGUAGES.map((l) => l.code);
      expect(codes).toContain('zh');
      expect(codes).toContain('en');
      expect(codes).toContain('ja');
      expect(codes).toContain('ko');
      expect(codes).toContain('fr');
      expect(codes).toContain('de');
      expect(codes).toContain('es');
      expect(codes).toContain('ru');
    });

    it('should have code and name for each language', () => {
      LANGUAGES.forEach((lang) => {
        expect(lang.code).toBeTruthy();
        expect(lang.name).toBeTruthy();
      });
    });
  });

  describe('VARIABLE_FORMATS', () => {
    it('should contain expected formats', () => {
      expect(VARIABLE_FORMATS).toBeDefined();
      expect(VARIABLE_FORMATS).toContain('snake_case');
      expect(VARIABLE_FORMATS).toContain('SNAKE_CASE');
      expect(VARIABLE_FORMATS).toContain('kebab-case');
      expect(VARIABLE_FORMATS).toContain('camelCase');
      expect(VARIABLE_FORMATS).toContain('PascalCase');
      expect(VARIABLE_FORMATS).toContain('dot.notation');
      expect(VARIABLE_FORMATS).toContain('Title Case');
    });

    it('should have 7 formats', () => {
      expect(VARIABLE_FORMATS).toHaveLength(7);
    });
  });
});
