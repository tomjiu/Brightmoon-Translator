import { describe, it, expect, vi, beforeEach } from "vitest";
import { useConfigStore } from "./configStore";
import { safeInvoke } from "../services/invoke";
import type { AppConfig } from "../types";

// Mock the invoke module
vi.mock("../services/invoke", () => ({
  safeInvoke: vi.fn(),
  invokeOrDefault: vi.fn(),
}));

describe("configStore", () => {
  beforeEach(() => {
    // Reset store to initial state
    useConfigStore.setState({
      config: {
        llm: {
          provider: "deepseek",
          apiKey: "",
          apiKeys: [],
          baseUrl: "",
          model: "",
        },
        engines: {
          google: { enabled: false },
          baidu: { enabled: false, appId: "", secret: "" },
          youdao: { enabled: false, useAi: false },
          deepl: { enabled: false, apiKey: "", pro: false },
          deeplx: { enabled: false, pro: false },
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
          ocrTranslate: "",
          showWindow: "",
          translateSelection: "",
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
        ttsVoice: "",
      },
      loaded: false,
      saved: false,
      cacheSize: 0,
    });

    vi.clearAllMocks();
  });

  describe("loadDefaults", () => {
    it("should load default config from backend", async () => {
      const mockDefaults: Partial<AppConfig> = {
        llm: { provider: "openai", apiKey: "", apiKeys: [], baseUrl: "", model: "gpt-4" },
        defaultFrom: "en",
        defaultTo: "zh",
      };

      vi.mocked(safeInvoke).mockResolvedValue([mockDefaults as AppConfig, null]);

      await useConfigStore.getState().loadDefaults();

      expect(safeInvoke).toHaveBeenCalledWith("get_default_config");
      expect(useConfigStore.getState().config.llm.provider).toBe("openai");
    });

    it("should not overwrite if already loaded", async () => {
      const mockDefaults: Partial<AppConfig> = {
        llm: { provider: "openai", apiKey: "", apiKeys: [], baseUrl: "", model: "gpt-4" },
      };

      useConfigStore.setState({ loaded: true });
      vi.mocked(safeInvoke).mockResolvedValue([mockDefaults as AppConfig, null]);

      await useConfigStore.getState().loadDefaults();

      expect(useConfigStore.getState().config.llm.provider).toBe("deepseek");
    });

    it("should handle error gracefully", async () => {
      const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
      vi.mocked(safeInvoke).mockResolvedValue([null, { code: "ERR", message: "Failed" }]);

      await useConfigStore.getState().loadDefaults();

      expect(consoleSpy).toHaveBeenCalled();
      consoleSpy.mockRestore();
    });
  });

  describe("loadConfig", () => {
    it("should load config from backend", async () => {
      const mockConfig: Partial<AppConfig> = {
        llm: { provider: "custom", apiKey: "test-key", apiKeys: [], baseUrl: "http://test.com", model: "test-model" },
        defaultFrom: "ja",
        defaultTo: "en",
      };

      vi.mocked(safeInvoke).mockResolvedValue([mockConfig as AppConfig, null]);

      await useConfigStore.getState().loadConfig();

      expect(safeInvoke).toHaveBeenCalledWith("get_config");
      expect(useConfigStore.getState().config.llm.provider).toBe("custom");
      expect(useConfigStore.getState().loaded).toBe(true);
    });

    it("should set loaded to true even on error", async () => {
      vi.mocked(safeInvoke).mockResolvedValue([null, { code: "ERR", message: "Failed" }]);

      await useConfigStore.getState().loadConfig();

      expect(useConfigStore.getState().loaded).toBe(true);
    });
  });

  describe("saveConfig", () => {
    it("should save current config to backend", async () => {
      vi.mocked(safeInvoke).mockResolvedValue([null, null]);

      await useConfigStore.getState().saveConfig();

      expect(safeInvoke).toHaveBeenCalledWith("save_config", {
        config: useConfigStore.getState().config,
      });
      expect(useConfigStore.getState().saved).toBe(true);
    });

    it("should reset saved flag after timeout", async () => {
      vi.useFakeTimers();
      vi.mocked(safeInvoke).mockResolvedValue([null, null]);

      await useConfigStore.getState().saveConfig();

      expect(useConfigStore.getState().saved).toBe(true);

      vi.advanceTimersByTime(2000);

      expect(useConfigStore.getState().saved).toBe(false);
      vi.useRealTimers();
    });

    it("should handle save error", async () => {
      const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
      vi.mocked(safeInvoke).mockResolvedValue([null, { code: "ERR", message: "Save failed" }]);

      await useConfigStore.getState().saveConfig();

      expect(useConfigStore.getState().saved).toBe(false);
      expect(consoleSpy).toHaveBeenCalled();
      consoleSpy.mockRestore();
    });
  });

  describe("updateConfig", () => {
    it("should update config with updater function", () => {
      const { updateConfig } = useConfigStore.getState();

      updateConfig((prev) => ({
        ...prev,
        defaultFrom: "en",
        defaultTo: "ja",
      }));

      const { config } = useConfigStore.getState();
      expect(config.defaultFrom).toBe("en");
      expect(config.defaultTo).toBe("ja");
    });

    it("should preserve other config fields", () => {
      const { updateConfig } = useConfigStore.getState();
      const originalProvider = useConfigStore.getState().config.llm.provider;

      updateConfig((prev) => ({
        ...prev,
        defaultFrom: "en",
      }));

      expect(useConfigStore.getState().config.llm.provider).toBe(originalProvider);
    });
  });

  describe("updateLlm", () => {
    it("should update specific LLM field", () => {
      const { updateLlm } = useConfigStore.getState();

      updateLlm("apiKey", "new-api-key");

      expect(useConfigStore.getState().config.llm.apiKey).toBe("new-api-key");
    });

    it("should update provider", () => {
      const { updateLlm } = useConfigStore.getState();

      updateLlm("provider", "openai");

      expect(useConfigStore.getState().config.llm.provider).toBe("openai");
    });

    it("should update model", () => {
      const { updateLlm } = useConfigStore.getState();

      updateLlm("model", "gpt-4-turbo");

      expect(useConfigStore.getState().config.llm.model).toBe("gpt-4-turbo");
    });

    it("should not affect other LLM fields", () => {
      useConfigStore.setState({
        config: {
          ...useConfigStore.getState().config,
          llm: {
            provider: "deepseek",
            apiKey: "old-key",
            apiKeys: [],
            baseUrl: "http://old.com",
            model: "old-model",
          },
        },
      });

      useConfigStore.getState().updateLlm("apiKey", "new-key");

      const { llm } = useConfigStore.getState().config;
      expect(llm.apiKey).toBe("new-key");
      expect(llm.provider).toBe("deepseek");
      expect(llm.baseUrl).toBe("http://old.com");
      expect(llm.model).toBe("old-model");
    });
  });

  describe("loadCacheSize", () => {
    it("should load cache size from backend", async () => {
      const { invokeOrDefault } = await import("../services/invoke");
      vi.mocked(invokeOrDefault).mockResolvedValue(1024);

      await useConfigStore.getState().loadCacheSize();

      expect(invokeOrDefault).toHaveBeenCalledWith("cache_size", undefined, 0);
      expect(useConfigStore.getState().cacheSize).toBe(1024);
    });

    it("should default to 0 on error", async () => {
      const { invokeOrDefault } = await import("../services/invoke");
      vi.mocked(invokeOrDefault).mockResolvedValue(0);

      await useConfigStore.getState().loadCacheSize();

      expect(useConfigStore.getState().cacheSize).toBe(0);
    });
  });

  describe("clearCache", () => {
    it("should clear cache and reset size", async () => {
      useConfigStore.setState({ cacheSize: 1024 });
      vi.mocked(safeInvoke).mockResolvedValue([null, null]);

      await useConfigStore.getState().clearCache();

      expect(safeInvoke).toHaveBeenCalledWith("clear_cache");
      expect(useConfigStore.getState().cacheSize).toBe(0);
    });

    it("should handle clear cache error", async () => {
      const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
      useConfigStore.setState({ cacheSize: 1024 });
      vi.mocked(safeInvoke).mockResolvedValue([null, { code: "ERR", message: "Failed" }]);

      await useConfigStore.getState().clearCache();

      expect(useConfigStore.getState().cacheSize).toBe(1024);
      expect(consoleSpy).toHaveBeenCalled();
      consoleSpy.mockRestore();
    });
  });
});
