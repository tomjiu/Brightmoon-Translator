// Content script: Full page translator for Moon Translator

(function() {
  let isTranslated = false;
  let originalTexts = new Map();
  let translateBtn = null;

  // Create floating translate button
  function createTranslateButton() {
    const btn = document.createElement("div");
    btn.id = "moon-translate-page-btn";
    btn.innerHTML = "译";
    btn.title = "翻译整页";
    btn.addEventListener("click", togglePageTranslation);
    document.body.appendChild(btn);
    return btn;
  }

  // Get all text nodes in the page
  function getTextNodes() {
    const walker = document.createTreeWalker(
      document.body,
      NodeFilter.SHOW_TEXT,
      {
        acceptNode: function(node) {
          // Skip script, style, and hidden elements
          const parent = node.parentElement;
          if (!parent) return NodeFilter.FILTER_REJECT;
          const tag = parent.tagName.toLowerCase();
          if (["script", "style", "noscript", "code", "pre"].includes(tag)) {
            return NodeFilter.FILTER_REJECT;
          }
          // Skip if parent is hidden
          if (parent.offsetParent === null && parent.tagName !== "BODY") {
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

  // Translate text in batches
  async function translatePage() {
    const textNodes = getTextNodes();
    const batchSize = 5;

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

    // Translate in batches
    const parents = Array.from(groups.keys());
    for (let i = 0; i < parents.length; i += batchSize) {
      const batch = parents.slice(i, i + batchSize);
      const promises = batch.map(async (parent) => {
        const nodes = groups.get(parent);
        const fullText = nodes.map(n => n.textContent).join("");

        try {
          const response = await browser.runtime.sendMessage({
            type: "translate",
            text: fullText,
            from: "auto",
            to: "zh"
          });

          if (response.success) {
            // Apply translation to first node, clear others
            if (nodes.length > 0) {
              nodes[0].textContent = response.result;
              for (let j = 1; j < nodes.length; j++) {
                nodes[j].textContent = "";
              }
            }
          }
        } catch (e) {
          console.warn("Translation failed for node:", e);
        }
      });

      await Promise.all(promises);
    }
  }

  // Restore original text
  function restorePage() {
    originalTexts.forEach((text, node) => {
      node.textContent = text;
    });
    originalTexts.clear();
  }

  // Toggle page translation
  async function togglePageTranslation() {
    if (isTranslated) {
      restorePage();
      isTranslated = false;
      translateBtn.classList.remove("active");
      translateBtn.title = "翻译整页";
    } else {
      translateBtn.classList.add("active");
      translateBtn.title = "恢复原文";
      await translatePage();
      isTranslated = true;
    }
  }

  // Initialize
  translateBtn = createTranslateButton();

  // Observe DOM changes for SPA support
  const observer = new MutationObserver((mutations) => {
    if (!isTranslated) return;

    mutations.forEach(mutation => {
      mutation.addedNodes.forEach(node => {
        if (node.nodeType === Node.TEXT_NODE && node.textContent.trim()) {
          // Translate new text nodes
          const text = node.textContent;
          originalTexts.set(node, text);
          browser.runtime.sendMessage({
            type: "translate",
            text: text,
            from: "auto",
            to: "zh"
          }).then(response => {
            if (response.success) {
              node.textContent = response.result;
            }
          });
        }
      });
    });
  });

  observer.observe(document.body, {
    childList: true,
    subtree: true
  });
})();
