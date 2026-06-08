// Content script: Full page translator for Moon Translator
// Injected on demand via context menu or button
// Performance optimized with: caching, lazy loading, batch merging, Web Worker

(function() {
  "use strict";

  let isTranslated = false;
  let originalTexts = new Map();
  let translateBtn = null;
  let isProcessing = false;

  // Cache reference (loaded async)
  let cache = null;
  let workerManager = null;

  // IntersectionObserver for lazy loading
  let intersectionObserver = null;
  let visibleNodes = new Set();
  let pendingNodes = new Set();

  // Batch translation queue
  let translationQueue = [];
  let batchTimeout = null;
  const BATCH_DELAY_MS = 100; // Wait time to collect multiple requests
  const MAX_BATCH_SIZE = 10;  // Maximum texts per batch request

  // Initialize modules
  async function initModules() {
    // Wait for cache module
    if (window.MoonTranslationCache) {
      cache = window.MoonTranslationCache;
    }

    // Wait for worker manager
    if (window.MoonWorkerManager) {
      workerManager = window.MoonWorkerManager;
      await workerManager.initPromise;
    }
  }

  // Create floating translate button
  function createTranslateButton() {
    if (document.getElementById("moon-translate-page-btn")) return;

    const btn = document.createElement("div");
    btn.id = "moon-translate-page-btn";
    btn.innerHTML = "译";
    btn.title = "翻译整页 (点击切换)";
    btn.addEventListener("click", togglePageTranslation);
    document.body.appendChild(btn);
    translateBtn = btn;
  }

  // Get all text nodes in the page
  function getTextNodes() {
    const walker = document.createTreeWalker(
      document.body,
      NodeFilter.SHOW_TEXT,
      {
        acceptNode: function(node) {
          const parent = node.parentElement;
          if (!parent) return NodeFilter.FILTER_REJECT;
          const tag = parent.tagName.toLowerCase();

          // Skip script, style, hidden elements
          if (["script", "style", "noscript", "code", "pre", "svg"].includes(tag)) {
            return NodeFilter.FILTER_REJECT;
          }

          // Skip if parent is hidden
          if (parent.offsetParent === null && parent.tagName !== "BODY") {
            return NodeFilter.FILTER_REJECT;
          }

          // Skip moon translator elements
          if (parent.id?.startsWith("moon-") || parent.closest("#moon-translator-popup")) {
            return NodeFilter.FILTER_REJECT;
          }

          // Skip empty or whitespace-only text
          if (!node.textContent.trim()) {
            return NodeFilter.FILTER_REJECT;
          }

          return NodeFilter.FILTER_ACCEPT;
        }
      }
    );

    const nodes = [];
    while (walker.nextNode()) {
      nodes.push(walker.currentNode);
    }
    return nodes;
  }

  // Setup IntersectionObserver for lazy loading
  function setupIntersectionObserver() {
    if (intersectionObserver) {
      intersectionObserver.disconnect();
    }

    intersectionObserver = new IntersectionObserver(
      (entries) => {
        entries.forEach(entry => {
          const node = entry.target;
          if (entry.isIntersecting) {
            visibleNodes.add(node);
            // If node is pending translation, translate it
            if (pendingNodes.has(node)) {
              pendingNodes.delete(node);
              translateNode(node);
            }
          } else {
            visibleNodes.delete(node);
          }
        });
      },
      {
        root: null, // viewport
        rootMargin: "200px", // Pre-load slightly outside viewport
        threshold: 0.1
      }
    );
  }

  // Observe text nodes for visibility
  function observeNodes(nodes) {
    if (!intersectionObserver) return;

    nodes.forEach(node => {
      if (node.parentElement) {
        intersectionObserver.observe(node.parentElement);
      }
    });
  }

  // Unobserve all nodes
  function unobserveAll() {
    if (intersectionObserver) {
      intersectionObserver.disconnect();
    }
    visibleNodes.clear();
    pendingNodes.clear();
  }

  // Send message to background
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

  // Build a CSS selector path from a text node's parent up to body
  function getCssSelector(node) {
    const parts = [];
    let el = node.parentElement;
    while (el && el !== document.body) {
      let selector = el.tagName.toLowerCase();
      if (el.id) {
        selector = `#${el.id}`;
        parts.unshift(selector);
        break;
      }
      if (el.className && typeof el.className === "string") {
        const cls = el.className.trim().split(/\s+/).filter(c => !c.startsWith("moon-")).slice(0, 2).join(".");
        if (cls) selector += `.${cls}`;
      }
      // Add nth-child if needed for uniqueness
      const parent = el.parentElement;
      if (parent) {
        const siblings = Array.from(parent.children).filter(c => c.tagName === el.tagName);
        if (siblings.length > 1) {
          const idx = siblings.indexOf(el) + 1;
          selector += `:nth-child(${idx})`;
        }
      }
      parts.unshift(selector);
      el = el.parentElement;
    }
    return parts.join(" > ") || "body";
  }

  // Translate a single node (with cache check)
  async function translateNode(node) {
    const text = node.textContent.trim();
    if (text.length < 2) return;

    // Check cache first
    if (cache) {
      const cached = cache.get(text, "auto", "zh");
      if (cached) {
        const translatedText = cached.primary?.text || cached.results?.[0]?.text;
        if (translatedText) {
          originalTexts.set(node, node.textContent);
          node.textContent = translatedText;
          return;
        }
      }
    }

    // Add to translation queue
    addToQueue(node, text);
  }

  // Add node to translation queue for batch processing
  function addToQueue(node, text) {
    translationQueue.push({ node, text });

    // Clear existing timeout
    if (batchTimeout) {
      clearTimeout(batchTimeout);
    }

    // Process batch when size limit reached or after delay
    if (translationQueue.length >= MAX_BATCH_SIZE) {
      processBatch();
    } else {
      batchTimeout = setTimeout(processBatch, BATCH_DELAY_MS);
    }
  }

  // Process queued translations as a batch
  async function processBatch() {
    if (translationQueue.length === 0 || isProcessing) return;

    const batch = translationQueue.splice(0, MAX_BATCH_SIZE);
    const texts = batch.map(item => item.text);
    const nodes = batch.map(item => item.node);

    try {
      // Try batch translation
      const results = await translateBatch(texts);

      // Apply results
      for (let i = 0; i < nodes.length; i++) {
        const node = nodes[i];
        const result = results[i];

        if (result && node.parentElement) {
          originalTexts.set(node, node.textContent);
          const translatedText = result.primary?.text || result.results?.[0]?.text;
          if (translatedText) {
            node.textContent = translatedText;
          }
        }
      }
    } catch (e) {
      console.warn("Batch translation failed:", e);
    }

    // Process remaining queue
    if (translationQueue.length > 0) {
      setTimeout(processBatch, 50);
    }
  }

  // Translate a batch of texts
  async function translateBatch(texts) {
    if (texts.length === 0) return [];

    // Check cache for each text
    const results = new Array(texts.length);
    const uncachedTexts = [];
    const uncachedIndices = [];

    if (cache) {
      for (let i = 0; i < texts.length; i++) {
        const cached = cache.get(texts[i], "auto", "zh");
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

    // If all cached, return immediately
    if (uncachedTexts.length === 0) return results;

    // Try desktop batch translation first
    try {
      const desktopResults = await translateBatchDesktop(uncachedTexts);
      if (desktopResults) {
        for (let i = 0; i < uncachedTexts.length; i++) {
          const idx = uncachedIndices[i];
          results[idx] = desktopResults[i];

          // Cache result
          if (cache && desktopResults[i]) {
            cache.set(uncachedTexts[i], "auto", "zh", null, desktopResults[i]);
          }
        }
        return results;
      }
    } catch (e) {
      console.warn("Desktop batch translation failed:", e);
    }

    // Fallback: translate individually with parallel requests
    const promises = uncachedTexts.map(async (text, i) => {
      try {
        const response = await sendMessage({
          type: "translate",
          text,
          from: "auto",
          to: "zh"
        });

        const idx = uncachedIndices[i];
        results[idx] = response;

        // Cache result
        if (cache && response) {
          cache.set(text, "auto", "zh", null, response);
        }
      } catch (e) {
        console.warn("Translation failed for text:", e);
      }
    });

    await Promise.all(promises);
    return results;
  }

  // Try desktop batch translation
  async function translateBatchDesktop(texts) {
    const segments = texts.map((text, index) => ({
      text: text.trim(),
      index
    })).filter(s => s.text.length >= 2);

    if (segments.length === 0) return null;

    try {
      const response = await sendMessage({
        type: "translatePageDesktop",
        segments,
        from: "auto",
        to: "zh"
      });

      if (!response.success) return null;

      const translations = response.translations;
      if (!translations || translations.length === 0) return null;

      // Map results back to original indices
      const results = new Array(texts.length);
      for (const t of translations) {
        if (t.translated) {
          results[t.index] = {
            primary: { engine: "desktop", text: t.translated },
            results: [{ engine: "desktop", text: t.translated }]
          };
        }
      }

      return results;
    } catch (e) {
      console.warn("Desktop batch translation error:", e);
      return null;
    }
  }

  // Translate text in batches (legacy method, now uses queue)
  async function translatePage() {
    if (isProcessing) return;
    isProcessing = true;

    showProgress(0, 1);

    const textNodes = getTextNodes();

    // Store original texts
    textNodes.forEach(node => {
      originalTexts.set(node, node.textContent);
    });

    // Try desktop batch translation first
    const desktopOk = await translatePageDesktop(textNodes);
    if (desktopOk) {
      isProcessing = false;
      hideProgress();
      return;
    }

    // Setup lazy loading with IntersectionObserver
    setupIntersectionObserver();
    observeNodes(textNodes);

    // Separate visible and non-visible nodes
    const visibleNow = [];
    const deferred = [];

    textNodes.forEach(node => {
      const parent = node.parentElement;
      if (parent && isElementInViewport(parent)) {
        visibleNow.push(node);
      } else {
        deferred.push(node);
        pendingNodes.add(node);
      }
    });

    // Translate visible nodes immediately
    const totalNodes = textNodes.length;
    let processed = 0;

    // Group by parent for better context
    const groups = groupByParent(visibleNow);
    const totalGroups = groups.size;

    for (const [parent, nodes] of groups) {
      const fullText = nodes.map(n => n.textContent).join("").trim();
      if (fullText.length < 2) continue;

      try {
        const results = await translateBatch([fullText]);
        if (results[0]) {
          const translatedText = results[0].primary?.text || results[0].results?.[0]?.text;
          if (translatedText && nodes[0].parentElement) {
            nodes[0].textContent = translatedText;
            for (let j = 1; j < nodes.length; j++) {
              if (nodes[j].parentElement) {
                nodes[j].textContent = "";
              }
            }
          }
        }
      } catch (e) {
        console.warn("Translation failed:", e);
      }

      processed++;
      updateProgress(processed, totalGroups + deferred.length);
    }

    // Deferred nodes will be translated when they become visible
    updateProgress(totalGroups, totalGroups + deferred.length);

    isProcessing = false;

    // Hide progress after a short delay
    setTimeout(hideProgress, 1000);
  }

  // Group text nodes by parent element
  function groupByParent(nodes) {
    const groups = new Map();
    nodes.forEach(node => {
      const parent = node.parentElement;
      if (!groups.has(parent)) {
        groups.set(parent, []);
      }
      groups.get(parent).push(node);
    });
    return groups;
  }

  // Check if element is in viewport
  function isElementInViewport(el) {
    const rect = el.getBoundingClientRect();
    return (
      rect.top < (window.innerHeight || document.documentElement.clientHeight) + 200 &&
      rect.bottom > -200 &&
      rect.left < (window.innerWidth || document.documentElement.clientWidth) + 200 &&
      rect.right > -200
    );
  }

  // Try desktop batch translation. Returns true if successful, false to fall back.
  async function translatePageDesktop(textNodes) {
    if (textNodes.length === 0) return false;

    // Build segments with CSS selectors
    const segments = textNodes.map((node, index) => ({
      selector: getCssSelector(node),
      text: node.textContent.trim(),
      index
    })).filter(s => s.text.length >= 2);

    if (segments.length === 0) return false;

    try {
      const response = await sendMessage({
        type: "translatePageDesktop",
        segments,
        from: "auto",
        to: "zh"
      });

      if (!response.success) return false;

      const translations = response.translations;
      if (!translations || translations.length === 0) return false;

      // Apply translations: match by index
      const nodeByIndex = new Map();
      textNodes.forEach((node, i) => nodeByIndex.set(i, node));

      for (const t of translations) {
        const node = nodeByIndex.get(t.index);
        if (node && t.translated) {
          node.textContent = t.translated;

          // Cache the translation
          if (cache) {
            cache.set(node.textContent, "auto", "zh", null, {
              primary: { engine: "desktop", text: t.translated },
              results: [{ engine: "desktop", text: t.translated }]
            });
          }
        }
      }

      return true;
    } catch (e) {
      console.warn("Desktop batch translation failed, falling back:", e.message);
      return false;
    }
  }

  // Restore original text
  function restorePage() {
    // Stop lazy loading
    unobserveAll();

    // Clear translation queue
    translationQueue = [];
    if (batchTimeout) {
      clearTimeout(batchTimeout);
      batchTimeout = null;
    }

    // Restore original texts
    originalTexts.forEach((text, node) => {
      if (node.parentElement) {
        node.textContent = text;
      }
    });
    originalTexts.clear();
  }

  // Toggle page translation
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

  // Progress indicator
  let progressEl = null;

  function showProgress(current, total) {
    if (!progressEl) {
      progressEl = document.createElement("div");
      progressEl.id = "moon-translate-progress";
      progressEl.style.cssText = `
        position: fixed;
        top: 20px;
        left: 50%;
        transform: translateX(-50%);
        background: rgba(0, 0, 0, 0.8);
        color: white;
        padding: 8px 16px;
        border-radius: 20px;
        font-size: 13px;
        z-index: 2147483647;
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      `;
      document.body.appendChild(progressEl);
    }

    if (progressEl) {
      progressEl.textContent = `翻译中... ${current}/${total}`;
    }
  }

  function updateProgress(current, total) {
    showProgress(current, total);
  }

  function hideProgress() {
    if (progressEl) {
      progressEl.remove();
      progressEl = null;
    }
  }

  // Observe DOM changes for SPA support
  function setupObserver() {
    const observer = new MutationObserver((mutations) => {
      if (!isTranslated || isProcessing) return;

      mutations.forEach(mutation => {
        mutation.addedNodes.forEach(node => {
          if (node.nodeType === Node.TEXT_NODE && node.textContent.trim()) {
            const text = node.textContent;
            originalTexts.set(node, text);

            // Add to queue instead of immediate translation
            addToQueue(node, text);
          }
        });
      });
    });

    observer.observe(document.body, {
      childList: true,
      subtree: true
    });
  }

  // ==================== Initialize ====================

  // Create button when script loads
  createTranslateButton();
  setupObserver();

  // Initialize modules asynchronously
  initModules().then(() => {
    console.log("Moon Translator page translator loaded (with cache and worker support)");
  });

  // Expose functions for content script communication
  window.moonTranslatePage = translatePage;
  window.moonRestorePage = restorePage;
  window.moonToggleTranslation = togglePageTranslation;

  console.log("Moon Translator page translator loaded");
})();
