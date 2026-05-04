// Background service worker for Moon Translator

const DEFAULT_CONFIG = {
  llm: {
    provider: "deepseek",
    apiKey: "",
    baseUrl: "https://api.deepseek.com/v1",
    model: "deepseek-chat"
  },
  targetLang: "zh",
  sourceLang: "auto"
};

// Load config from storage
async function getConfig() {
  const result = await browser.storage.local.get("config");
  return { ...DEFAULT_CONFIG, ...result.config };
}

// Save config to storage
async function saveConfig(config) {
  await browser.storage.local.set({ config });
}

// Translate using LLM API
async function translateWithLLM(text, from, to, config) {
  if (!config.llm.apiKey) {
    throw new Error("请先配置API Key");
  }

  const langMap = {
    zh: "中文", en: "English", ja: "日本語", ko: "한국어",
    fr: "Français", de: "Deutsch", es: "Español", ru: "Русский"
  };

  const fromLang = langMap[from] || from;
  const toLang = langMap[to] || to;

  const systemPrompt = `你是一个专业的翻译专家。请遵循以下规则：
1. 准确传达原文含义，保持自然流畅
2. 专业术语使用标准译法
3. 保持原文的语气和风格
4. 只返回翻译结果，不要添加任何解释

源语言：${fromLang}
目标语言：${toLang}`;

  const response = await fetch(`${config.llm.baseUrl}/chat/completions`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `Bearer ${config.llm.apiKey}`
    },
    body: JSON.stringify({
      model: config.llm.model,
      messages: [
        { role: "system", content: systemPrompt },
        { role: "user", content: text }
      ],
      temperature: 0.3,
      max_tokens: 4096
    })
  });

  if (!response.ok) {
    const error = await response.text();
    throw new Error(`API错误: ${response.status} - ${error}`);
  }

  const data = await response.json();
  return data.choices[0].message.content.trim();
}

// Translate using Google Translate (free)
async function translateWithGoogle(text, from, to) {
  const fromCode = from === "auto" ? "auto" : from;
  const url = `https://translate.googleapis.com/translate_a/single?client=gtx&sl=${fromCode}&tl=${to}&dt=t&q=${encodeURIComponent(text)}`;

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Google翻译错误: ${response.status}`);
  }

  const data = await response.json();
  return data[0].map(item => item[0]).join("");
}

// Main translate function
async function translate(text, from, to) {
  const config = await getConfig();

  // Try LLM first if configured
  if (config.llm.apiKey) {
    try {
      return await translateWithLLM(text, from, to, config);
    } catch (e) {
      console.warn("LLM translation failed, falling back to Google:", e);
    }
  }

  // Fallback to Google
  return await translateWithGoogle(text, from, to);
}

// Listen for messages from content script and popup
browser.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === "translate") {
    translate(message.text, message.from || "auto", message.to || "zh")
      .then(result => sendResponse({ success: true, result }))
      .catch(error => sendResponse({ success: false, error: error.message }));
    return true; // Keep message channel open for async response
  }

  if (message.type === "getConfig") {
    getConfig().then(config => sendResponse({ config }));
    return true;
  }

  if (message.type === "saveConfig") {
    saveConfig(message.config).then(() => sendResponse({ success: true }));
    return true;
  }
});

// Create context menu
browser.contextMenus.create({
  id: "translate-selection",
  title: "翻译选中文本",
  contexts: ["selection"]
});

browser.contextMenus.onClicked.addListener((info, tab) => {
  if (info.menuItemId === "translate-selection") {
    browser.tabs.sendMessage(tab.id, {
      type: "translate-selection",
      text: info.selectionText
    });
  }
});
