import { create } from 'zustand';
import { safeInvoke, invokeOrDefault } from '../services/invoke';
import type { AppConfig } from '../types';

/**
 * Minimal initial config — only used before the Rust backend returns
 * the authoritative defaults via `get_default_config`.
 * All values here are safe placeholders; they will be replaced on startup.
 */
const INITIAL_CONFIG: AppConfig = {
  llm: { provider: 'deepseek', apiKey: '', apiKeys: [], baseUrl: '', model: '' },
  engines: {
    google: { enabled: false },
    baidu: { enabled: false, appId: '', secret: '' },
    youdao: { enabled: false, useAi: false, ocrAppKey: '', ocrAppSecret: '' },
    deepl: { enabled: false, apiKey: '', pro: false },
    deeplx: { enabled: false, pro: false },
    microsoft: { enabled: false },
    yandex: { enabled: false },
    offline: { enabled: false, autoSwitch: true, downloadedModels: [], modelDir: '' },
  },
  defaultFrom: 'auto',
  defaultTo: 'zh',
  customPrompt: '',
  promptTemplates: [],
  llmTemperature: 0.3,
  llmMaxTokens: 4096,
  clipboardMonitor: false,
  autoCopyResult: false,
  autoCopyMode: 'translated',
  translationMask: false,
  apiServerEnabled: false,
  apiServerPort: 60828,
  hotkeys: { ocrTranslate: '', showWindow: '', translateSelection: '' },
  proxy: { enabled: false, proxyType: 'http', host: '', port: 7890, username: '', password: '' },
  windowFollowMode: 'none',
  translationBlacklist: [],
  routingStrategy: 'fallback_on_error',
  ocrEngine: 'auto',
  overlayLevel: 2,
  overlayAutoDismissMs: 3000,
  overlayFollowMode: 'none',
  hook: {
    enabledSources: [],
    showOverlay: true,
    autoCopy: false,
    enabled: true,
    uiaIntervalMs: 500,
    ocrIntervalMs: 5000,
  },
  tmEnabled: false,
  tmThreshold: 0.8,
  furiganaEnabled: false,
  ttsAutoPlay: false,
  ttsVoice: '',
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
};

interface EngineCacheStats {
  engine: string;
  entries: number;
  hits: number;
}

interface CacheStats {
  total_entries: number;
  memory_entries: number;
  memory_capacity: number;
  disk_entries: number;
  hit_rate: number;
  total_hits: number;
  total_misses: number;
  engine_stats: EngineCacheStats[];
}

interface ConfigState {
  config: AppConfig;
  loaded: boolean;
  saved: boolean;
  cacheSize: number;
  cacheStats: CacheStats | null;

  loadDefaults: () => Promise<void>;
  loadConfig: () => Promise<void>;
  saveConfig: () => Promise<void>;
  updateConfig: (updater: (prev: AppConfig) => AppConfig) => void;
  updateLlm: (field: keyof AppConfig['llm'], value: string) => void;
  loadCacheSize: () => Promise<void>;
  clearCache: () => Promise<void>;
  loadCacheStats: () => Promise<void>;
}

export const useConfigStore = create<ConfigState>((set, get) => ({
  config: INITIAL_CONFIG,
  loaded: false,
  saved: false,
  cacheSize: 0,
  cacheStats: null,

  /**
   * Fetch authoritative defaults from Rust backend.
   * Call once at app startup to replace INITIAL_CONFIG.
   */
  loadDefaults: async () => {
    const [defaults, error] = await safeInvoke<AppConfig>('get_default_config');
    if (error || !defaults) {
      console.error('Failed to load default config:', error);
      return;
    }
    // Only apply defaults if config hasn't been loaded yet
    const { loaded } = get();
    if (!loaded) {
      set({ config: defaults });
    }
  },

  /**
   * Load saved config from Rust backend.
   * Rust serde already fills missing fields with defaults,
   * so no deep merge is needed — the returned config is complete.
   */
  loadConfig: async () => {
    const [loaded, error] = await safeInvoke<AppConfig>('get_config');
    if (error || !loaded) {
      console.error('Failed to load config:', error);
      set({ loaded: true });
      return;
    }
    set({ config: loaded, loaded: true });
  },

  saveConfig: async () => {
    const { config } = get();
    const [, error] = await safeInvoke('save_config', { config });
    if (error) {
      console.error('Failed to save config:', error);
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

  loadCacheStats: async () => {
    const [stats, error] = await safeInvoke<CacheStats>('cache_stats');
    if (error || !stats) {
      console.error('Failed to load cache stats:', error);
      return;
    }
    set({ cacheStats: stats });
  },
}));
