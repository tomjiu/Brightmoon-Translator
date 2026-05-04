// Moon Translator - Popup Script

document.addEventListener("DOMContentLoaded", async () => {
  // Tab switching
  const tabs = document.querySelectorAll(".tab");
  const tabContents = document.querySelectorAll(".tab-content");

  tabs.forEach(tab => {
    tab.addEventListener("click", () => {
      tabs.forEach(t => t.classList.remove("active"));
      tabContents.forEach(c => c.style.display = "none");

      tab.classList.add("active");
      const targetId = tab.dataset.tab + "-tab";
      document.getElementById(targetId).style.display = "block";
    });
  });

  // Language swap
  const fromLang = document.getElementById("from-lang");
  const toLang = document.getElementById("to-lang");
  const swapBtn = document.getElementById("swap-btn");

  swapBtn.addEventListener("click", () => {
    if (fromLang.value === "auto") return;
    const temp = fromLang.value;
    fromLang.value = toLang.value;
    toLang.value = temp;
  });

  // Translate
  const sourceText = document.getElementById("source-text");
  const translateBtn = document.getElementById("translate-btn");
  const resultContainer = document.getElementById("result-container");
  const engineName = document.getElementById("engine-name");
  const resultText = document.getElementById("result-text");
  const errorContainer = document.getElementById("error-container");
  const errorText = document.getElementById("error-text");
  const copyBtn = document.getElementById("copy-btn");

  translateBtn.addEventListener("click", async () => {
    const text = sourceText.value.trim();
    if (!text) return;

    translateBtn.disabled = true;
    translateBtn.textContent = "翻译中...";
    resultContainer.style.display = "none";
    errorContainer.style.display = "none";

    try {
      const response = await browser.runtime.sendMessage({
        type: "translate",
        text: text,
        from: fromLang.value,
        to: toLang.value
      });

      if (response.success) {
        engineName.textContent = "翻译结果";
        resultText.textContent = response.result;
        resultContainer.style.display = "block";
      } else {
        errorText.textContent = response.error;
        errorContainer.style.display = "block";
      }
    } catch (err) {
      errorText.textContent = "翻译请求失败: " + err.message;
      errorContainer.style.display = "block";
    } finally {
      translateBtn.disabled = false;
      translateBtn.textContent = "翻译";
    }
  });

  // Copy result
  copyBtn.addEventListener("click", () => {
    navigator.clipboard.writeText(resultText.textContent).then(() => {
      copyBtn.textContent = "已复制";
      setTimeout(() => { copyBtn.textContent = "复制"; }, 1500);
    });
  });

  // Settings
  const engineSelect = document.getElementById("engine-select");
  const llmSettings = document.getElementById("llm-settings");
  const llmProvider = document.getElementById("llm-provider");
  const apiKey = document.getElementById("api-key");
  const baseUrl = document.getElementById("base-url");
  const baseUrlGroup = document.getElementById("base-url-group");
  const model = document.getElementById("model");
  const saveBtn = document.getElementById("save-btn");
  const saveStatus = document.getElementById("save-status");

  // Load saved config
  const configResponse = await browser.runtime.sendMessage({ type: "getConfig" });
  if (configResponse && configResponse.config) {
    const config = configResponse.config;
    llmProvider.value = config.llm.provider;
    apiKey.value = config.llm.apiKey;
    baseUrl.value = config.llm.baseUrl;
    model.value = config.llm.model;
    toLang.value = config.targetLang || "zh";

    if (config.llm.provider === "custom") {
      baseUrlGroup.style.display = "block";
    }
  }

  // Provider change
  llmProvider.addEventListener("change", () => {
    const providers = {
      deepseek: { baseUrl: "https://api.deepseek.com/v1", model: "deepseek-chat" },
      openai: { baseUrl: "https://api.openai.com/v1", model: "gpt-4o-mini" }
    };

    if (llmProvider.value === "custom") {
      baseUrlGroup.style.display = "block";
    } else {
      baseUrlGroup.style.display = "none";
      const p = providers[llmProvider.value];
      if (p) {
        baseUrl.value = p.baseUrl;
        model.value = p.model;
      }
    }
  });

  // Save settings
  saveBtn.addEventListener("click", async () => {
    const config = {
      llm: {
        provider: llmProvider.value,
        apiKey: apiKey.value,
        baseUrl: baseUrl.value,
        model: model.value
      },
      targetLang: toLang.value
    };

    await browser.runtime.sendMessage({ type: "saveConfig", config });
    saveStatus.textContent = "已保存";
    setTimeout(() => { saveStatus.textContent = ""; }, 2000);
  });

  // Keyboard shortcut: Ctrl+Enter to translate
  sourceText.addEventListener("keydown", (e) => {
    if (e.ctrlKey && e.key === "Enter") {
      translateBtn.click();
    }
  });
});
