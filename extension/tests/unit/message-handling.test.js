import { describe, it, expect, beforeEach, vi } from "vitest";

// ==================== Message Handling Logic ====================
// Tests for the service worker's message handling and config management

function createMockConfig() {
  return {
    engines: {
      google: { enabled: true },
      llm: {
        enabled: false,
        provider: "deepseek",
        apiKey: "",
        baseUrl: "https://api.deepseek.com/v1",
        model: "deepseek-chat",
      },
      youdao: { enabled: true },
      deepl: { enabled: false, apiKey: "", pro: false },
      deeplx: { enabled: false, endpoint: "http://localhost:1188" },
      microsoft: { enabled: false },
    },
    targetLang: "zh",
    sourceLang: "auto",
    autoTranslate: false,
    showButton: true,
    hover: {
      enabled: true,
      delay: 300,
      minTextLength: 2,
      modifierKey: "none",
    },
  };
}

// Simulate message routing logic from service-worker.js
function createMessageRouter() {
  const config = createMockConfig();
  const translations = new Map();

  function getConfig() {
    return { ...config };
  }

  function saveConfig(newConfig) {
    Object.assign(config, newConfig);
  }

  function handleTranslate(text, from, to) {
    // Simulate translation
    return {
      success: true,
      results: [{ engine: "Google", text: `translated:${text}` }],
      primary: { engine: "Google", text: `translated:${text}` },
    };
  }

  function handleMessage(message, sender) {
    return new Promise((resolve) => {
      const sendResponse = (response) => resolve(response);

      switch (message.type) {
        case "translate":
          try {
            const result = handleTranslate(
              message.text,
              message.from || "auto",
              message.to || "zh"
            );
            sendResponse(result);
          } catch (error) {
            sendResponse({ success: false, error: error.message });
          }
          break;

        case "getConfig":
          sendResponse({ config: getConfig() });
          break;

        case "saveConfig":
          saveConfig(message.config);
          sendResponse({ success: true });
          break;

        case "translatePage":
          sendResponse({ success: true });
          break;

        case "restorePage":
          sendResponse({ success: true });
          break;

        case "translatePageDesktop":
          if (!message.segments || message.segments.length === 0) {
            sendResponse({
              success: false,
              error: "No segments provided",
            });
          } else {
            sendResponse({
              success: true,
              translations: message.segments.map((seg) => ({
                index: seg.index,
                translated: `translated:${seg.text}`,
              })),
            });
          }
          break;

        case "desktopStatus":
          sendResponse({ reachable: false });
          break;

        case "checkDesktopHealth":
          sendResponse({ reachable: false });
          break;

        default:
          sendResponse({ success: false, error: "Unknown message type" });
      }
    });
  }

  return { handleMessage, getConfig, saveConfig };
}

// ==================== Tests ====================

describe("Service Worker Message Handling", () => {
  let router;

  beforeEach(() => {
    router = createMessageRouter();
  });

  describe("translate message", () => {
    it("should handle translation request", async () => {
      const response = await router.handleMessage({
        type: "translate",
        text: "hello",
        from: "en",
        to: "zh",
      });

      expect(response.success).toBe(true);
      expect(response.results).toBeDefined();
      expect(response.results[0].text).toBe("translated:hello");
    });

    it("should use default language values", async () => {
      const response = await router.handleMessage({
        type: "translate",
        text: "hello",
      });

      expect(response.success).toBe(true);
    });

    it("should return primary result", async () => {
      const response = await router.handleMessage({
        type: "translate",
        text: "test",
      });

      expect(response.primary).toBeDefined();
      expect(response.primary.engine).toBe("Google");
    });
  });

  describe("getConfig message", () => {
    it("should return config", async () => {
      const response = await router.handleMessage({ type: "getConfig" });

      expect(response.config).toBeDefined();
      expect(response.config.targetLang).toBe("zh");
      expect(response.config.sourceLang).toBe("auto");
    });

    it("should include engine settings", async () => {
      const response = await router.handleMessage({ type: "getConfig" });

      expect(response.config.engines).toBeDefined();
      expect(response.config.engines.google.enabled).toBe(true);
      expect(response.config.engines.youdao.enabled).toBe(true);
    });

    it("should include hover settings", async () => {
      const response = await router.handleMessage({ type: "getConfig" });

      expect(response.config.hover).toBeDefined();
      expect(response.config.hover.enabled).toBe(true);
      expect(response.config.hover.delay).toBe(300);
    });
  });

  describe("saveConfig message", () => {
    it("should save config", async () => {
      const newConfig = {
        targetLang: "en",
        sourceLang: "zh",
      };

      const response = await router.handleMessage({
        type: "saveConfig",
        config: newConfig,
      });

      expect(response.success).toBe(true);

      // Verify config was saved
      const configResponse = await router.handleMessage({ type: "getConfig" });
      expect(configResponse.config.targetLang).toBe("en");
      expect(configResponse.config.sourceLang).toBe("zh");
    });

    it("should preserve other config values", async () => {
      await router.handleMessage({
        type: "saveConfig",
        config: { targetLang: "ja" },
      });

      const response = await router.handleMessage({ type: "getConfig" });
      expect(response.config.targetLang).toBe("ja");
      expect(response.config.sourceLang).toBe("auto"); // unchanged
    });
  });

  describe("translatePageDesktop message", () => {
    it("should handle batch translation", async () => {
      const response = await router.handleMessage({
        type: "translatePageDesktop",
        segments: [
          { index: 0, text: "Hello", selector: "p:nth-child(1)" },
          { index: 1, text: "World", selector: "p:nth-child(2)" },
        ],
        from: "en",
        to: "zh",
      });

      expect(response.success).toBe(true);
      expect(response.translations).toHaveLength(2);
      expect(response.translations[0].translated).toBe("translated:Hello");
      expect(response.translations[1].translated).toBe("translated:World");
    });

    it("should handle empty segments", async () => {
      const response = await router.handleMessage({
        type: "translatePageDesktop",
        segments: [],
      });

      expect(response.success).toBe(false);
      expect(response.error).toBe("No segments provided");
    });
  });

  describe("page control messages", () => {
    it("should handle translatePage", async () => {
      const response = await router.handleMessage({ type: "translatePage" });
      expect(response.success).toBe(true);
    });

    it("should handle restorePage", async () => {
      const response = await router.handleMessage({ type: "restorePage" });
      expect(response.success).toBe(true);
    });
  });

  describe("desktop status messages", () => {
    it("should handle desktopStatus", async () => {
      const response = await router.handleMessage({ type: "desktopStatus" });
      expect(response.reachable).toBe(false);
    });

    it("should handle checkDesktopHealth", async () => {
      const response = await router.handleMessage({
        type: "checkDesktopHealth",
      });
      expect(response.reachable).toBe(false);
    });
  });

  describe("unknown message types", () => {
    it("should return error for unknown types", async () => {
      const response = await router.handleMessage({
        type: "unknownType",
      });

      expect(response.success).toBe(false);
      expect(response.error).toBe("Unknown message type");
    });
  });
});

describe("Config Management", () => {
  let router;

  beforeEach(() => {
    router = createMessageRouter();
  });

  it("should have correct default config", () => {
    const config = router.getConfig();

    expect(config.targetLang).toBe("zh");
    expect(config.sourceLang).toBe("auto");
    expect(config.engines.google.enabled).toBe(true);
    expect(config.engines.youdao.enabled).toBe(true);
    expect(config.engines.llm.enabled).toBe(false);
    expect(config.engines.microsoft.enabled).toBe(false);
    expect(config.engines.deepl.enabled).toBe(false);
    expect(config.engines.deeplx.enabled).toBe(false);
  });

  it("should update specific config values", () => {
    router.saveConfig({ targetLang: "ja" });
    const config = router.getConfig();

    expect(config.targetLang).toBe("ja");
    expect(config.sourceLang).toBe("auto"); // unchanged
  });

  it("should update engine settings", () => {
    router.saveConfig({
      engines: {
        ...router.getConfig().engines,
        llm: { enabled: true, apiKey: "test-key" },
      },
    });

    const config = router.getConfig();
    expect(config.engines.llm.enabled).toBe(true);
  });

  it("should update hover settings", () => {
    router.saveConfig({
      hover: {
        enabled: false,
        delay: 500,
        minTextLength: 5,
        modifierKey: "ctrl",
      },
    });

    const config = router.getConfig();
    expect(config.hover.enabled).toBe(false);
    expect(config.hover.delay).toBe(500);
    expect(config.hover.minTextLength).toBe(5);
    expect(config.hover.modifierKey).toBe("ctrl");
  });
});

describe("Engine Selection Logic", () => {
  it("should select enabled engines", () => {
    const config = createMockConfig();
    const enabledEngines = [];

    if (config.engines.google.enabled) enabledEngines.push("Google");
    if (config.engines.youdao.enabled) enabledEngines.push("Youdao");
    if (config.engines.microsoft.enabled) enabledEngines.push("Microsoft");
    if (config.engines.llm.enabled && config.engines.llm.apiKey) enabledEngines.push("LLM");
    if (config.engines.deepl.enabled && config.engines.deepl.apiKey) enabledEngines.push("DeepL");
    if (config.engines.deeplx.enabled) enabledEngines.push("DeepLX");

    expect(enabledEngines).toContain("Google");
    expect(enabledEngines).toContain("Youdao");
    expect(enabledEngines).not.toContain("LLM");
    expect(enabledEngines).not.toContain("DeepL");
  });

  it("should require API key for LLM engine", () => {
    const config = createMockConfig();
    config.engines.llm.enabled = true;
    // No API key set

    const canUseLLM = config.engines.llm.enabled && config.engines.llm.apiKey;
    expect(canUseLLM).toBeFalsy();
  });

  it("should require API key for DeepL engine", () => {
    const config = createMockConfig();
    config.engines.deepl.enabled = true;
    // No API key set

    const canUseDeepL = config.engines.deepl.enabled && config.engines.deepl.apiKey;
    expect(canUseDeepL).toBeFalsy();
  });

  it("should not require API key for Google engine", () => {
    const config = createMockConfig();
    config.engines.google.enabled = true;

    const canUseGoogle = config.engines.google.enabled;
    expect(canUseGoogle).toBe(true);
  });

  it("should not require API key for Youdao engine", () => {
    const config = createMockConfig();
    config.engines.youdao.enabled = true;

    const canUseYoudao = config.engines.youdao.enabled;
    expect(canUseYoudao).toBe(true);
  });
});
