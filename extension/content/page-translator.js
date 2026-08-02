// Content script: Full page translator for Moon Translator
// Bilingual display (B1-B6): font wrapper + dual-language clone + filter +
// fooCount guard + piece splitting + dualStyle modes.
//
// Mode: "replace" (default, 替换原文) | "bilingual" (双语对照)
// Style: "underline" | "highlight" | "weakening" | "mask" (bilingual 模式下生效)

(function() {
  "use strict";

  // ==================== State ====================

  let isTranslated = false;
  let isProcessing = false;
  let translateBtn = null;

  // B5 fooCount guard: 每次 translatePage/restorePage 自增, 异步结果校验
  let translateGeneration = 0;

  // B1 nodesToRestore: 记录所有被翻译的节点, 用于 restore
  const nodesToRestore = [];

  // Cache + worker
  let cache = null;
  let workerManager = null;

  // IntersectionObserver for lazy loading
  let intersectionObserver = null;
  const visibleNodes = new Set();
  const pendingNodes = new Set();

  // Batch translation queue
  let translationQueue = [];
  let batchTimeout = null;
  const BATCH_DELAY_MS = 100;
  const MAX_BATCH_SIZE = 10;

  // B6 Piece 切分
  const PIECE_MAX_CHARS = 1000;
  const INLINE_TAGS = new Set([
    "a", "abbr", "b", "bdi", "bdo", "cite", "code", "data",
    "dfn", "em", "i", "kbd", "mark", "q", "rp", "rt", "ruby",
    "s", "samp", "small", "span", "strong", "sub", "sup",
    "time", "u", "var", "wbr", "#text",
  ]);
  const SKIP_TAGS = new Set([
    "script", "style", "noscript", "code", "pre", "svg",
    "textarea", "input", "select", "option", "button",
    "iframe", "canvas", "video", "audio", "object",
    // Tier4-1: structural chrome tags — nav/footer/header/aside/form
    // are almost never main content; translating them adds noise.
    "nav", "footer", "header", "aside", "form", "figcaption",
  ]);

  // Tier4-1: ARIA landmarks that signal chrome, not content.
  // Used by shouldSkipElement via closest('[role="..."]').
  const SKIP_ROLES = new Set([
    "navigation", "banner", "contentinfo", "search", "complementary",
    "form", "menu", "menubar", "tablist", "toolbar",
  ]);

  // B4 dualStyle: 从 storage 加载
  let bilingualMode = "replace"; // "replace" | "bilingual"
  let dualStyle = "underline"; // "underline" | "highlight" | "weakening" | "mask"

  // ==================== Config ====================

  async function loadDisplayConfig() {
    try {
      const { config } = await chrome.storage.local.get("config");
      if (config?.pageTranslation) {
        bilingualMode = config.pageTranslation.mode || "replace";
        dualStyle = config.pageTranslation.dualStyle || "underline";
      }
    } catch {
      // storage 不可用时用默认值
    }
  }

  // ==================== B3: Filter ====================

  function shouldSkipElement(el) {
    if (!el || el.nodeType !== Node.ELEMENT_NODE) return false;
    if (SKIP_TAGS.has(el.tagName.toLowerCase())) return true;
    // B3: notranslate 属性
    if (el.closest("[notranslate]")) return true;
    // B3: translate="no"
    if (el.getAttribute("translate") === "no") return true;
    // B3: contenteditable
    if (el.isContentEditable) return true;
    // B3: data-translationmark="copiedNode" (避免翻译克隆节点)
    if (el.closest('[data-translationmark="copiedNode"]')) return true;
    // Tier4-1: ARIA landmark roles for chrome regions
    const role = el.getAttribute("role");
    if (role && SKIP_ROLES.has(role)) return true;
    const roleParent = el.closest("[role]");
    if (roleParent) {
      const pr = roleParent.getAttribute("role");
      if (pr && SKIP_ROLES.has(pr)) return true;
    }
    // 跳过 moon translator 自身元素
    if (el.id?.startsWith("moon-") || el.closest("#moon-translator-popup") ||
        el.closest("#moon-translate-page-btn") || el.closest("#moon-translate-progress")) {
      return true;
    }
    return false;
  }

  // ==================== Tier4-4: specialRules 站点适配 ====================
  //
  // 高频站点的专门规则：每个规则定义 host 匹配模式和特殊容器选择器。
  // findMainContainer() 会优先尝试匹配的 specialRule，找不到才走通用启发式。
  //
  // 为什么需要：通用 <p> 聚类对 SPA 站点（Twitter/Reddit 等）效果差，
  // 因为这些站点用 div 渲染文本、shadow DOM、懒加载等技巧。
  // 硬编码选择器更可靠，且能在站点改版时集中维护。

  const SPECIAL_RULES = [
    {
      name: "twitter",
      hostMatch: /(^|\.)twitter\.com$|(^|\.)x\.com$/,
      // Twitter 用 article[data-testid="tweetText"] 渲染推文
      containerSelector: 'article[data-testid="tweetText"], [data-testid="tweetText"]',
      // Twitter 是强 SPA，需要监听路由变化
      isSPA: true,
    },
    {
      name: "reddit",
      hostMatch: /(^|\.)reddit\.com$/,
      // Reddit 新版用 shreddit-post 或 RichTextJSONElement
      containerSelector: 'shreddit-post, [data-testid="post-content"], .RichTextJSON-root',
      isSPA: true,
    },
    {
      name: "github",
      hostMatch: /(^|\.)github\.com$/,
      // GitHub: README/Issue/PR 正文
      containerSelector: '.markdown-body, .comment-body, .blob-code-inner',
      isSPA: false,
    },
    {
      name: "youtube",
      hostMatch: /(^|\.)youtube\.com$/,
      // YouTube: 视频描述和评论
      containerSelector: '#description-inner, #content-text, yt-formatted-string#content-text',
      isSPA: true,
    },
    {
      name: "zhihu",
      hostMatch: /(^|\.)zhihu\.com$/,
      // 知乎专栏和回答
      containerSelector: '.RichText, .Post-RichText, .AnswerItem .RichText',
      isSPA: false,
    },
    {
      name: "wechat",
      hostMatch: /(^|\.)weixin\.qq\.com$/,
      // 微信公众号文章
      containerSelector: '#js_content, .rich_media_content',
      isSPA: false,
    },
  ];

  function matchSpecialRule() {
    const host = location.hostname;
    for (const rule of SPECIAL_RULES) {
      if (rule.hostMatch.test(host)) {
        return rule;
      }
    }
    return null;
  }

  // ==================== Tier4-1: Main container heuristic ====================
  //
  // Find the element that most likely contains the page's main article text.
  // Strategy (old-immersive-translate enhance.js:488-571 adapted):
  //   1. <article> / <main> — direct win if it has substantial text
  //   2. [role="main"] / [role="article"] — ARIA landmarks
  //   3. Largest <p> cluster: find the <div>/<section> that contains the most
  //      <p> elements whose total text length exceeds 40% of the page's
  //      visible <p> text. Climb from the largest <p> to its ancestor.
  //   4. Fallback: document.body (old behavior — translate everything)
  //
  // This reduces noise from nav/footer/sidebar translation and speeds up
  // large pages by skipping chrome that users don't want translated.

  function visibleTextLength(el) {
    let len = 0;
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        const p = node.parentElement;
        if (!p || shouldSkipElement(p)) return NodeFilter.FILTER_REJECT;
        if (p.offsetParent === null && p.tagName !== "BODY") return NodeFilter.FILTER_REJECT;
        const t = node.textContent.trim();
        return t ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
      },
    });
    while (walker.nextNode()) len += walker.currentNode.textContent.trim().length;
    return len;
  }

  function findMainContainer() {
    // Tier4-4: 0. specialRules — site-specific selectors first
    const rule = matchSpecialRule();
    if (rule) {
      const el = document.querySelector(rule.containerSelector);
      if (el && visibleTextLength(el) >= 50) {
        console.debug("[Moon] Tier4-4: matched specialRule", rule.name);
        return el;
      }
      // 站点规则匹配但元素不存在/空 — 可能 SPA 还没渲染，继续走通用逻辑
    }

    // 1. <article> / <main> — direct win if substantial
    for (const sel of ["article", "main", "[role='main']", "[role='article']"]) {
      const el = document.querySelector(sel);
      if (el && visibleTextLength(el) >= 200) {
        return el;
      }
    }

    // 2. Largest <p> cluster heuristic
    const paragraphs = Array.from(document.querySelectorAll("p"));
    if (paragraphs.length < 3) return document.body;

    // Total visible <p> text length
    const totalPText = paragraphs.reduce(
      (sum, p) => sum + (p.offsetParent !== null ? p.textContent.trim().length : 0),
      0,
    );
    if (totalPText < 200) return document.body;

    // Group <p> by their nearest common ancestor (climb 1-3 levels)
    const buckets = new Map(); // element → { count, len }
    for (const p of paragraphs) {
      if (p.offsetParent === null) continue;
      let ancestor = p.parentElement;
      // Climb up to 3 levels to find a meaningful container
      for (let i = 0; i < 3 && ancestor && ancestor !== document.body; i++) {
        const next = ancestor.parentElement;
        if (!next || next === document.body) break;
        // Stop climbing if we hit a structural boundary
        const tag = ancestor.tagName.toLowerCase();
        if (tag === "article" || tag === "main" || tag === "section") break;
        ancestor = next;
      }
      const key = ancestor || document.body;
      const entry = buckets.get(key) || { count: 0, len: 0 };
      entry.count += 1;
      entry.len += p.textContent.trim().length;
      buckets.set(key, entry);
    }

    // Find the bucket with the most text; require >= 40% of total
    let best = document.body;
    let bestLen = 0;
    const threshold = totalPText * 0.4;
    for (const [el, { count, len }] of buckets) {
      if (count >= 3 && len >= threshold && len > bestLen) {
        best = el;
        bestLen = len;
      }
    }
    return best;
  }

  // TreeWalker acceptNode
  function acceptNode(node) {
    const parent = node.parentElement;
    if (!parent) return NodeFilter.FILTER_REJECT;
    if (shouldSkipElement(parent)) return NodeFilter.FILTER_REJECT;
    if (parent.offsetParent === null && parent.tagName !== "BODY") {
      return NodeFilter.FILTER_REJECT;
    }
    if (!node.textContent.trim()) return NodeFilter.FILTER_REJECT;
    return NodeFilter.FILTER_ACCEPT;
  }

  // ==================== B6: Piece 切分 ====================

  // Tier4-1: cache main container per translation pass; re-detect on each
  // translatePage() call because SPAs may swap content between invocations.
  let mainContainerCache = null;

  function getTextNodes() {
    const root = mainContainerCache || (mainContainerCache = findMainContainer());
    if (root === document.body) {
      // Fallback path — translate everything (old behavior)
    }
    const walker = document.createTreeWalker(
      root, NodeFilter.SHOW_TEXT, { acceptNode }
    );
    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);
    return nodes;
  }

  // 将文本节点按 block 分组, 合并 inline 兄弟, 超长自动切分
  function buildPieces(textNodes) {
    const pieces = [];
    let current = null; // { nodes: [], text: "" }

    for (const node of textNodes) {
      const parent = node.parentElement;
      if (!parent) continue;

      // 判断父元素是 block 还是 inline
      const tag = parent.tagName.toLowerCase();
      const isInline = INLINE_TAGS.has(tag);

      if (current && isInline && current.text.length + node.textContent.length <= PIECE_MAX_CHARS) {
        // 追加到当前 piece
        current.nodes.push(node);
        current.text += node.textContent;
      } else {
        // 开始新 piece
        if (current && current.text.trim()) pieces.push(current);
        current = { nodes: [node], text: node.textContent };
      }
    }
    if (current && current.text.trim()) pieces.push(current);

    // B6: 超长 piece 按 1000 字符切分
    const finalPieces = [];
    for (const piece of pieces) {
      if (piece.text.length <= PIECE_MAX_CHARS) {
        finalPieces.push(piece);
      } else {
        splitPiece(piece, finalPieces);
      }
    }
    return finalPieces;
  }

  function splitPiece(piece, out) {
    // 按句子边界切分
    const sentences = piece.text.split(/(?<=[.!?。！？；\n])\s*/);
    let current = { nodes: [], text: "" };

    for (const sentence of sentences) {
      if (current.text.length + sentence.length > PIECE_MAX_CHARS && current.text.trim()) {
        out.push(current);
        current = { nodes: [], text: "" };
      }
      current.text += sentence;
      // 节点映射简化: 第一个 piece 拿所有节点, 后续空
      if (current.nodes.length === 0) {
        current.nodes = piece.nodes.slice(0, 1);
      }
    }
    if (current.text.trim()) out.push(current);
  }

  // ==================== B1+B2: Apply Translation ====================

  // B1: replace 模式 — 用 span 包裹译文, 原文存 data 属性
  function applyReplace(piece, translated) {
    const firstNode = piece.nodes[0];
    if (!firstNode.parentElement) return;

    // 创建译文 span (B1 font wrapper)
    const span = document.createElement("span");
    span.className = "moon-translated-text";
    span.textContent = translated;
    span.setAttribute("data-moon-original", piece.text.trim());

    // 在第一个节点前插入译文
    firstNode.parentElement.insertBefore(span, firstNode);

    // 清空原文节点
    for (const node of piece.nodes) {
      nodesToRestore.push({ node, originalText: node.textContent });
      node.textContent = "";
    }

    // 记录 span 用于 restore
    nodesToRestore.push({ span, type: "remove" });
  }

  // B2: bilingual 模式 — 原文保留 + 译文插入
  function applyBilingual(piece, translated) {
    const firstNode = piece.nodes[0];
    const parent = firstNode.parentElement;
    if (!parent) return;

    // B2: 克隆原文容器
    const clone = parent.cloneNode(true);
    clone.classList.add("moon-original-clone");
    clone.setAttribute("notranslate", "");
    clone.setAttribute("data-translationmark", "copiedNode");
    clone.classList.add(`moon-dual-style-${dualStyle}`);

    // 创建译文容器
    const transWrapper = document.createElement(parent.tagName.toLowerCase() || "div");
    transWrapper.className = `moon-translated-wrapper moon-dual-style-${dualStyle}`;
    transWrapper.textContent = translated;
    transWrapper.setAttribute("data-moon-mode", "bilingual");

    // 在 parent 之后插入译文
    if (parent.nextSibling) {
      parent.parentNode.insertBefore(transWrapper, parent.nextSibling);
    } else {
      parent.parentNode.appendChild(transWrapper);
    }

    // 记录用于 restore
    nodesToRestore.push({ span: transWrapper, type: "remove" });
    // 清空原文 (clone 已保留原文)
    for (const node of piece.nodes) {
      nodesToRestore.push({ node, originalText: node.textContent });
      node.textContent = "";
    }
  }

  function applyTranslation(piece, translated) {
    if (bilingualMode === "bilingual") {
      applyBilingual(piece, translated);
    } else {
      applyReplace(piece, translated);
    }
  }

  // ==================== B5: fooCount Guard ====================

  function startGeneration() {
    translateGeneration++;
    return translateGeneration;
  }

  function isStale(gen) {
    return gen !== translateGeneration;
  }

  // ==================== Batch Translation ====================

  function sendMessage(message) {
    return new Promise((resolve, reject) => {
      try {
        chrome.runtime.sendMessage(message, (response) => {
          if (chrome.runtime.lastError) {
            reject(new Error(chrome.runtime.lastError.message));
          } else {
            resolve(response);
          }
        });
      } catch (e) {
        reject(e);
      }
    });
  }

  async function translateBatch(texts, from, to) {
    if (texts.length === 0) return [];
    const results = new Array(texts.length);
    const uncachedTexts = [];
    const uncachedIndices = [];

    if (cache) {
      for (let i = 0; i < texts.length; i++) {
        // B7: 先查同步层 (L1+L2), 未命中再查 IndexedDB (L3)
        let cached = cache.getSync(texts[i], from, to);
        if (cached === null && typeof cache.get === "function") {
          cached = await cache.get(texts[i], from, to);
        }
        if (cached) {
          results[i] = cached;
        } else {
          uncachedTexts.push(texts[i]);
          uncachedIndices.push(i);
        }
      }
    } else {
      uncachedTexts.push(...texts);
      uncachedIndices.push(...texts.map((_, i) => i));
    }

    if (uncachedTexts.length === 0) return results;

    // 尝试桌面 batch
    try {
      const desktopResults = await translateBatchDesktop(uncachedTexts, from, to);
      if (desktopResults) {
        for (let i = 0; i < uncachedTexts.length; i++) {
          const idx = uncachedIndices[i];
          results[idx] = desktopResults[i];
          if (cache && desktopResults[i]) {
            cache.setSync(uncachedTexts[i], from, to, desktopResults[i]);
          }
        }
        return results;
      }
    } catch (e) {
      console.warn("Desktop batch failed:", e);
    }

    // B8: 尝试 LLM batch (separator protocol)
    try {
      const llmResults = await translateBatchLLM(uncachedTexts, from, to);
      if (llmResults) {
        for (let i = 0; i < uncachedTexts.length; i++) {
          const idx = uncachedIndices[i];
          results[idx] = llmResults[i];
          if (cache && llmResults[i]) {
            cache.setSync(uncachedTexts[i], from, to, llmResults[i]);
          }
        }
        return results;
      }
    } catch (e) {
      console.warn("LLM batch failed:", e);
    }

    // Fallback: 逐条翻译
    const promises = uncachedTexts.map(async (text, i) => {
      try {
        const response = await sendMessage({ type: "translate", text, from, to });
        const idx = uncachedIndices[i];
        results[idx] = response;
        if (cache && response) cache.setSync(text, from, to, response);
      } catch {
        // 保留 undefined
      }
    });
    await Promise.all(promises);
    return results;
  }

  // B8: LLM batch translation via service-worker separator protocol
  async function translateBatchLLM(texts, from, to) {
    if (texts.length === 0) return null;
    try {
      const response = await sendMessage({
        type: "translateBatchLLM",
        texts: texts.map(t => t.trim()).filter(t => t.length >= 2),
        from: from || "auto",
        to: to || "zh",
      });
      if (!response.success) return null;
      const translations = response.translations;
      if (!translations || translations.length === 0) return null;

      const results = new Array(texts.length);
      for (const t of translations) {
        if (t.translated) {
          results[t.index] = {
            primary: { engine: "llm-batch", text: t.translated },
            results: [{ engine: "llm-batch", text: t.translated }],
          };
        }
      }
      return results;
    } catch {
      return null;
    }
  }

  async function translateBatchDesktop(texts, from, to) {
    const segments = texts.map((text, index) => ({ text: text.trim(), index }))
      .filter(s => s.text.length >= 2);
    if (segments.length === 0) return null;

    const response = await sendMessage({
      type: "translatePageDesktop",
      segments, from: from || "auto", to: to || "zh",
    });
    if (!response.success) return null;

    const translations = response.translations;
    if (!translations || translations.length === 0) return null;

    const results = new Array(texts.length);
    for (const t of translations) {
      if (t.translated) {
        results[t.index] = {
          primary: { engine: "desktop", text: t.translated },
          results: [{ engine: "desktop", text: t.translated }],
        };
      }
    }
    return results;
  }

  // ==================== Translation Flow ====================

  async function translatePage() {
    if (isProcessing) return;
    isProcessing = true;
    const gen = startGeneration();
    showProgress(0, 1);

    // Tier4-1: re-detect main container on each translation pass (SPA navigations
    // may replace the article element between invocations).
    mainContainerCache = null;
    const detectedRoot = findMainContainer();
    mainContainerCache = detectedRoot;
    if (detectedRoot !== document.body) {
      console.debug("[Moon] Tier4-1: detected main container", detectedRoot.tagName, detectedRoot.id || detectedRoot.className);
    }

    const textNodes = getTextNodes();
    const pieces = buildPieces(textNodes);

    setupIntersectionObserver();

    // 分离可见 / 不可见 piece
    const visibleNow = [];
    const deferred = [];
    for (const piece of pieces) {
      const parent = piece.nodes[0]?.parentElement;
      if (parent && isElementInViewport(parent)) {
        visibleNow.push(piece);
      } else {
        deferred.push(piece);
        piece.nodes.forEach(n => pendingNodes.add(n));
      }
    }

    const total = pieces.length;
    let processed = 0;

    // 翻译可见 piece (batch)
    const batchSize = MAX_BATCH_SIZE;
    for (let i = 0; i < visibleNow.length; i += batchSize) {
      if (isStale(gen)) { isProcessing = false; return; }

      const batch = visibleNow.slice(i, i + batchSize);
      const texts = batch.map(p => p.text.trim()).filter(t => t.length >= 2);
      const results = await translateBatch(texts, "auto", "zh");

      for (let j = 0; j < batch.length; j++) {
        if (isStale(gen)) { isProcessing = false; return; }
        const result = results[j];
        if (result) {
          const translated = result.primary?.text || result.results?.[0]?.text;
          if (translated) applyTranslation(batch[j], translated);
        }
        processed++;
      }
      updateProgress(processed, total);
    }

    // 延迟 piece 翻译 (IntersectionObserver 触发)
    observePieces(deferred, gen);

    isProcessing = false;
    setTimeout(hideProgress, 1000);
  }

  function observePieces(pieces, gen) {
    for (const piece of pieces) {
      const parent = piece.nodes[0]?.parentElement;
      if (parent) {
        piece._gen = gen;
        intersectionObserver.observe(parent);
        // 存储映射
        parent._moonPiece = piece;
      }
    }
  }

  function setupIntersectionObserver() {
    if (intersectionObserver) intersectionObserver.disconnect();
    intersectionObserver = new IntersectionObserver((entries) => {
      entries.forEach(entry => {
        if (entry.isIntersecting) {
          const el = entry.target;
          const piece = el._moonPiece;
          if (piece && !piece._translated) {
            piece._translated = true;
            translateDeferredPiece(piece);
          }
        }
      });
    }, { root: null, rootMargin: "200px", threshold: 0.1 });
  }

  async function translateDeferredPiece(piece) {
    if (isStale(piece._gen)) return;
    const text = piece.text.trim();
    if (text.length < 2) return;

    try {
      const results = await translateBatch([text], "auto", "zh");
      if (isStale(piece._gen)) return;
      const result = results[0];
      if (result) {
        const translated = result.primary?.text || result.results?.[0]?.text;
        if (translated) applyTranslation(piece, translated);
      }
    } catch (e) {
      console.warn("Deferred translation failed:", e);
    }
  }

  // ==================== Restore ====================

  function restorePage() {
    startGeneration(); // B5: 使所有在途异步结果失效
    if (intersectionObserver) intersectionObserver.disconnect();
    visibleNodes.clear();
    pendingNodes.clear();

    translationQueue = [];
    if (batchTimeout) { clearTimeout(batchTimeout); batchTimeout = null; }

    // 逆序 restore (DOM 操作从后往前, 避免索引错位)
    for (let i = nodesToRestore.length - 1; i >= 0; i--) {
      const entry = nodesToRestore[i];
      if (entry.span && entry.type === "remove") {
        entry.span.remove();
      } else if (entry.node && entry.node.parentElement) {
        entry.node.textContent = entry.originalText;
      }
    }
    nodesToRestore.length = 0;
  }

  // ==================== Toggle ====================

  async function togglePageTranslation() {
    if (isTranslated) {
      restorePage();
      isTranslated = false;
      if (translateBtn) {
        translateBtn.classList.remove("active");
        translateBtn.title = "翻译整页";
      }
    } else {
      if (translateBtn) {
        translateBtn.classList.add("active");
        translateBtn.title = "恢复原文";
      }
      await translatePage();
      isTranslated = true;
    }
  }

  // ==================== UI ====================
  //
  // Tier4-2: Closed Shadow DOM isolation
  // --------------------------------------
  // The translate button and progress bar are wrapped in closed shadow roots
  // to prevent host-page CSS from leaking into our UI. A closed shadow root
  // is opaque to `element.shadowRoot` (returns null) and to external
  // querySelector, so hostile/buggy stylesheets can't reach inside.
  //
  // Layout:
  //   <div id="moon-translate-page-btn">          ← host (positioning anchor)
  //     #shadow-root (closed)
  //       <style> ... button styles ... </style>
  //       <button class="mt-btn-inner">译</button>
  //   </div>
  //
  // The host element keeps its id so e2e tests (getElementById) and the
  // external CSS (host :hover) still work. The visible button content lives
  // inside the shadow root where only our own <style> can reach it.

  const TRANSLATE_BTN_STYLE = `
    .mt-btn-inner {
      width: 100%; height: 100%;
      border: none; outline: none;
      background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
      color: white;
      border-radius: 50%;
      display: flex; align-items: center; justify-content: center;
      font-size: 20px; font-weight: bold; cursor: pointer;
      transition: transform 0.2s, box-shadow 0.2s, background 0.2s;
      user-select: none; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }
    .mt-btn-inner:hover {
      transform: scale(1.1);
      box-shadow: 0 6px 16px rgba(102, 126, 234, 0.5);
    }
    :host(.active) .mt-btn-inner {
      background: linear-gradient(135deg, #48bb78 0%, #38a169 100%);
      box-shadow: 0 4px 12px rgba(72, 187, 120, 0.4);
    }
  `;

  const PROGRESS_STYLE = `
    .mt-progress-inner {
      background: rgba(0,0,0,0.8); color: white; padding: 8px 16px;
      border-radius: 20px; font-size: 13px;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }
  `;

  function createTranslateButton() {
    if (document.getElementById("moon-translate-page-btn")) return;
    const host = document.createElement("div");
    host.id = "moon-translate-page-btn";
    // Host acts as positioning anchor — external CSS sets position/size/z-index
    // via #moon-translate-page-btn selector in selector.css.
    const shadow = host.attachShadow({ mode: "closed" });
    const style = document.createElement("style");
    style.textContent = TRANSLATE_BTN_STYLE;
    const btn = document.createElement("button");
    btn.className = "mt-btn-inner";
    btn.textContent = "译";
    btn.title = "翻译整页 (点击切换)";
    btn.addEventListener("click", togglePageTranslation);
    shadow.append(style, btn);
    document.body.appendChild(host);
    translateBtn = host; // togglePageTranslation reads .active on host via :host(.active)
  }

  let progressHost = null;
  let progressInner = null; // captured reference into closed shadow root
  function showProgress(current, total) {
    if (!progressHost) {
      progressHost = document.createElement("div");
      progressHost.id = "moon-translate-progress";
      progressHost.style.cssText = `
        position: fixed; top: 20px; left: 50%; transform: translateX(-50%);
        z-index: 2147483647;
      `;
      const shadow = progressHost.attachShadow({ mode: "closed" });
      const style = document.createElement("style");
      style.textContent = PROGRESS_STYLE;
      progressInner = document.createElement("div");
      progressInner.className = "mt-progress-inner";
      shadow.append(style, progressInner);
      document.body.appendChild(progressHost);
    }
    // Closed shadow root → can't query via shadowRoot, use captured reference.
    progressInner.textContent = `翻译中... ${current}/${total}`;
  }
  function updateProgress(c, t) { showProgress(c, t); }
  function hideProgress() {
    if (progressHost) { progressHost.remove(); progressHost = null; progressInner = null; }
  }

  function isElementInViewport(el) {
    const rect = el.getBoundingClientRect();
    return rect.top < (window.innerHeight || document.documentElement.clientHeight) + 200 &&
           rect.bottom > -200 &&
           rect.left < (window.innerWidth || document.documentElement.clientWidth) + 200 &&
           rect.right > -200;
  }

  // ==================== SPA MutationObserver ====================

  function setupObserver() {
    const observer = new MutationObserver((mutations) => {
      if (!isTranslated || isProcessing) return;
      for (const mutation of mutations) {
        for (const node of mutation.addedNodes) {
          if (node.nodeType !== Node.ELEMENT_NODE) continue;
          if (shouldSkipElement(node)) continue;
          // B5: 新增节点异步翻译
          const gen = translateGeneration;
          setTimeout(() => {
            if (isStale(gen) || !isTranslated) return;
            translateNewElement(node, gen);
          }, 100);
        }
      }
    });
    // Tier4-1: observe the detected main container if available, else body.
    // Narrowing the observer scope reduces callback fires from SPA chrome
    // (nav menus, ads, comment widgets) that users don't want translated.
    const observeRoot = mainContainerCache || document.body;
    observer.observe(observeRoot, { childList: true, subtree: true });
  }

  async function translateNewElement(el, gen) {
    const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, { acceptNode });
    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);
    if (nodes.length === 0) return;

    const pieces = buildPieces(nodes);
    for (const piece of pieces) {
      if (isStale(gen)) return;
      const text = piece.text.trim();
      if (text.length < 2) continue;
      try {
        const results = await translateBatch([text], "auto", "zh");
        if (isStale(gen)) return;
        const result = results[0];
        if (result) {
          const translated = result.primary?.text || result.results?.[0]?.text;
          if (translated) applyTranslation(piece, translated);
        }
      } catch { /* ignore */ }
    }
  }

  // ==================== Init ====================

  async function initModules() {
    if (window.MoonTranslationCache) cache = window.MoonTranslationCache;
    if (window.MoonWorkerManager) {
      workerManager = window.MoonWorkerManager;
      await workerManager.initPromise;
    }
  }

  createTranslateButton();
  setupObserver();

  // Tier4-4: SPA route change detection for specialRule sites.
  // 当 specialRule 标记 isSPA=true 时，hook pushState/replaceState 和 popstate，
  // 路由变化后重置 mainContainerCache 让下次翻译重新检测容器。
  (function setupSPARouteDetection() {
    const rule = matchSpecialRule();
    if (!rule || !rule.isSPA) return;

    let lastUrl = location.href;
    const onRouteChange = () => {
      const newUrl = location.href;
      if (newUrl !== lastUrl) {
        lastUrl = newUrl;
        mainContainerCache = null; // force re-detect on next translatePage()
        console.debug("[Moon] Tier4-4: SPA route change detected, cache invalidated", rule.name);
      }
    };

    // Hook history methods
    const origPush = history.pushState;
    const origReplace = history.replaceState;
    history.pushState = function (...args) {
      const r = origPush.apply(this, args);
      setTimeout(onRouteChange, 0);
      return r;
    };
    history.replaceState = function (...args) {
      const r = origReplace.apply(this, args);
      setTimeout(onRouteChange, 0);
      return r;
    };
    window.addEventListener("popstate", onRouteChange);
    console.debug("[Moon] Tier4-4: SPA route detection enabled for", rule.name);
  })();

  loadDisplayConfig().then(() => {
    return initModules();
  }).then(() => {
    console.log("Moon Translator page translator loaded (bilingual mode:", bilingualMode, "style:", dualStyle, ")");
  });

  // 监听来自 popup / background 的消息
  chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
    if (message.type === "translatePage") {
      if (!isTranslated) togglePageTranslation();
      sendResponse({ success: true });
      return false;
    }
    if (message.type === "restorePage") {
      if (isTranslated) {
        restorePage();
        isTranslated = false;
        if (translateBtn) {
          translateBtn.classList.remove("active");
          translateBtn.title = "翻译整页";
        }
      }
      sendResponse({ success: true });
      return false;
    }
    if (message.type === "updateDisplayMode") {
      bilingualMode = message.mode || "replace";
      dualStyle = message.dualStyle || "underline";
      sendResponse({ success: true });
      return false;
    }
    return false;
  });

  window.moonTranslatePage = translatePage;
  window.moonRestorePage = restorePage;
  window.moonToggleTranslation = togglePageTranslation;

  console.log("Moon Translator page translator loaded");
})();
