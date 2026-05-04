import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type { AppConfig } from "../types";

const defaultConfig: AppConfig = {
  llm: {
    provider: "deepseek",
    apiKey: "",
    apiKeys: [],
    baseUrl: "https://api.deepseek.com/v1",
    model: "deepseek-chat",
  },
  engines: {
    google: { enabled: true },
    baidu: { enabled: false, appId: "", secret: "" },
    youdao: { enabled: false, useAi: false },
    deepl: { enabled: false, apiKey: "", pro: false },
    deeplx: { enabled: false, apiKey: "", pro: false },
    microsoft: { enabled: false },
    yandex: { enabled: false },
  },
  defaultFrom: "auto",
  defaultTo: "zh",
  customPrompt: "",
  promptTemplates: [],
  clipboardMonitor: false,
  autoCopyResult: false,
  autoCopyMode: "translated",
  translationMask: false,
  apiServerEnabled: false,
  apiServerPort: 60828,
  hotkeys: {
    ocrTranslate: "Ctrl+Shift+T",
    showWindow: "Ctrl+T",
    translateSelection: "Ctrl+Shift+Y",
  },
  proxy: {
    enabled: false,
    proxyType: "http",
    host: "",
    port: 7890,
    username: "",
    password: "",
  },
  windowFollowMode: "none",
  translationBlacklist: [],
};

interface ConfigState {
  config: AppConfig;
  loaded: boolean;
  saved: boolean;
  cacheSize: number;

  loadConfig: () => Promise<void>;
  saveConfig: () => Promise<void>;
  updateConfig: (updater: (prev: AppConfig) => AppConfig) => void;
  updateLlm: (field: string, value: string) => void;
  loadCacheSize: () => Promise<void>;
  clearCache: () => Promise<void>;
}

export const useConfigStore = create<ConfigState>((set, get) => ({
  config: defaultConfig,
  loaded: false,
  saved: false,
  cacheSize: 0,

  loadConfig: async () => {
    try {
      const config = await invoke<AppConfig>("get_config");
      set({ config, loaded: true });
    } catch (err) {
      console.error("Failed to load config:", err);
      set({ loaded: true });
    }
  },

  saveConfig: async () => {
    const { config } = get();
    try {
      await invoke("save_config", { config });
      set({ saved: true });
      setTimeout(() => set({ saved: false }), 2000);
    } catch (err) {
      console.error("Failed to save config:", err);
    }
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
    try {
      const size = await invoke<number>("cache_size");
      set({ cacheSize: size });
    } catch (err) {
      console.error("Failed to load cache size:", err);
    }
  },

  clearCache: async () => {
    try {
      await invoke("clear_cache");
      set({ cacheSize: 0 });
    } catch (err) {
      console.error("Failed to clear cache:", err);
    }
  },
}));
