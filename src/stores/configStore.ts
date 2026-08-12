import { create } from 'zustand';
import { safeInvoke, invokeOrDefault } from '../services/invoke';
import { useI18n } from '../i18n';
import type { AppConfig } from '../types';

/**
 * Minimal initial config — only used before the Rust backend returns
 * the authoritative defaults via `get_default_config`.
 * All values here are safe placeholders; they will be replaced on startup.
 */
const INITIAL_CONFIG: AppConfig = {
  llm: { provider: 'deepseek', apiKey: '', apiKeys: [], baseUrl: '', model: '', providers: [] },
  engines: {
    google: { enabled: false },
    baidu: { enabled: false, appId: '', secret: '' },
    youdao: { enabled: false, useAi: false, ocrAppKey: '', ocrAppSecret: '' },
    deepl: { enabled: false, apiKey: '', pro: false },
    deeplx: { enabled: false, pro: false },
    microsoft: { enabled: false },
    yandex: { enabled: false },
    offline: { enabled: false, autoSwitch: true, downloadedModels: [], modelDir: '' },
    tatoeba: { enabled: false },
    baiduWeb: { enabled: false },
    caiyun: { enabled: false, apiToken: '' },
    caiyunWeb: { enabled: false },
    volcengineWeb: { enabled: false },
    transmart: { enabled: false },
    papago: { enabled: false },
  },
  defaultFrom: 'auto',
  defaultTo: 'zh',
  customPrompt: '',
  promptTemplates: [],
  llmTemperature: 0.3,
  llmMaxTokens: 4096,
  clipboardMonitor: false,
  useClipboardOutput: true,
  autoCopyResult: false,
  autoCopyMode: 'translated',
  translationMask: false,
  apiServerEnabled: false,
  apiServerPort: 60828,
  apiServerToken: '',
  // Placeholders only — replaced by get_config / get_default_config at startup.
  hotkeys: {
    ocrTranslate: '',
    showWindow: '',
    translateSelection: '',
    replaceTranslate: '',
    toggleOverlayClickThrough: '',
    dictionaryLookup: '',
  },
  selectionUx: {
    triggerMode: 'auto_on_select',
    hoverDictionary: false,
    hoverCjk: false,
    hoverDwellMs: 400,
    hoverUnit: 'word',
    hoverDictSource: 'auto',
    ocrForcePickup: false,
    ocrModifierKey: '',
    autoMinChars: 1,
    minDragPx: 10,
    excludeProcesses: [],
  },
  proxy: { enabled: false, proxyType: 'http', host: '', port: 7890, username: '', password: '' },
  windowFollowMode: 'none',
  translationBlacklist: [],
  routingStrategy: null,
  ocrEngine: 'winrt',
  offlineOcr: { backend: 'rapid', pluginDir: '' },
  pdfExtractionEngine: 'pdf-extract',
  pdfExtractionSidecar: { mineruCmd: '', markerCmd: '', ocrmypdfCmd: '' },
  /** Screenshot OCR continuous refresh (ms). Distinct from hook.ocrIntervalMs. */
  ocrInterval: 2000,
  overlayLevel: 2,
  overlayAutoDismissMs: 3000,
  overlayFollowMode: 'none',
  hook: {
    enabledSources: ['uia', 'clipboard'],
    showOverlay: true,
    autoCopy: false,
    enabled: true,
    uiaIntervalMs: 500,
    ocrIntervalMs: 5000,
  },
  tmEnabled: false,
  tmThreshold: 0.8,
  cacheTtlHours: 72,
  furiganaEnabled: false,
  ttsAutoPlay: false,
  ttsVoice: '',
  ttsProvider: 'edge',
  batchPreferredEngine: '',
  openaiTts: {
    apiKey: '',
    baseUrl: 'https://api.openai.com/v1',
    model: 'tts-1',
    voice: 'alloy',
    speed: 1,
  },
  fishTts: {
    apiKey: '',
    model: 's2.1-pro-free',
    referenceId: '12b8a0bf8e0042c3b11e519d11db8b68',
    format: 'mp3',
    speed: 1,
  },
  httpTimeoutSecs: 30,
  ocrTimeoutSecs: 30,
  llmTimeoutSecs: 120,
  translationTimeoutSecs: 30,
  edgeTtsToken: '',
  sync: {
    enabled: false,
    serverUrl: '',
    username: '',
    password: '',
    remoteDir: 'moontranslator',
    intervalMins: 30,
    syncConfig: true,
    syncGlossary: true,
    syncHistory: true,
    syncWordbook: true,
    lastSyncAt: 0,
    lastSyncStatus: '',
  },
  collection: {
    eudic: { enabled: false, token: '', bookName: 'Moon' },
    anki: { enabled: false, port: 8765, deck: 'Moon', model: 'Moon Card' },
    shanbay: { enabled: false, credential: '', wordbookId: '' },
    youdao: { enabled: false, cookie: '', lan: 'en' },
    maimemo: { enabled: false, token: '', notepadId: '', notepadTitle: 'Moon' },
    autoPushOnSave: true,
  },
  layoutDetectionEnabled: false,
  /** Hidden WebView preload budget (0-3, default 1). Matches Rust default. */
  hotLoadPageCount: 1,
  /** Defer screenshot-warmup + OCR hot-start until first use. */
  deferStartupWarmup: true,
};

interface ConfigState {
  config: AppConfig;
  loaded: boolean;
  saved: boolean;
  cacheSize: number;

  loadDefaults: () => Promise<void>;
  loadConfig: () => Promise<void>;
  saveConfig: () => Promise<void>;
  updateConfig: (updater: (prev: AppConfig) => AppConfig) => void;
  updateLlm: (field: keyof AppConfig['llm'], value: string) => void;
  loadCacheSize: () => Promise<void>;
  clearCache: () => Promise<void>;
}

export const useConfigStore = create<ConfigState>((set, get) => ({
  config: INITIAL_CONFIG,
  loaded: false,
  saved: false,
  cacheSize: 0,

  /**
   * Fetch authoritative defaults from Rust backend.
   * Prefer loadConfig() at startup; this is for reset-to-defaults UX.
   */
  loadDefaults: async () => {
    const [defaults, error] = await safeInvoke<AppConfig>('get_default_config');
    if (error || !defaults) {
      console.error('Failed to load default config:', error);
      return;
    }
    const { loaded } = get();
    if (!loaded) {
      set({ config: defaults });
    }
  },

  /**
   * Load saved config from Rust backend (disk + defaults).
   * Must run once at MainApp mount before any saveConfig.
   */
  loadConfig: async () => {
    const [loadedCfg, error] = await safeInvoke<AppConfig>('get_config');
    if (error || !loadedCfg) {
      console.error('Failed to load config:', error);
      // Fall back to compile-time defaults so UI can still open; mark loaded
      // so we do not block forever — but refuse save until a successful load.
      const [defaults] = await safeInvoke<AppConfig>('get_default_config');
      if (defaults) {
        set({ config: defaults, loaded: true });
      } else {
        set({ loaded: true });
      }
      return;
    }
    // Merge with INITIAL so missing nested fields never crash settings UI (.length on undefined)
    const merged: AppConfig = {
      ...INITIAL_CONFIG,
      ...loadedCfg,
      llm: { ...INITIAL_CONFIG.llm, ...(loadedCfg.llm ?? {}) },
      engines: {
        ...INITIAL_CONFIG.engines,
        ...(loadedCfg.engines ?? {}),
        caiyun: {
          ...INITIAL_CONFIG.engines.caiyun,
          ...loadedCfg.engines?.caiyun,
        },
      } as AppConfig['engines'],
      hotkeys: { ...INITIAL_CONFIG.hotkeys, ...(loadedCfg.hotkeys ?? {}) },
      selectionUx: {
        ...INITIAL_CONFIG.selectionUx!,
        ...(loadedCfg.selectionUx ?? {}),
      } as AppConfig['selectionUx'],
      proxy: { ...INITIAL_CONFIG.proxy, ...(loadedCfg.proxy ?? {}) },
      hook: { ...INITIAL_CONFIG.hook!, ...(loadedCfg.hook ?? {}) },
      sync: { ...INITIAL_CONFIG.sync!, ...(loadedCfg.sync ?? {}) },
      collection: {
        ...INITIAL_CONFIG.collection!,
        ...(loadedCfg.collection ?? {}),
        eudic: {
          ...INITIAL_CONFIG.collection!.eudic,
          ...(loadedCfg.collection?.eudic ?? {}),
        },
        anki: {
          ...INITIAL_CONFIG.collection!.anki,
          ...(loadedCfg.collection?.anki ?? {}),
        },
        shanbay: {
          ...INITIAL_CONFIG.collection!.shanbay,
          ...(loadedCfg.collection?.shanbay ?? {}),
        },
        youdao: {
          ...INITIAL_CONFIG.collection!.youdao,
          ...(loadedCfg.collection?.youdao ?? {}),
        },
        maimemo: {
          ...INITIAL_CONFIG.collection!.maimemo,
          ...(loadedCfg.collection?.maimemo ?? {}),
        },
      },
      offlineOcr: {
        backend: loadedCfg.offlineOcr?.backend ?? INITIAL_CONFIG.offlineOcr?.backend ?? 'rapid',
        pluginDir: loadedCfg.offlineOcr?.pluginDir ?? INITIAL_CONFIG.offlineOcr?.pluginDir ?? '',
      },
      pdfExtractionEngine:
        loadedCfg.pdfExtractionEngine ?? INITIAL_CONFIG.pdfExtractionEngine ?? 'pdf-extract',
      pdfExtractionSidecar: {
        mineruCmd:
          loadedCfg.pdfExtractionSidecar?.mineruCmd ??
          INITIAL_CONFIG.pdfExtractionSidecar?.mineruCmd ??
          '',
        markerCmd:
          loadedCfg.pdfExtractionSidecar?.markerCmd ??
          INITIAL_CONFIG.pdfExtractionSidecar?.markerCmd ??
          '',
        ocrmypdfCmd:
          loadedCfg.pdfExtractionSidecar?.ocrmypdfCmd ??
          INITIAL_CONFIG.pdfExtractionSidecar?.ocrmypdfCmd ??
          '',
      },
      openaiTts: {
        apiKey: loadedCfg.openaiTts?.apiKey ?? INITIAL_CONFIG.openaiTts?.apiKey ?? '',
        baseUrl:
          loadedCfg.openaiTts?.baseUrl ??
          INITIAL_CONFIG.openaiTts?.baseUrl ??
          'https://api.openai.com/v1',
        model: loadedCfg.openaiTts?.model ?? INITIAL_CONFIG.openaiTts?.model ?? 'tts-1',
        voice: loadedCfg.openaiTts?.voice ?? INITIAL_CONFIG.openaiTts?.voice ?? 'alloy',
        speed: loadedCfg.openaiTts?.speed ?? INITIAL_CONFIG.openaiTts?.speed ?? 1,
      },
      fishTts: {
        apiKey: loadedCfg.fishTts?.apiKey ?? INITIAL_CONFIG.fishTts?.apiKey ?? '',
        model: loadedCfg.fishTts?.model ?? INITIAL_CONFIG.fishTts?.model ?? 's2.1-pro-free',
        referenceId:
          loadedCfg.fishTts?.referenceId ??
          INITIAL_CONFIG.fishTts?.referenceId ??
          '12b8a0bf8e0042c3b11e519d11db8b68',
        format: loadedCfg.fishTts?.format ?? INITIAL_CONFIG.fishTts?.format ?? 'mp3',
        speed: loadedCfg.fishTts?.speed ?? INITIAL_CONFIG.fishTts?.speed ?? 1,
      },
      layoutDetectionEnabled:
        loadedCfg.layoutDetectionEnabled ?? INITIAL_CONFIG.layoutDetectionEnabled ?? false,
    };
    set({ config: merged, loaded: true });
  },

  saveConfig: async () => {
    const { config, loaded } = get();
    if (!loaded) {
      console.error('Refusing saveConfig: config not loaded yet (would clobber disk)');
      return;
    }
    const prev = structuredClone(config);
    const [, error] = await safeInvoke('save_config', { config });
    if (error) {
      console.error('Failed to save config:', error);
      set({ config: prev, saved: false });
      const { useToastStore } = await import('./toastStore');
      useToastStore.getState().addToast({
        type: 'error',
        message: useI18n.getState().t('settings.configSaveFailed', { message: error.message }),
        duration: 4000,
      });
      return;
    }
    set({ saved: true });
    setTimeout(() => set({ saved: false }), 2000);
  },

  updateConfig: (updater) => {
    set((state) => ({ config: updater(state.config) }));
  },

  updateLlm: (field, value) => {
    set((state) => ({
      config: {
        ...state.config,
        llm: { ...state.config.llm, [field]: value },
      },
    }));
  },

  loadCacheSize: async () => {
    const size = await invokeOrDefault<number>('cache_size', undefined, 0);
    set({ cacheSize: size });
  },

  clearCache: async () => {
    const [, error] = await safeInvoke('clear_cache');
    if (error) {
      console.error('Failed to clear cache:', error);
      return;
    }
    set({ cacheSize: 0 });
  },
}));
