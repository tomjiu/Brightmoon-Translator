// Popup script for Moon Translator

const DEFAULT_CONFIG = {
  engines: {
    google: { enabled: true },
    llm: {
      enabled: false,
      provider: "deepseek",
      apiKey: "",
      baseUrl: "https://api.deepseek.com/v1",
      model: "deepseek-chat"
    },
    youdao: { enabled: true },
    deepl: { enabled: false, apiKey: "", pro: false },
    deeplx: { enabled: false, apiKey: "", pro: false },
    microsoft: { enabled: false }
  },
  targetLang: "zh",
  sourceLang: "auto"
};

let config = { ...DEFAULT_CONFIG };

// ==================== Init ====================

document.addEventListener("DOMContentLoaded", async () => {
  await loadConfig();
  setupEventListeners();
  updateUI();
  checkDesktopStatus();
});

// ==================== Config ====================

async function loadConfig() {
  try {
    const response = await chrome.runtime.sendMessage({ type: "getConfig" });
    if (response?.config) {
      config = { ...DEFAULT_CONFIG, ...response.config };
      if (!config.engines) config.engines = DEFAULT_CONFIG.engines;
    }
  } catch (e) {
    console.error("Failed to load config:", e);
  }
}

async function saveConfig() {
  try {
    await chrome.runtime.sendMessage({ type: "saveConfig", config });
    showNotification("设置已保存");
  } catch (e) {
    console.error("Failed to save config:", e);
    showNotification("保存失败", true);
  }
}

// ==================== UI ====================

function updateUI() {
  // Language selects
  document.getElementById("sourceLang").value = config.sourceLang || "auto";
  document.getElementById("targetLang").value = config.targetLang || "zh";

  // Engine checkboxes
  document.getElementById("engineGoogle").checked = config.engines?.google?.enabled ?? true;
  document.getElementById("engineYoudao").checked = config.engines?.youdao?.enabled ?? true;
  document.getElementById("engineMicrosoft").checked = config.engines?.microsoft?.enabled ?? false;
  document.getElementById("engineLlm").checked = config.engines?.llm?.enabled ?? false;
  document.getElementById("engineDeepl").checked = config.engines?.deepl?.enabled ?? false;
  document.getElementById("engineDeeplx").checked = config.engines?.deeplx?.enabled ?? false;

  // LLM settings
  document.getElementById("llmProvider").value = config.engines?.llm?.provider || "deepseek";
  document.getElementById("llmApiKey").value = config.engines?.llm?.apiKey || "";
  document.getElementById("llmBaseUrl").value = config.engines?.llm?.baseUrl || "https://api.deepseek.com/v1";
  document.getElementById("llmModel").value = config.engines?.llm?.model || "deepseek-chat";

  // DeepL settings
  document.getElementById("deeplApiKey").value = config.engines?.deepl?.apiKey || "";
  document.getElementById("deeplPro").checked = config.engines?.deepl?.pro ?? false;

  // DeepLX settings
  document.getElementById("deeplxApiKey").value = config.engines?.deeplx?.apiKey || "";
  document.getElementById("deeplxPro").checked = config.engines?.deeplx?.pro ?? false;

  // Show/hide settings sections
  toggleLlmSettings();
  toggleDeeplSettings();
  toggleDeeplxSettings();
}

function toggleLlmSettings() {
  const enabled = document.getElementById("engineLlm").checked;
  document.getElementById("llmSettings").style.display = enabled ? "flex" : "none";
}

function toggleDeeplSettings() {
  const enabled = document.getElementById("engineDeepl").checked;
  document.getElementById("deeplSettings").style.display = enabled ? "flex" : "none";
}

function toggleDeeplxSettings() {
  const enabled = document.getElementById("engineDeeplx").checked;
  document.getElementById("deeplxSettings").style.display = enabled ? "flex" : "none";
}

function showNotification(message, isError = false) {
  const errorDiv = document.getElementById("error");
  errorDiv.textContent = message;
  errorDiv.style.display = "block";
  errorDiv.style.background = isError ? "#fff5f5" : "#f0fff4";
  errorDiv.style.borderColor = isError ? "#fed7d7" : "#c6f6d5";
  errorDiv.style.color = isError ? "#c53030" : "#276749";

  setTimeout(() => {
    errorDiv.style.display = "none";
  }, 2000);
}

// ==================== Desktop Status ====================

async function checkDesktopStatus() {
  try {
    // Use checkDesktopHealth for a real-time probe, not the cached value
    const response = await chrome.runtime.sendMessage({ type: "checkDesktopHealth" });
    const dot = document.querySelector("#desktopStatus .status-dot");
    const syncBtn = document.getElementById("syncGlossary");

    if (response?.reachable) {
      dot.classList.add("connected");
      dot.classList.remove("disconnected");
      if (syncBtn) syncBtn.style.display = "block";
    } else {
      dot.classList.add("disconnected");
      dot.classList.remove("connected");
      if (syncBtn) syncBtn.style.display = "none";
    }
  } catch {
    const dot = document.querySelector("#desktopStatus .status-dot");
    if (dot) {
      dot.classList.add("disconnected");
      dot.classList.remove("connected");
    }
  }
}

async function syncGlossary() {
  const DESKTOP_URL = "http://127.0.0.1:60828";
  try {
    // Fetch glossary and blacklist from desktop
    const [glossaryResp, blacklistResp] = await Promise.all([
      fetch(`${DESKTOP_URL}/glossary`),
      fetch(`${DESKTOP_URL}/blacklist`)
    ]);

    if (glossaryResp.ok) {
      const glossary = await glossaryResp.json();
      await chrome.storage.local.set({ desktopGlossary: glossary });
    }

    if (blacklistResp.ok) {
      const blacklist = await blacklistResp.json();
      await chrome.storage.local.set({ desktopBlacklist: blacklist.words || [] });
    }

    showNotification("术语库和黑名单已同步");
  } catch (e) {
    showNotification("同步失败: " + e.message, true);
  }
}

// ==================== Translation ====================

async function translateText() {
  const text = document.getElementById("sourceText").value.trim();
  if (!text) return;

  const from = document.getElementById("sourceLang").value;
  const to = document.getElementById("targetLang").value;

  // Show loading
  document.getElementById("loading").style.display = "flex";
  document.getElementById("results").style.display = "none";
  document.getElementById("error").style.display = "none";
  document.getElementById("translateBtn").disabled = true;

  try {
    const response = await chrome.runtime.sendMessage({
      type: "translate",
      text: text,
      from: from,
      to: to
    });

    document.getElementById("loading").style.display = "none";
    document.getElementById("translateBtn").disabled = false;

    if (response.success) {
      const resultsDiv = document.getElementById("resultsList");
      resultsDiv.innerHTML = "";

      const items = response.results || (response.primary ? [response.primary] : []);
      items.forEach(result => {
        const item = document.createElement("div");
        item.className = "result-item";
        item.innerHTML = `
          <div class="result-engine">${escapeHtml(result.engine)}</div>
          <div class="result-text">${escapeHtml(result.text)}</div>
        `;
        resultsDiv.appendChild(item);
      });

      document.getElementById("results").style.display = "flex";
    } else {
      const errorDiv = document.getElementById("error");
      errorDiv.textContent = response.error || "翻译失败";
      errorDiv.style.display = "block";
    }
  } catch (err) {
    document.getElementById("loading").style.display = "none";
    document.getElementById("translateBtn").disabled = false;
    const errorDiv = document.getElementById("error");
    errorDiv.textContent = "翻译请求失败: " + err.message;
    errorDiv.style.display = "block";
  }
}

function copyResult() {
  const results = document.querySelectorAll(".result-text");
  if (results.length > 0) {
    const text = Array.from(results).map(r => r.textContent).join("\n");
    navigator.clipboard.writeText(text).then(() => {
      const btn = document.getElementById("copyBtn");
      btn.textContent = "已复制 ✓";
      setTimeout(() => { btn.textContent = "复制结果"; }, 1500);
    });
  }
}

// ==================== Event Listeners ====================

function setupEventListeners() {
  // Translate button
  document.getElementById("translateBtn").addEventListener("click", translateText);

  // Translate on Enter (Ctrl+Enter)
  document.getElementById("sourceText").addEventListener("keydown", (e) => {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      translateText();
    }
  });

  // Copy button
  document.getElementById("copyBtn").addEventListener("click", copyResult);

  // Swap languages
  document.getElementById("swapLang").addEventListener("click", () => {
    const source = document.getElementById("sourceLang");
    const target = document.getElementById("targetLang");
    if (source.value === "auto") return;

    const temp = source.value;
    source.value = target.value;
    target.value = temp;
  });

  // Toggle settings
  document.getElementById("toggleSettings").addEventListener("click", () => {
    const panel = document.getElementById("settingsPanel");
    const btn = document.getElementById("toggleSettings");
    if (panel.style.display === "none") {
      panel.style.display = "flex";
      btn.textContent = "✕ 关闭设置";
    } else {
      panel.style.display = "none";
      btn.textContent = "⚙️ 设置";
    }
  });

  // Engine checkboxes
  document.getElementById("engineLlm").addEventListener("change", toggleLlmSettings);
  document.getElementById("engineDeepl").addEventListener("change", toggleDeeplSettings);
  document.getElementById("engineDeeplx").addEventListener("change", toggleDeeplxSettings);

  // LLM Provider change
  document.getElementById("llmProvider").addEventListener("change", (e) => {
    const provider = e.target.value;
    const baseUrlInput = document.getElementById("llmBaseUrl");
    const modelInput = document.getElementById("llmModel");

    switch (provider) {
      case "deepseek":
        baseUrlInput.value = "https://api.deepseek.com/v1";
        modelInput.value = "deepseek-chat";
        break;
      case "openai":
        baseUrlInput.value = "https://api.openai.com/v1";
        modelInput.value = "gpt-3.5-turbo";
        break;
    }
  });

  // Save settings
  document.getElementById("saveSettings").addEventListener("click", async () => {
    config.sourceLang = document.getElementById("sourceLang").value;
    config.targetLang = document.getElementById("targetLang").value;

    config.engines.google.enabled = document.getElementById("engineGoogle").checked;
    config.engines.youdao.enabled = document.getElementById("engineYoudao").checked;
    config.engines.microsoft.enabled = document.getElementById("engineMicrosoft").checked;
    config.engines.llm.enabled = document.getElementById("engineLlm").checked;
    config.engines.deepl.enabled = document.getElementById("engineDeepl").checked;
    config.engines.deeplx.enabled = document.getElementById("engineDeeplx").checked;

    config.engines.llm.provider = document.getElementById("llmProvider").value;
    config.engines.llm.apiKey = document.getElementById("llmApiKey").value;
    config.engines.llm.baseUrl = document.getElementById("llmBaseUrl").value;
    config.engines.llm.model = document.getElementById("llmModel").value;

    config.engines.deepl.apiKey = document.getElementById("deeplApiKey").value;
    config.engines.deepl.pro = document.getElementById("deeplPro").checked;

    config.engines.deeplx.apiKey = document.getElementById("deeplxApiKey").value;
    config.engines.deeplx.pro = document.getElementById("deeplxPro").checked;

    await saveConfig();
  });

  // Open full app
  document.getElementById("openFullApp").addEventListener("click", (e) => {
    e.preventDefault();
    // This would open the Tauri app if installed
    // For now, just show a message
    showNotification("请使用桌面版 Moon Translator");
  });

  // Sync glossary from desktop
  const syncBtn = document.getElementById("syncGlossary");
  if (syncBtn) {
    syncBtn.addEventListener("click", syncGlossary);
  }
}

// ==================== Helpers ====================

function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}
