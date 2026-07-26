// Background service worker for Moon Translator
// Supports Chrome MV3 and Firefox MV3

// ==================== Desktop Bridge ====================
// Connects to the Tauri desktop app's local HTTP API for real translation.
// Falls back to local browser-based engines when desktop is unreachable.

const DESKTOP_URL = "http://127.0.0.1:60828";

/** Auth headers for desktop bridge (token from chrome.storage.local.desktopApiToken). */
async function desktopAuthHeaders(extra = {}) {
  const { desktopApiToken } = await chrome.storage.local.get(["desktopApiToken"]);
  const headers = { ...extra };
  if (desktopApiToken) {
    headers["Authorization"] = `Bearer ${desktopApiToken}`;
    headers["X-Api-Token"] = desktopApiToken;
  }
  return headers;
}

// ==================== Translation Cache ====================
// In-memory LRU cache shared across all tabs to avoid redundant API calls.

const TranslationCache = {
  maxSize: 1000,
  expiryMs: 24 * 60 * 60 * 1000, // 24 hours
  cache: new Map(),

  _makeKey(text, from, to, engine) {
    const normalized = text.trim().toLowerCase().replace(/\s+/g, " ");
    return `${engine || "any"}:${from || "auto"}:${to || "zh"}:${normalized}`;
  },

  get(text, from, to, engine) {
    const key = this._makeKey(text, from, to, engine);
    const entry = this.cache.get(key);
    if (!entry) return null;

    // Check expiry
    if (Date.now() - entry.timestamp > this.expiryMs) {
      this.cache.delete(key);
      return null;
    }

    // Move to end (most recently used)
    this.cache.delete(key);
    this.cache.set(key, entry);
    return entry.value;
  },

  set(text, from, to, engine, value) {
    const key = this._makeKey(text, from, to, engine);

    // Remove oldest if at capacity
    if (this.cache.size >= this.maxSize) {
      const firstKey = this.cache.keys().next().value;
      this.cache.delete(firstKey);
    }

    this.cache.set(key, { value, timestamp: Date.now() });
  },

  // Batch get: returns { hits: Map<text, result>, misses: string[] }
  batchGet(texts, from, to) {
    const hits = new Map();
    const misses = [];

    for (const text of texts) {
      // Try each enabled engine's cache
      let found = false;
      for (const engine of ["Google", "有道", "Microsoft", "LLM", "DeepL", "DeepLX"]) {
        const cached = this.get(text, from, to, engine);
        if (cached) {
          hits.set(text, cached);
          found = true;
          break;
        }
      }
      // Also try the "any" engine key (used by content script cache)
      if (!found) {
        const cached = this.get(text, from, to, "any");
        if (cached) {
          hits.set(text, cached);
          found = true;
        }
      }
      if (!found) {
        misses.push(text);
      }
    }

    return { hits, misses };
  }
};

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
      headers: await desktopAuthHeaders({ "Content-Type": "application/json" }),
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(15000)
    });

    if (!resp.ok) {
      const err = await resp.json().catch(() => ({}));
      throw new Error(err.message || err.error || `Desktop API error: ${resp.status}`);
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
      headers: await desktopAuthHeaders({ "Content-Type": "application/json" }),
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
    deeplx: { enabled: false, apiKey: "", pro: false },
    microsoft: { enabled: false }
  },
  targetLang: "zh",
  sourceLang: "auto",
  autoTranslate: false,
  showButton: true,
  hover: {
    enabled: true,
    delay: 300,
    minTextLength: 2,
    modifierKey: "none"
  }
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
  const sign = md5(signStr);
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

// ==================== MD5 Implementation (RFC 1321) ====================
// Pure JS MD5 since Web Crypto API does not support MD5.
// Required for Youdao translation signing which uses MD5.

function md5(string) {
  function md5cycle(x, k) {
    let a = x[0], b = x[1], c = x[2], d = x[3];
    a = ff(a, b, c, d, k[0], 7, -680876936);  d = ff(d, a, b, c, k[1], 12, -389564586);
    c = ff(c, d, a, b, k[2], 17, 606105819);   b = ff(b, c, d, a, k[3], 22, -1044525330);
    a = ff(a, b, c, d, k[4], 7, -176418897);   d = ff(d, a, b, c, k[5], 12, 1200080426);
    c = ff(c, d, a, b, k[6], 17, -1473231341); b = ff(b, c, d, a, k[7], 22, -45705983);
    a = ff(a, b, c, d, k[8], 7, 1770035416);   d = ff(d, a, b, c, k[9], 12, -1958414417);
    c = ff(c, d, a, b, k[10], 17, -42063);     b = ff(b, c, d, a, k[11], 22, -1990404162);
    a = ff(a, b, c, d, k[12], 7, 1804603682);  d = ff(d, a, b, c, k[13], 12, -40341101);
    c = ff(c, d, a, b, k[14], 17, -1502002290);b = ff(b, c, d, a, k[15], 22, 1236535329);
    a = gg(a, b, c, d, k[1], 5, -165796510);   d = gg(d, a, b, c, k[6], 9, -1069501632);
    c = gg(c, d, a, b, k[11], 14, 643717713);  b = gg(b, c, d, a, k[0], 20, -373897302);
    a = gg(a, b, c, d, k[5], 5, -701558691);   d = gg(d, a, b, c, k[10], 9, 38016083);
    c = gg(c, d, a, b, k[15], 14, -660478335); b = gg(b, c, d, a, k[4], 20, -405537848);
    a = gg(a, b, c, d, k[9], 5, 568446438);    d = gg(d, a, b, c, k[14], 9, -1019803690);
    c = gg(c, d, a, b, k[3], 14, -187363961);  b = gg(b, c, d, a, k[8], 20, 1163531501);
    a = gg(a, b, c, d, k[13], 5, -1444681467); d = gg(d, a, b, c, k[2], 9, -51403784);
    c = gg(c, d, a, b, k[7], 14, 1735328473);  b = gg(b, c, d, a, k[12], 20, -1926607734);
    a = hh(a, b, c, d, k[5], 4, -378558);      d = hh(d, a, b, c, k[8], 11, -2022574463);
    c = hh(c, d, a, b, k[11], 16, 1839030562); b = hh(b, c, d, a, k[14], 23, -35309556);
    a = hh(a, b, c, d, k[1], 4, -1530992060);  d = hh(d, a, b, c, k[4], 11, 1272893353);
    c = hh(c, d, a, b, k[7], 16, -155497632);  b = hh(b, c, d, a, k[10], 23, -1094730640);
    a = hh(a, b, c, d, k[13], 4, 681279174);   d = hh(d, a, b, c, k[0], 11, -358537222);
    c = hh(c, d, a, b, k[3], 16, -722521979);  b = hh(b, c, d, a, k[6], 23, 76029189);
    a = hh(a, b, c, d, k[9], 4, -640364487);   d = hh(d, a, b, c, k[12], 11, -421815835);
    c = hh(c, d, a, b, k[15], 16, 530742520);  b = hh(b, c, d, a, k[2], 23, -995338651);
    a = ii(a, b, c, d, k[0], 6, -198630844);   d = ii(d, a, b, c, k[7], 10, 1126891415);
    c = ii(c, d, a, b, k[14], 15, -1416354905);b = ii(b, c, d, a, k[5], 21, -57434055);
    a = ii(a, b, c, d, k[12], 6, 1700485571);  d = ii(d, a, b, c, k[3], 10, -1894986606);
    c = ii(c, d, a, b, k[10], 15, -1051523);   b = ii(b, c, d, a, k[1], 21, -2054922799);
    a = ii(a, b, c, d, k[8], 6, 1873313359);   d = ii(d, a, b, c, k[15], 10, -30611744);
    c = ii(c, d, a, b, k[6], 15, -1560198380); b = ii(b, c, d, a, k[13], 21, 1309151649);
    a = ii(a, b, c, d, k[4], 6, -145523070);   d = ii(d, a, b, c, k[11], 10, -1120210379);
    c = ii(c, d, a, b, k[2], 15, 718787259);   b = ii(b, c, d, a, k[9], 21, -343485551);
    x[0] = add32(a, x[0]); x[1] = add32(b, x[1]);
    x[2] = add32(c, x[2]); x[3] = add32(d, x[3]);
  }
  function cmn(q, a, b, x, s, t) {
    a = add32(add32(a, q), add32(x, t));
    return add32((a << s) | (a >>> (32 - s)), b);
  }
  function ff(a, b, c, d, x, s, t) { return cmn((b & c) | (~b & d), a, b, x, s, t); }
  function gg(a, b, c, d, x, s, t) { return cmn((b & d) | (c & ~d), a, b, x, s, t); }
  function hh(a, b, c, d, x, s, t) { return cmn(b ^ c ^ d, a, b, x, s, t); }
  function ii(a, b, c, d, x, s, t) { return cmn(c ^ (b | ~d), a, b, x, s, t); }
  function md5blk(s) {
    const md5blks = [];
    for (let i = 0; i < 64; i += 4) {
      md5blks[i >> 2] = s.charCodeAt(i) + (s.charCodeAt(i + 1) << 8) +
        (s.charCodeAt(i + 2) << 16) + (s.charCodeAt(i + 3) << 24);
    }
    return md5blks;
  }
  function add32(a, b) { return (a + b) & 0xFFFFFFFF; }
  function rhex(n) {
    const hc = "0123456789abcdef";
    let s = "";
    for (let j = 0; j < 4; j++) {
      s += hc.charAt((n >> (j * 8 + 4)) & 0x0F) + hc.charAt((n >> (j * 8)) & 0x0F);
    }
    return s;
  }

  // Convert UTF-8 string to byte string for MD5 input
  function utf8Encode(str) {
    const bytes = [];
    for (let i = 0; i < str.length; i++) {
      let c = str.charCodeAt(i);
      if (c < 128) {
        bytes.push(String.fromCharCode(c));
      } else if (c < 2048) {
        bytes.push(String.fromCharCode((c >> 6) | 192));
        bytes.push(String.fromCharCode((c & 63) | 128));
      } else {
        bytes.push(String.fromCharCode((c >> 12) | 224));
        bytes.push(String.fromCharCode(((c >> 6) & 63) | 128));
        bytes.push(String.fromCharCode((c & 63) | 128));
      }
    }
    return bytes.join("");
  }

  const s = utf8Encode(string);
  let n = s.length;
  let state = [1732584193, -271733879, -1732584194, 271733878];
  let i;
  for (i = 64; i <= n; i += 64) {
    md5cycle(state, md5blk(s.substring(i - 64, i)));
  }
  const tail = Array(16).fill(0);
  const remaining = s.substring(i - 64);
  for (i = 0; i < remaining.length; i++) {
    tail[i >> 2] |= remaining.charCodeAt(i) << ((i % 4) << 3);
  }
  tail[i >> 2] |= 0x80 << ((i % 4) << 3);
  if (i > 55) {
    md5cycle(state, tail);
    for (i = 0; i < 16; i++) tail[i] = 0;
  }
  tail[14] = n * 8;
  md5cycle(state, tail);
  return rhex(state[0]) + rhex(state[1]) + rhex(state[2]) + rhex(state[3]);
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
  // Check service worker cache first (shared across all tabs)
  const cached = TranslationCache.get(text, from, to, "any");
  if (cached) {
    return cached;
  }

  // Try desktop bridge first if reachable (desktop handles glossary/blacklist/cache internally)
  if (DesktopBridge.reachable) {
    try {
      const result = await DesktopBridge.translateViaDesktop(text, from, to);
      // Cache the result
      TranslationCache.set(text, from, to, "desktop", result);
      TranslationCache.set(text, from, to, "any", result);
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

  const finalResult = {
    results: results,
    primary: results[0]
  };

  // Cache the result
  TranslationCache.set(text, from, to, "any", finalResult);

  return finalResult;
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
