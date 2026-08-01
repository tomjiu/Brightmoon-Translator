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

  // Tier4-2: Closed Shadow DOM isolation for the selection popup.
  //
  // All popup styles live inside a closed shadow root so host-page CSS
  // cannot leak in (e.g. a site's `div { background: red }` would have
  // colored our popup white background). The host element keeps its id
  // `moon-translator-popup` for external positioning and e2e tests.
  //
  // Internal elements are queried via the captured `popupShadow` reference
  // because `el.shadowRoot` returns null for closed roots.
  const POPUP_STYLE = `
    :host {
      position: absolute;
      z-index: 2147483647;
      background: #ffffff;
      border-radius: 12px;
      box-shadow: 0 4px 24px rgba(0, 0, 0, 0.15), 0 0 0 1px rgba(0, 0, 0, 0.05);
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
      font-size: 14px;
      line-height: 1.5;
      color: #1a1a1a;
      max-width: 420px;
      min-width: 280px;
      overflow: hidden;
      animation: mt-slide-in 0.15s ease-out;
      user-select: none;
      display: block;
    }
    @keyframes mt-slide-in {
      from { opacity: 0; transform: translateY(-8px); }
      to { opacity: 1; transform: translateY(0); }
    }
    .mt-header {
      display: flex; align-items: center; justify-content: space-between;
      padding: 10px 14px;
      background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
      color: white;
    }
    .mt-title { font-size: 12px; font-weight: 600; letter-spacing: 0.5px; }
    .mt-close {
      background: none; border: none; color: white;
      font-size: 18px; cursor: pointer; padding: 0 4px;
      opacity: 0.8; line-height: 1; border-radius: 4px;
    }
    .mt-close:hover { opacity: 1; background: rgba(255, 255, 255, 0.2); }
    .mt-body { padding: 12px 14px; max-height: 300px; overflow-y: auto; }
    .mt-engine {
      font-size: 11px; color: #667eea; font-weight: 600;
      margin-bottom: 4px; text-transform: uppercase; letter-spacing: 0.5px;
    }
    .mt-result-item {
      margin-bottom: 10px; padding-bottom: 10px;
      border-bottom: 1px solid #f0f0f0;
    }
    .mt-result-item:last-child { margin-bottom: 0; padding-bottom: 0; border-bottom: none; }
    .mt-result-text { color: #1a1a1a; font-size: 14px; line-height: 1.6; word-break: break-word; }
    .mt-loading { display: flex; align-items: center; gap: 8px; color: #666; padding: 8px 0; }
    .mt-spinner {
      width: 16px; height: 16px;
      border: 2px solid #e0e0e0; border-top-color: #667eea;
      border-radius: 50%; animation: mt-spin 0.6s linear infinite;
    }
    @keyframes mt-spin { to { transform: rotate(360deg); } }
    .mt-error { color: #e53e3e; font-size: 13px; padding: 4px 0; }
    .mt-footer {
      display: flex; gap: 8px; padding: 8px 14px;
      border-top: 1px solid #f0f0f0; background: #fafafa;
    }
    .mt-btn {
      flex: 1; padding: 6px 12px; border: none; border-radius: 6px;
      font-size: 12px; font-weight: 500; cursor: pointer; transition: all 0.15s;
    }
    .mt-btn-primary { background: #667eea; color: white; }
    .mt-btn-primary:hover { background: #5a6fd6; }
    .mt-btn-secondary { background: #e8e8e8; color: #333; }
    .mt-btn-secondary:hover { background: #d8d8d8; }
    @media (prefers-color-scheme: dark) {
      :host { background: #1e1e2e; color: #e0e0e0; }
      .mt-header { background: linear-gradient(135deg, #4a5568 0%, #553c9a 100%); }
      .mt-result-text { color: #e0e0e0; }
      .mt-result-item { border-bottom-color: #333; }
      .mt-footer { background: #252535; border-top-color: #333; }
      .mt-btn-secondary { background: #333; color: #e0e0e0; }
      .mt-btn-secondary:hover { background: #444; }
      .mt-error { color: #fc8181; }
    }
  `;

  let popupShadow = null; // captured reference into closed shadow root

  // Create popup element
  function createPopup() {
    const host = document.createElement("div");
    host.id = "moon-translator-popup";
    popupShadow = host.attachShadow({ mode: "closed" });
    popupShadow.innerHTML = `
      <style>${POPUP_STYLE}</style>
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
    document.body.appendChild(host);

    // Event listeners — query inside the captured shadow root
    const q = (sel) => popupShadow.querySelector(sel);
    q(".mt-close").addEventListener("click", hidePopup);
    q(".mt-btn-close").addEventListener("click", hidePopup);
    q(".mt-btn-copy").addEventListener("click", copyResult);

    // Prevent popup from closing when clicking inside
    host.addEventListener("mousedown", (e) => e.stopPropagation());

    return host;
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
    popupShadow.querySelector(".mt-loading").style.display = "flex";
    popupShadow.querySelector(".mt-results").style.display = "none";
    popupShadow.querySelector(".mt-error").style.display = "none";

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

      popupShadow.querySelector(".mt-loading").style.display = "none";

      if (response.success) {
        const resultsDiv = popupShadow.querySelector(".mt-results");
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
        const errorDiv = popupShadow.querySelector(".mt-error");
        errorDiv.textContent = response.error || "翻译失败";
        errorDiv.style.display = "block";
      }
    } catch (err) {
      if (!isTranslating) return;
      popupShadow.querySelector(".mt-loading").style.display = "none";
      const errorDiv = popupShadow.querySelector(".mt-error");
      errorDiv.textContent = "翻译请求失败: " + err.message;
      errorDiv.style.display = "block";
    }
  }

  // Copy result to clipboard
  function copyResult() {
    const results = popupShadow.querySelectorAll(".mt-result-text");
    if (results.length > 0) {
      const text = Array.from(results).map(r => r.textContent).join("\n");
      navigator.clipboard.writeText(text).then(() => {
        const btn = popupShadow.querySelector(".mt-btn-copy");
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
