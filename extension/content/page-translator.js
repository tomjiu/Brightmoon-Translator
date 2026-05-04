// Content script: Full page translator for Moon Translator
// Injected on demand via context menu or button

(function() {
  "use strict";

  let isTranslated = false;
  let originalTexts = new Map();
  let translateBtn = null;
  let isProcessing = false;

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

  // Translate text in batches
  async function translatePage() {
    if (isProcessing) return;
    isProcessing = true;

    const textNodes = getTextNodes();
    const batchSize = 3;

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

    // Fallback: group text by parent to maintain context, translate per-group
    const groups = new Map();
    textNodes.forEach(node => {
      const parent = node.parentElement;
      if (!groups.has(parent)) {
        groups.set(parent, []);
      }
      groups.get(parent).push(node);
    });

    const totalParents = groups.size;
    let processed = 0;

    const parents = Array.from(groups.keys());
    for (let i = 0; i < parents.length; i += batchSize) {
      const batch = parents.slice(i, i + batchSize);
      const promises = batch.map(async (parent) => {
        const nodes = groups.get(parent);
        const fullText = nodes.map(n => n.textContent).join("").trim();

        if (fullText.length < 2) return;

        try {
          const response = await sendMessage({
            type: "translate",
            text: fullText,
            from: "auto",
            to: "zh"
          });

          if (response.success) {
            const translatedText = response.primary?.text || response.results?.[0]?.text;
            if (translatedText) {
              if (nodes.length > 0) {
                nodes[0].textContent = translatedText;
                for (let j = 1; j < nodes.length; j++) {
                  nodes[j].textContent = "";
                }
              }
            }
          }
        } catch (e) {
          console.warn("Translation failed for node:", e);
        }

        processed++;
        updateProgress(processed, totalParents);
      });

      await Promise.all(promises);
    }

    isProcessing = false;
    hideProgress();
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

  function showProgress() {
    if (progressEl) return;

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

  function updateProgress(current, total) {
    showProgress();
    if (progressEl) {
      progressEl.textContent = `翻译中... ${current}/${total}`;
    }
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

            sendMessage({
              type: "translate",
              text: text,
              from: "auto",
              to: "zh"
            }).then(response => {
              if (response.success) {
                const translatedText = response.primary?.text || response.results?.[0]?.text;
                if (translatedText) {
                  node.textContent = translatedText;
                }
              }
            }).catch(() => {});
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

  // Expose functions for content script communication
  window.moonTranslatePage = translatePage;
  window.moonRestorePage = restorePage;
  window.moonToggleTranslation = togglePageTranslation;

  console.log("Moon Translator page translator loaded");
})();
