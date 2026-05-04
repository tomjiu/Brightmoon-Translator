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

    // Group text by parent to maintain context
    const groups = new Map();
    textNodes.forEach(node => {
      const parent = node.parentElement;
      if (!groups.has(parent)) {
        groups.set(parent, []);
      }
      groups.get(parent).push(node);
    });

    // Show progress
    const totalParents = groups.size;
    let processed = 0;

    // Translate in batches
    const parents = Array.from(groups.keys());
    for (let i = 0; i < parents.length; i += batchSize) {
      const batch = parents.slice(i, i + batchSize);
      const promises = batch.map(async (parent) => {
        const nodes = groups.get(parent);
        const fullText = nodes.map(n => n.textContent).join("").trim();

        if (fullText.length < 2) return; // Skip very short text

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
              // Apply translation to first node, clear others
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
