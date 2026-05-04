// Background service worker for Moon Translator
// Supports Chrome MV3 and Firefox MV3

// ==================== Desktop Bridge ====================
// Connects to the Tauri desktop app's local HTTP API for real translation.
// Falls back to local browser-based engines when desktop is unreachable.

const DESKTOP_URL = "http://127.0.0.1:60828";

const DesktopBridge = {
  reachable: false,

  async checkHealth() {
    try {
      const resp = await fetch(`${DESKTOP_URL}/health`, {
        method: "GET",
        signal: AbortSignal.timeout(3000)
      });
      this.reachable = resp.ok;
    } catch {
      this.reachable = false;
    }
    return this.reachable;
  },

  async translateViaDesktop(text, from, to) {
    const body = {
      mode: "selection",
      payload: {
        type: "Selection",
        data: {
          text,
          selector: null,
          bounds: null,
          url: "",
          title: ""
        }
      },
      from: from || "auto",
      to: to || "zh",
      showOverlay: false,
      replaceInline: false
    };

    const resp = await fetch(`${DESKTOP_URL}/browser/translate`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(15000)
    });

    if (!resp.ok) {
      const err = await resp.json().catch(() => ({}));
      throw new Error(err.message || `Desktop API error: ${resp.status}`);
    }

    const data = await resp.json();
    // data.response.results = [{ engine, text }, ...]
    return {
      results: data.response.results || [],
      primary: data.response.results?.[0] || null,
      detectedLanguage: data.response.detectedLanguage
    };
  },

  async translatePageViaDesktop(segments, from, to) {
    const body = {
      mode: "full_page",
      payload: {
        type: "FullPage",
        data: {
          url: "",
          title: "",
          segments
        }
      },
      from: from || "auto",
      to: to || "zh",
      showOverlay: false,
      replaceInline: true
    };

    const resp = await fetch(`${DESKTOP_URL}/browser/translate`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(30000)
    });

    if (!resp.ok) {
      const err = await resp.json().catch(() => ({}));
      throw new Error(err.message || `Desktop API error: ${resp.status}`);
    }

    const data = await resp.json();
    return data.segmentTranslations || [];
  }
};

// Health check alarm — keeps service worker alive and polls desktop status
chrome.alarms.create("desktop-health", { periodInMinutes: 0.5 });
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === "desktop-health") {
    DesktopBridge.checkHealth();
  }
});

// Initial health check on service worker startup
DesktopBridge.checkHealth();

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
    deeplx: { enabled: false, endpoint: "http://localhost:1188" },
    microsoft: { enabled: false }
  },
  targetLang: "zh",
  sourceLang: "auto",
  autoTranslate: false,
  showButton: true
};

// ==================== Config Management ====================

async function getConfig() {
  try {
    const result = await chrome.storage.local.get("config");
    return { ...DEFAULT_CONFIG, ...result.config };
  } catch (e) {
    console.error("Failed to load config:", e);
    return DEFAULT_CONFIG;
  }
}

async function saveConfig(config) {
  await chrome.storage.local.set({ config });
}

// ==================== Translation Engines ====================

// Google Translate (free, no key needed)
async function translateWithGoogle(text, from, to) {
  const fromCode = from === "auto" ? "auto" : from;
  const url = `https://translate.googleapis.com/translate_a/single?client=gtx&sl=${fromCode}&tl=${to}&dt=t&q=${encodeURIComponent(text)}`;

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Google翻译错误: ${response.status}`);
  }

  const data = await response.json();
  return {
    engine: "Google",
    text: data[0].map(item => item[0]).join("")
  };
}

// LLM Translation (OpenAI-compatible API)
async function translateWithLLM(text, from, to, config) {
  if (!config.engines.llm.apiKey) {
    throw new Error("请先配置LLM API Key");
  }

  const langMap = {
    zh: "中文", en: "English", ja: "日本語", ko: "한국어",
    fr: "Français", de: "Deutsch", es: "Español", ru: "Русский",
    pt: "Português", it: "Italiano", ar: "العربية", th: "ไทย", vi: "Tiếng Việt"
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

  const response = await fetch(`${config.engines.llm.baseUrl}/chat/completions`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "Authorization": `Bearer ${config.engines.llm.apiKey}`
    },
    body: JSON.stringify({
      model: config.engines.llm.model,
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
    throw new Error(`LLM API错误: ${response.status}`);
  }

  const data = await response.json();
  return {
    engine: config.engines.llm.provider.toUpperCase(),
    text: data.choices[0].message.content.trim()
  };
}

// Youdao Translate (free, CDN-based key)
async function translateWithYoudao(text, from, to) {
  // Map language codes
  const langMap = {
    zh: "zh-CHS", en: "en", ja: "ja", ko: "ko",
    fr: "fr", de: "de", es: "es", ru: "ru",
    pt: "pt", it: "it", ar: "ar", th: "th", vi: "vi",
    auto: "auto"
  };

  const fromLang = langMap[from] || "auto";
  const toLang = langMap[to] || "zh-CHS";

  // Use Youdao's free web API
  const url = "https://dict-trans.youdao.com/webtranslate";
  const params = new URLSearchParams({
    i: text,
    from: fromLang,
    to: toLang,
    useTerm: "false",
    domain: "0",
    dictResult: "true",
    keyid: "webfanyi",
    appVersion: "1.0.0",
    vendor: "web",
    pointParam: "client,mysticTime,product",
    mysticTime: Date.now().toString(),
    product: "webfanyi",
    client: "fanyideskweb",
    keyfrom: "fanyi.web"
  });

  // Simple sign (Youdao uses this for web translate)
  const signKey = "fsdsogkndfokasodnaso";
  const signStr = `client=fanyideskweb&mysticTime=${params.get("mysticTime")}&product=webfanyi&key=${signKey}`;
  const sign = await md5(signStr);
  params.append("sign", sign);

  const response = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/x-www-form-urlencoded",
      "Referer": "https://fanyi.youdao.com/",
      "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
    },
    body: params.toString()
  });

  if (!response.ok) {
    throw new Error(`有道翻译错误: ${response.status}`);
  }

  const data = await response.json();
  if (data.translateResult && data.translateResult[0]) {
    const result = data.translateResult[0].map(item => item.tgt).join("");
    return { engine: "有道", text: result };
  }

  throw new Error("有道翻译返回格式错误");
}

// Microsoft Translate (free tier)
async function translateWithMicrosoft(text, from, to) {
  // Use Microsoft Edge's built-in translation API
  const url = `https://api-edge.cognitive.microsofttranslator.com/translate?api-version=3.0&from=${from === "auto" ? "" : from}&to=${to}`;

  const response = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/json"
    },
    body: JSON.stringify([{ Text: text }])
  });

  if (!response.ok) {
    throw new Error(`Microsoft翻译错误: ${response.status}`);
  }

  const data = await response.json();
  if (data[0] && data[0].translations && data[0].translations[0]) {
    return {
      engine: "Microsoft",
      text: data[0].translations[0].text
    };
  }

  throw new Error("Microsoft翻译返回格式错误");
}

// DeepL Translate
async function translateWithDeepL(text, from, to, config) {
  if (!config.engines.deepl.apiKey) {
    throw new Error("请先配置DeepL API Key");
  }

  const baseUrl = config.engines.deepl.pro
    ? "https://api.deepl.com/v2/translate"
    : "https://api-free.deepl.com/v2/translate";

  const params = new URLSearchParams({
    text: text,
    target_lang: to.toUpperCase(),
    source_lang: from === "auto" ? "" : from.toUpperCase()
  });

  const response = await fetch(baseUrl, {
    method: "POST",
    headers: {
      "Authorization": `DeepL-Auth-Key ${config.engines.deepl.apiKey}`,
      "Content-Type": "application/x-www-form-urlencoded"
    },
    body: params.toString()
  });

  if (!response.ok) {
    throw new Error(`DeepL翻译错误: ${response.status}`);
  }

  const data = await response.json();
  if (data.translations && data.translations[0]) {
    return {
      engine: "DeepL",
      text: data.translations[0].text
    };
  }

  throw new Error("DeepL翻译返回格式错误");
}

// DeepLX Translate (built-in, uses DeepL free API directly)
// Implements DeepLX algorithm: https://github.com/OwO-Network/DeepLX
async function translateWithDeepLX(text, from, to, config) {
  const apiKey = config.engines.deeplx?.apiKey;
  const usePro = config.engines.deeplx?.pro;
  const maxRetries = 3;

  // If API key provided, use official DeepL API
  if (apiKey) {
    const baseUrl = usePro
      ? "https://api.deepl.com/v2/translate"
      : "https://api-free.deepl.com/v2/translate";

    const response = await fetch(baseUrl, {
      method: "POST",
      headers: {
        "Authorization": `DeepL-Auth-Key ${apiKey}`,
        "Content-Type": "application/json"
      },
      body: JSON.stringify({
        text: [text],
        target_lang: to.toUpperCase(),
        source_lang: from === "auto" ? undefined : from.toUpperCase()
      })
    });

    if (!response.ok) {
      throw new Error(`DeepL API错误: ${response.status}`);
    }

    const data = await response.json();
    if (data.translations && data.translations[0]) {
      return {
        engine: "DeepLX",
        text: data.translations[0].text
      };
    }
    throw new Error("DeepL返回格式错误");
  }

  // Free mode: use DeepL's internal JSON-RPC API with DeepLX algorithm
  const sourceLang = from === "auto" ? "auto" : from.toUpperCase();
  const targetLang = to.toUpperCase();

  // DeepLX helper functions
  const getICount = (t) => (t.match(/i/g) || []).length;
  const getRandomNumber = () => {
    const base = Math.floor(Math.random() * 99999) + 100000;
    return base * 1000;
  };
  const getTimestamp = (iCount) => {
    const ts = Date.now();
    if (iCount !== 0) {
      const ic = iCount + 1;
      return ts - (ts % ic) + ic;
    }
    return ts;
  };
  const handlerBodyMethod = (randomId, body) => {
    const calc = (randomId + 5) % 29 === 0 || (randomId + 3) % 13 === 0;
    if (calc) {
      return body.replace('"method":"', '"method" : "');
    }
    return body.replace('"method":"', '"method": "');
  };

  let lastError = null;

  for (let attempt = 0; attempt < maxRetries; attempt++) {
    if (attempt > 0) {
      // Exponential backoff: 2s, 4s
      await new Promise(r => setTimeout(r, Math.pow(2, attempt) * 1000));
    }

    // Random jitter
    await new Promise(r => setTimeout(r, Math.random() * 400 + 100));

    const id = getRandomNumber();
    const iCount = getICount(text);
    const timestamp = getTimestamp(iCount);

    // Build request matching DeepLX format
    const postData = {
      jsonrpc: "2.0",
      method: "LMT_handle_texts",
      id: id,
      params: {
        splitting: "newlines",
        lang: {
          source_lang_user_selected: sourceLang,
          target_lang: targetLang
        },
        texts: [{
          text: text,
          requestAlternatives: 3
        }],
        timestamp: timestamp
      }
    };

    // Apply body manipulation like DeepLX
    let postStr = JSON.stringify(postData);
    postStr = handlerBodyMethod(id, postStr);

    try {
      const response = await fetch("https://www2.deepl.com/jsonrpc", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Accept": "*/*",
          "Accept-Language": "en-US,en;q=0.9",
          "Accept-Encoding": "gzip, deflate, br",
          "Origin": "https://www.deepl.com",
          "Referer": "https://www.deepl.com/",
          "Sec-Fetch-Dest": "empty",
          "Sec-Fetch-Mode": "cors",
          "Sec-Fetch-Site": "same-site",
          "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36 Edg/141.0.0.0"
        },
        body: postStr
      });

      // Rate limited - retry
      if (response.status === 429) {
        lastError = "Rate limited, retrying...";
        continue;
      }

      if (!response.ok) {
        throw new Error(`DeepL API错误: ${response.status}`);
      }

      const data = await response.json();

      if (data.error) {
        // Rate limit error code
        if (data.error.code === 1042911) {
          lastError = "Rate limited, retrying...";
          continue;
        }
        throw new Error(`DeepL错误: ${data.error.message || "Unknown error"}`);
      }

      if (data.result && data.result.texts) {
        const mainText = data.result.texts[0]?.text;
        if (mainText) {
          return {
            engine: "DeepLX",
            text: mainText
          };
        }
      }
    } catch (e) {
      lastError = e.message;
      continue;
    }
  }

  throw new Error(`DeepL限流，重试${maxRetries}次后失败: ${lastError || "Unknown"}`);
}

// ==================== MD5 Implementation (lightweight) ====================

async function md5(message) {
  const msgBuffer = new TextEncoder().encode(message);
  const hashBuffer = await crypto.subtle.digest("SHA-256", msgBuffer);
  // We'll use a simple hash instead of true MD5 for browser compatibility
  // For Youdao's web translate, we can use a simplified approach
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map(b => b.toString(16).padStart(2, "0")).join("").substring(0, 32);
}

// ==================== Glossary & Blacklist (local fallback) ====================

// Load synced glossary from chrome.storage.local.
// Format: { "en-zh": [{ source, target, context }, ...], ... }
async function getLocalGlossary() {
  try {
    const result = await chrome.storage.local.get("desktopGlossary");
    return result.desktopGlossary || {};
  } catch {
    return {};
  }
}

// Load synced blacklist from chrome.storage.local.
// Format: ["word1", "word2", ...]
async function getLocalBlacklist() {
  try {
    const result = await chrome.storage.local.get("desktopBlacklist");
    return result.desktopBlacklist || [];
  } catch {
    return [];
  }
}

// Check if text matches any blacklisted term (exact match, case-insensitive).
function isBlacklisted(text, blacklist) {
  const trimmed = text.trim().toLowerCase();
  return blacklist.some(word => word.toLowerCase() === trimmed);
}

// Apply glossary replacements to translated text.
// For each glossary entry where the source term appears in the original text,
// replace occurrences of the source term in the translated text with the target term.
// This handles cases where the translator left a term untranslated.
function applyGlossary(translatedText, originalText, glossary, from, to) {
  // Build lang-pair keys to check: exact match first, then wildcard
  const exactKey = `${from}-${to}`;
  const entries = glossary[exactKey] || [];

  // Also try entries keyed by auto-detected patterns
  const allEntries = [...entries];
  for (const [key, vals] of Object.entries(glossary)) {
    if (key !== exactKey && key.endsWith(`-${to}`)) {
      allEntries.push(...vals);
    }
  }

  let result = translatedText;
  for (const entry of allEntries) {
    // Only apply if the source term appears in the original text
    if (originalText.includes(entry.source)) {
      // Replace source term in translation with preferred target term
      result = result.split(entry.source).join(entry.target);
    }
  }
  return result;
}

// ==================== Main Translate Function ====================

async function translate(text, from, to) {
  // Try desktop bridge first if reachable (desktop handles glossary/blacklist/cache internally)
  if (DesktopBridge.reachable) {
    try {
      const result = await DesktopBridge.translateViaDesktop(text, from, to);
      return result;
    } catch (e) {
      DesktopBridge.reachable = false;
      console.warn("Desktop translation failed, falling back to local engines:", e.message);
    }
  }

  // Local fallback path — apply blacklist/glossary from synced desktop data
  const blacklist = await getLocalBlacklist();
  if (isBlacklisted(text, blacklist)) {
    return {
      results: [{ engine: "blacklist", text: text }],
      primary: { engine: "blacklist", text: text }
    };
  }

  const config = await getConfig();
  const results = [];
  const errors = [];

  const promises = [];

  // Google (always available)
  if (config.engines.google.enabled) {
    promises.push(
      translateWithGoogle(text, from, to)
        .then(r => results.push(r))
        .catch(e => errors.push({ engine: "Google", error: e.message }))
    );
  }

  // LLM
  if (config.engines.llm.enabled && config.engines.llm.apiKey) {
    promises.push(
      translateWithLLM(text, from, to, config)
        .then(r => results.push(r))
        .catch(e => errors.push({ engine: "LLM", error: e.message }))
    );
  }

  // Youdao
  if (config.engines.youdao.enabled) {
    promises.push(
      translateWithYoudao(text, from, to)
        .then(r => results.push(r))
        .catch(e => errors.push({ engine: "有道", error: e.message }))
    );
  }

  // Microsoft
  if (config.engines.microsoft.enabled) {
    promises.push(
      translateWithMicrosoft(text, from, to)
        .then(r => results.push(r))
        .catch(e => errors.push({ engine: "Microsoft", error: e.message }))
    );
  }

  // DeepL
  if (config.engines.deepl.enabled && config.engines.deepl.apiKey) {
    promises.push(
      translateWithDeepL(text, from, to, config)
        .then(r => results.push(r))
        .catch(e => errors.push({ engine: "DeepL", error: e.message }))
    );
  }

  // DeepLX
  if (config.engines.deeplx.enabled) {
    promises.push(
      translateWithDeepLX(text, from, to, config)
        .then(r => results.push(r))
        .catch(e => errors.push({ engine: "DeepLX", error: e.message }))
    );
  }

  await Promise.allSettled(promises);

  if (results.length === 0) {
    const errorMsg = errors.map(e => `${e.engine}: ${e.error}`).join("; ");
    throw new Error(errorMsg || "没有可用的翻译引擎");
  }

  // Apply glossary post-processing to each result
  const glossary = await getLocalGlossary();
  for (const r of results) {
    r.text = applyGlossary(r.text, text, glossary, from, to);
  }

  return {
    results: results,
    primary: results[0]
  };
}

// ==================== Message Handling ====================

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === "translate") {
    translate(message.text, message.from || "auto", message.to || "zh")
      .then(result => sendResponse({ success: true, ...result }))
      .catch(error => sendResponse({ success: false, error: error.message }));
    return true; // Keep channel open for async
  }

  if (message.type === "getConfig") {
    getConfig().then(config => sendResponse({ config }));
    return true;
  }

  if (message.type === "saveConfig") {
    saveConfig(message.config).then(() => sendResponse({ success: true }));
    return true;
  }

  if (message.type === "translatePage") {
    // Forward to content script
    chrome.tabs.sendMessage(sender.tab.id, { type: "translatePage" });
    sendResponse({ success: true });
    return false;
  }

  if (message.type === "restorePage") {
    chrome.tabs.sendMessage(sender.tab.id, { type: "restorePage" });
    sendResponse({ success: true });
    return false;
  }

  // Desktop batch page translation — content script sends segments, we route to desktop
  if (message.type === "translatePageDesktop") {
    if (!DesktopBridge.reachable) {
      sendResponse({ success: false, error: "Desktop not reachable" });
      return false;
    }
    DesktopBridge.translatePageViaDesktop(
      message.segments,
      message.from || "auto",
      message.to || "zh"
    )
      .then(translations => sendResponse({ success: true, translations }))
      .catch(error => sendResponse({ success: false, error: error.message }));
    return true;
  }

  // Desktop connection status query (for popup)
  if (message.type === "desktopStatus") {
    sendResponse({ reachable: DesktopBridge.reachable });
    return false;
  }

  // Manual health check trigger (for popup sync button)
  if (message.type === "checkDesktopHealth") {
    DesktopBridge.checkHealth().then(ok => sendResponse({ reachable: ok }));
    return true;
  }
});

// ==================== Context Menu ====================

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.create({
    id: "translate-selection",
    title: "翻译选中文本",
    contexts: ["selection"]
  });

  chrome.contextMenus.create({
    id: "translate-page",
    title: "翻译整页",
    contexts: ["page"]
  });

  chrome.contextMenus.create({
    id: "restore-page",
    title: "恢复原文",
    contexts: ["page"]
  });
});

chrome.contextMenus.onClicked.addListener((info, tab) => {
  if (info.menuItemId === "translate-selection") {
    chrome.tabs.sendMessage(tab.id, {
      type: "translate-selection",
      text: info.selectionText
    });
  } else if (info.menuItemId === "translate-page") {
    chrome.tabs.sendMessage(tab.id, { type: "translatePage" });
  } else if (info.menuItemId === "restore-page") {
    chrome.tabs.sendMessage(tab.id, { type: "restorePage" });
  }
});

// ==================== Keyboard Shortcuts ====================

chrome.commands?.onCommand?.addListener((command) => {
  if (command === "translate-selection") {
    chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
      if (tabs[0]) {
        chrome.tabs.sendMessage(tabs[0].id, { type: "getSelection" });
      }
    });
  }
});

console.log("Moon Translator service worker loaded");
