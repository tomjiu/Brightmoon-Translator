// Content script: Hover translator for Moon Translator
// Shows translation tooltip when hovering over text elements

(function() {
  "use strict";

  // ==================== State ====================

  let tooltip = null;
  let hoverTimeout = null;
  let currentTarget = null;
  let isTranslating = false;
  let enabled = true;

  // Configurable values (loaded from storage)
  let hoverDelay = 300;
  let minTextLength = 2;
  const MAX_TEXT_LENGTH = 2000;
  let modifierKey = "none"; // "none", "alt", "ctrl", "shift"

  // Skip these elements for hover translation
  const SKIP_TAGS = new Set([
    "INPUT", "TEXTAREA", "BUTTON", "SELECT", "A",
    "SCRIPT", "STYLE", "CODE", "PRE", "SVG", "CANVAS",
    "VIDEO", "AUDIO", "IFRAME", "EMBED", "OBJECT"
  ]);

  let config = {
    targetLang: "zh",
    sourceLang: "auto"
  };

  // ==================== I18n ====================

  const BROWSER_LANG = navigator.language.startsWith("zh") ? "zh" : "en";

  const UI_STRINGS = {
    zh: { loading: "翻译中...", noResult: "无翻译结果", failed: "翻译失败", requestFailed: "请求失败" },
    en: { loading: "Translating...", noResult: "No result", failed: "Translation failed", requestFailed: "Request failed" }
  };

  function t(key) {
    return (UI_STRINGS[BROWSER_LANG] || UI_STRINGS.en)[key] || key;
  }

  // ==================== Config ====================

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

  async function loadConfig() {
    try {
      const response = await sendMessage({ type: "getConfig" });
      if (response?.config) {
        config.targetLang = response.config.targetLang || config.targetLang;
        config.sourceLang = response.config.sourceLang || config.sourceLang;

        const h = response.config.hover || {};
        enabled = h.enabled !== false; // default true
        hoverDelay = Math.max(100, Math.min(2000, h.delay || 300));
        minTextLength = Math.max(1, Math.min(100, h.minTextLength || 2));
        modifierKey = h.modifierKey || "none";
      }
    } catch (e) {
      // Silent fail, use defaults
    }
  }

  // Listen for config changes while content script is alive
  chrome.storage.onChanged.addListener((changes, area) => {
    if (area === "local" && changes.config) {
      const newConfig = changes.config.newValue || {};
      config.targetLang = newConfig.targetLang || config.targetLang;
      config.sourceLang = newConfig.sourceLang || config.sourceLang;

      const h = newConfig.hover || {};
      const wasEnabled = enabled;
      enabled = h.enabled !== false;
      hoverDelay = Math.max(100, Math.min(2000, h.delay || 300));
      minTextLength = Math.max(1, Math.min(100, h.minTextLength || 2));
      modifierKey = h.modifierKey || "none";

      // If just disabled, clean up
      if (wasEnabled && !enabled) {
        hideTooltip();
        if (hoverTimeout) { clearTimeout(hoverTimeout); hoverTimeout = null; }
      }
    }
  });

  // ==================== Modifier Key ====================

  let modifierPressed = false;

  function isModifierMatch(e) {
    switch (modifierKey) {
      case "alt": return e.altKey;
      case "ctrl": return e.ctrlKey || e.metaKey;
      case "shift": return e.shiftKey;
      default: return true; // "none" — always triggers
    }
  }

  if (modifierKey !== "none") {
    document.addEventListener("keydown", (e) => {
      if (modifierKey === "alt" && e.key === "Alt") modifierPressed = true;
      if (modifierKey === "ctrl" && (e.key === "Control" || e.key === "Meta")) modifierPressed = true;
      if (modifierKey === "shift" && e.key === "Shift") modifierPressed = true;
    });
    document.addEventListener("keyup", (e) => {
      if (modifierKey === "alt" && e.key === "Alt") { modifierPressed = false; hideTooltip(); }
      if (modifierKey === "ctrl" && (e.key === "Control" || e.key === "Meta")) { modifierPressed = false; hideTooltip(); }
      if (modifierKey === "shift" && e.key === "Shift") { modifierPressed = false; hideTooltip(); }
    });
  }

  // ==================== Tooltip ====================

  function createTooltip() {
    const el = document.createElement("div");
    el.id = "moon-hover-tooltip";
    el.innerHTML = `
      <div class="mht-content">
        <div class="mht-loading">
          <div class="mht-spinner"></div>
          <span>${t("loading")}</span>
        </div>
        <div class="mht-result" style="display:none"></div>
        <div class="mht-error" style="display:none"></div>
      </div>
    `;
    document.body.appendChild(el);

    // Prevent tooltip from interfering with hover detection
    el.addEventListener("mouseenter", () => {
      if (hoverTimeout) {
        clearTimeout(hoverTimeout);
        hoverTimeout = null;
      }
    });

    el.addEventListener("mouseleave", () => {
      scheduleHide();
    });

    return el;
  }

  function showTooltip(x, y) {
    if (!tooltip) {
      tooltip = createTooltip();
    }

    // Reset state
    tooltip.querySelector(".mht-loading").style.display = "flex";
    tooltip.querySelector(".mht-result").style.display = "none";
    tooltip.querySelector(".mht-error").style.display = "none";
    tooltip.querySelector(".mht-loading span").textContent = t("loading");

    // Position tooltip
    const scrollX = window.scrollX;
    const scrollY = window.scrollY;
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;

    // Show temporarily to measure
    tooltip.style.display = "block";
    tooltip.style.left = "0px";
    tooltip.style.top = "0px";
    const tooltipRect = tooltip.getBoundingClientRect();

    let left = scrollX + x + 10;
    let top = scrollY + y + 10;

    // Keep within viewport
    if (left + tooltipRect.width > scrollX + viewportWidth) {
      left = scrollX + x - tooltipRect.width - 10;
    }
    if (top + tooltipRect.height > scrollY + viewportHeight) {
      top = scrollY + y - tooltipRect.height - 10;
    }

    tooltip.style.left = `${Math.max(scrollX + 5, left)}px`;
    tooltip.style.top = `${Math.max(scrollY + 5, top)}px`;
  }

  function hideTooltip() {
    if (tooltip) {
      tooltip.style.display = "none";
    }
    isTranslating = false;
    currentTarget = null;
  }

  let hideTimeout = null;
  function scheduleHide() {
    if (hideTimeout) clearTimeout(hideTimeout);
    hideTimeout = setTimeout(hideTooltip, 200);
  }

  function cancelHide() {
    if (hideTimeout) {
      clearTimeout(hideTimeout);
      hideTimeout = null;
    }
  }

  // ==================== Text Extraction ====================

  function getVisibleText(element) {
    if (SKIP_TAGS.has(element.tagName)) return "";

    const walker = document.createTreeWalker(
      element,
      NodeFilter.SHOW_TEXT,
      {
        acceptNode(node) {
          const parent = node.parentElement;
          if (!parent) return NodeFilter.FILTER_REJECT;
          const tag = parent.tagName;
          if (tag === "SCRIPT" || tag === "STYLE" || tag === "NOSCRIPT") {
            return NodeFilter.FILTER_REJECT;
          }
          const style = window.getComputedStyle(parent);
          if (style.display === "none" || style.visibility === "hidden") {
            return NodeFilter.FILTER_REJECT;
          }
          return NodeFilter.FILTER_ACCEPT;
        }
      }
    );

    let text = "";
    let node;
    while ((node = walker.nextNode())) {
      const t = node.textContent.trim();
      if (t) {
        text += (text ? " " : "") + t;
      }
    }

    return text.trim();
  }

  function isInteractiveElement(el) {
    if (SKIP_TAGS.has(el.tagName)) return true;
    if (el.isContentEditable) return true;
    if (el.getAttribute("role") === "textbox" || el.getAttribute("role") === "button") return true;
    if (el.getAttribute("tabindex") !== null && el.tagName !== "BODY") return true;
    return false;
  }

  function getHoverTextTarget(element) {
    const BLOCK_TAGS = new Set([
      "P", "LI", "TD", "TH", "DT", "DD", "BLOCKQUOTE",
      "DIV", "ARTICLE", "SECTION", "H1", "H2", "H3", "H4", "H5", "H6"
    ]);

    let current = element;
    let bestText = "";

    for (let depth = 0; depth < 5 && current && current !== document.body; depth++) {
      if (isInteractiveElement(current)) return { text: "", element: null };

      const text = getVisibleText(current);
      if (text.length >= minTextLength && text.length <= MAX_TEXT_LENGTH) {
        bestText = text;
        if (BLOCK_TAGS.has(current.tagName)) {
          return { text, element: current };
        }
      }

      current = current.parentElement;
    }

    return { text: bestText, element: current };
  }

  // ==================== Translation ====================

  async function translateHover(text, x, y) {
    isTranslating = true;
    showTooltip(x, y);

    try {
      const response = await sendMessage({
        type: "translate",
        text: text,
        from: config.sourceLang,
        to: config.targetLang
      });

      if (!isTranslating) return;

      tooltip.querySelector(".mht-loading").style.display = "none";

      if (response.success) {
        const resultDiv = tooltip.querySelector(".mht-result");
        const primary = response.primary || (response.results && response.results[0]);
        if (primary) {
          resultDiv.textContent = primary.text;
          resultDiv.style.display = "block";
        } else {
          tooltip.querySelector(".mht-error").textContent = t("noResult");
          tooltip.querySelector(".mht-error").style.display = "block";
        }
      } else {
        tooltip.querySelector(".mht-error").textContent = response.error || t("failed");
        tooltip.querySelector(".mht-error").style.display = "block";
      }
    } catch (err) {
      if (!isTranslating) return;
      tooltip.querySelector(".mht-loading").style.display = "none";
      tooltip.querySelector(".mht-error").textContent = t("requestFailed");
      tooltip.querySelector(".mht-error").style.display = "block";
    }
  }

  // ==================== Event Listeners ====================

  document.addEventListener("mouseover", (e) => {
    if (!enabled) return;

    // Check modifier key requirement
    if (modifierKey !== "none" && !isModifierMatch(e)) return;

    // Ignore if hovering over tooltip itself
    if (tooltip && tooltip.contains(e.target)) return;

    // Ignore if hovering over the selection popup
    if (e.target.closest("#moon-translator-popup")) return;

    cancelHide();

    const target = e.target;
    if (target === currentTarget) return;

    currentTarget = target;

    // Clear previous timeout
    if (hoverTimeout) {
      clearTimeout(hoverTimeout);
      hoverTimeout = null;
    }

    // Debounce
    hoverTimeout = setTimeout(() => {
      const { text, element } = getHoverTextTarget(target);
      if (!text || text.length < minTextLength) return;

      const rect = (element || target).getBoundingClientRect();
      const x = rect.left;
      const y = rect.bottom;

      translateHover(text, x, y);
    }, hoverDelay);
  });

  document.addEventListener("mouseout", (e) => {
    if (e.target === currentTarget) {
      const related = e.relatedTarget;
      if (tooltip && related && tooltip.contains(related)) return;

      currentTarget = null;
      if (hoverTimeout) {
        clearTimeout(hoverTimeout);
        hoverTimeout = null;
      }
      scheduleHide();
    }
  });

  // Hide on scroll
  document.addEventListener("scroll", hideTooltip, { passive: true });

  // Hide on Escape
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      hideTooltip();
    }
  });

  // ==================== Initialize ====================

  loadConfig();
})();
