// Content script: Selection translator for Moon Translator
// Works on Chrome MV3 and Firefox MV3

(function() {
  "use strict";

  let popup = null;
  let isTranslating = false;
  let translateTimeout = null;
  let config = {
    targetLang: "zh",
    sourceLang: "auto",
    autoTranslate: false,
    showButton: true
  };

  // Load config
  async function loadConfig() {
    try {
      const response = await sendMessage({ type: "getConfig" });
      if (response?.config) {
        config = { ...config, ...response.config };
      }
    } catch (e) {
      console.warn("Failed to load config:", e);
    }
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

  // Create popup element
  function createPopup() {
    const el = document.createElement("div");
    el.id = "moon-translator-popup";
    el.innerHTML = `
      <div class="mt-header">
        <span class="mt-title">🌙 Moon Translator</span>
        <button class="mt-close" title="关闭">&times;</button>
      </div>
      <div class="mt-body">
        <div class="mt-loading">
          <div class="mt-spinner"></div>
          <span>翻译中...</span>
        </div>
        <div class="mt-results" style="display:none"></div>
        <div class="mt-error" style="display:none"></div>
      </div>
      <div class="mt-footer">
        <button class="mt-btn mt-btn-copy mt-btn-secondary" title="复制翻译结果">复制</button>
        <button class="mt-btn mt-btn-close mt-btn-secondary">关闭</button>
      </div>
    `;
    document.body.appendChild(el);

    // Event listeners
    el.querySelector(".mt-close").addEventListener("click", hidePopup);
    el.querySelector(".mt-btn-close").addEventListener("click", hidePopup);
    el.querySelector(".mt-btn-copy").addEventListener("click", copyResult);

    // Prevent popup from closing when clicking inside
    el.addEventListener("mousedown", (e) => e.stopPropagation());

    return el;
  }

  // Show popup near selection
  function showPopup(text, x, y) {
    if (!popup) {
      popup = createPopup();
    }

    // Position popup
    const scrollX = window.scrollX;
    const scrollY = window.scrollY;
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;

    let left = scrollX + x;
    let top = scrollY + y + 10;

    // Adjust if popup would go off screen
    popup.style.display = "block";
    const popupRect = popup.getBoundingClientRect();

    if (left + popupRect.width > scrollX + viewportWidth) {
      left = scrollX + viewportWidth - popupRect.width - 10;
    }
    if (top + popupRect.height > scrollY + viewportHeight) {
      top = scrollY + y - popupRect.height - 10;
    }

    popup.style.left = `${Math.max(scrollX + 10, left)}px`;
    popup.style.top = `${Math.max(scrollY + 10, top)}px`;

    // Show loading
    popup.querySelector(".mt-loading").style.display = "flex";
    popup.querySelector(".mt-results").style.display = "none";
    popup.querySelector(".mt-error").style.display = "none";

    // Translate
    isTranslating = true;
    translateText(text);
  }

  // Hide popup
  function hidePopup() {
    if (popup) {
      popup.style.display = "none";
      isTranslating = false;
    }
  }

  // Translate text
  async function translateText(text) {
    try {
      const response = await sendMessage({
        type: "translate",
        text: text,
        from: config.sourceLang,
        to: config.targetLang
      });

      if (!isTranslating) return;

      popup.querySelector(".mt-loading").style.display = "none";

      if (response.success) {
        const resultsDiv = popup.querySelector(".mt-results");
        resultsDiv.innerHTML = "";

        // Show results from each engine
        if (response.results && response.results.length > 0) {
          response.results.forEach(result => {
            const item = document.createElement("div");
            item.className = "mt-result-item";
            item.innerHTML = `
              <div class="mt-engine">${escapeHtml(result.engine)}</div>
              <div class="mt-result-text">${escapeHtml(result.text)}</div>
            `;
            resultsDiv.appendChild(item);
          });
        } else if (response.primary) {
          const item = document.createElement("div");
          item.className = "mt-result-item";
          item.innerHTML = `
            <div class="mt-engine">${escapeHtml(response.primary.engine)}</div>
            <div class="mt-result-text">${escapeHtml(response.primary.text)}</div>
          `;
          resultsDiv.appendChild(item);
        }

        resultsDiv.style.display = "block";
      } else {
        const errorDiv = popup.querySelector(".mt-error");
        errorDiv.textContent = response.error || "翻译失败";
        errorDiv.style.display = "block";
      }
    } catch (err) {
      if (!isTranslating) return;
      popup.querySelector(".mt-loading").style.display = "none";
      const errorDiv = popup.querySelector(".mt-error");
      errorDiv.textContent = "翻译请求失败: " + err.message;
      errorDiv.style.display = "block";
    }
  }

  // Copy result to clipboard
  function copyResult() {
    const results = popup.querySelectorAll(".mt-result-text");
    if (results.length > 0) {
      const text = Array.from(results).map(r => r.textContent).join("\n");
      navigator.clipboard.writeText(text).then(() => {
        const btn = popup.querySelector(".mt-btn-copy");
        btn.textContent = "已复制 ✓";
        setTimeout(() => { btn.textContent = "复制"; }, 1500);
      });
    }
  }

  // Escape HTML
  function escapeHtml(text) {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
  }

  // Get selection position
  function getSelectionPosition() {
    const selection = window.getSelection();
    if (!selection.rangeCount) return null;

    const range = selection.getRangeAt(0);
    const rect = range.getBoundingClientRect();

    return {
      text: selection.toString().trim(),
      x: rect.left,
      y: rect.bottom
    };
  }

  // ==================== Event Listeners ====================

  // Mouse up - detect selection
  document.addEventListener("mouseup", (e) => {
    // Ignore if clicking inside popup
    if (popup && popup.contains(e.target)) return;

    // Small delay to allow selection to finalize
    setTimeout(() => {
      const pos = getSelectionPosition();
      if (pos && pos.text.length > 0 && pos.text.length < 5000) {
        showPopup(pos.text, pos.x, pos.y);
      }
    }, 10);
  });

  // Click outside - hide popup
  document.addEventListener("mousedown", (e) => {
    if (popup && !popup.contains(e.target)) {
      hidePopup();
    }
  });

  // Escape key - hide popup
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      hidePopup();
    }
  });

  // Listen for messages from background
  chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    if (message.type === "translate-selection") {
      const pos = getSelectionPosition();
      showPopup(message.text, pos?.x || 100, pos?.y || 100);
      sendResponse({ success: true });
    }

    if (message.type === "getSelection") {
      const pos = getSelectionPosition();
      if (pos && pos.text) {
        showPopup(pos.text, pos.x, pos.y);
      }
      sendResponse({ success: true });
    }

    if (message.type === "translatePage") {
      if (typeof window.moonTranslatePage === "function") {
        window.moonTranslatePage();
      }
      sendResponse({ success: true });
    }

    if (message.type === "restorePage") {
      if (typeof window.moonRestorePage === "function") {
        window.moonRestorePage();
      }
      sendResponse({ success: true });
    }
  });

  // Initialize
  loadConfig();

  console.log("Moon Translator content script loaded");
})();
