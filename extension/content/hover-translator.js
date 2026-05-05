// Content script: Hover translator for Moon Translator
// Shows translation tooltip when hovering over text elements

(function() {
  "use strict";

  let tooltip = null;
  let hoverTimeout = null;
  let currentTarget = null;
  let isTranslating = false;

  const HOVER_DELAY = 300; // ms debounce
  const MIN_TEXT_LENGTH = 2;
  const MAX_TEXT_LENGTH = 2000;

  // Skip these elements for hover translation
  const SKIP_TAGS = new Set(["INPUT", "TEXTAREA", "BUTTON", "SELECT", "A", "SCRIPT", "STYLE", "CODE", "PRE"]);

  let config = {
    targetLang: "zh",
    sourceLang: "auto"
  };

  // Load config from background
  async function loadConfig() {
    try {
      const response = await sendMessage({ type: "getConfig" });
      if (response?.config) {
        config.targetLang = response.config.targetLang || config.targetLang;
        config.sourceLang = response.config.sourceLang || config.sourceLang;
      }
    } catch (e) {
      // Silent fail, use defaults
    }
  }

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

  // Create tooltip element
  function createTooltip() {
    const el = document.createElement("div");
    el.id = "moon-hover-tooltip";
    el.innerHTML = `
      <div class="mht-content">
        <div class="mht-loading">
          <div class="mht-spinner"></div>
          <span>翻译中...</span>
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

  // Get visible text content of an element, skipping hidden children
  function getVisibleText(element) {
    // Skip non-text elements
    if (SKIP_TAGS.has(element.tagName)) return "";

    // Skip elements with no direct text
    const walker = document.createTreeWalker(
      element,
      NodeFilter.SHOW_TEXT,
      {
        acceptNode(node) {
          // Skip script/style content
          const parent = node.parentElement;
          if (!parent) return NodeFilter.FILTER_REJECT;
          const tag = parent.tagName;
          if (tag === "SCRIPT" || tag === "STYLE" || tag === "NOSCRIPT") {
            return NodeFilter.FILTER_REJECT;
          }
          // Skip hidden elements
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

  // Find the best text-bearing ancestor for hover
  function getHoverTextTarget(element) {
    // Walk up to find a meaningful text block (paragraph, list item, div with text)
    const BLOCK_TAGS = new Set(["P", "LI", "TD", "TH", "DT", "DD", "BLOCKQUOTE", "DIV", "ARTICLE", "SECTION", "H1", "H2", "H3", "H4", "H5", "H6"]);

    let current = element;
    let bestText = "";

    for (let depth = 0; depth < 5 && current && current !== document.body; depth++) {
      if (SKIP_TAGS.has(current.tagName)) return { text: "", element: null };

      const text = getVisibleText(current);
      if (text.length >= MIN_TEXT_LENGTH && text.length <= MAX_TEXT_LENGTH) {
        bestText = text;
        // If we hit a block element, use it
        if (BLOCK_TAGS.has(current.tagName)) {
          return { text, element: current };
        }
      }

      current = current.parentElement;
    }

    return { text: bestText, element: current };
  }

  // Show tooltip near element
  function showTooltip(element, x, y) {
    if (!tooltip) {
      tooltip = createTooltip();
    }

    // Reset state
    tooltip.querySelector(".mht-loading").style.display = "flex";
    tooltip.querySelector(".mht-result").style.display = "none";
    tooltip.querySelector(".mht-error").style.display = "none";

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

  // Translate and show result in tooltip
  async function translateHover(text, x, y) {
    isTranslating = true;
    showTooltip(null, x, y);

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
          tooltip.querySelector(".mht-error").textContent = "无翻译结果";
          tooltip.querySelector(".mht-error").style.display = "block";
        }
      } else {
        tooltip.querySelector(".mht-error").textContent = response.error || "翻译失败";
        tooltip.querySelector(".mht-error").style.display = "block";
      }
    } catch (err) {
      if (!isTranslating) return;
      tooltip.querySelector(".mht-loading").style.display = "none";
      tooltip.querySelector(".mht-error").textContent = "请求失败";
      tooltip.querySelector(".mht-error").style.display = "block";
    }
  }

  // ==================== Event Listeners ====================

  document.addEventListener("mouseover", (e) => {
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
      if (!text || text.length < MIN_TEXT_LENGTH) return;

      const rect = (element || target).getBoundingClientRect();
      const x = rect.left;
      const y = rect.bottom;

      translateHover(text, x, y);
    }, HOVER_DELAY);
  });

  document.addEventListener("mouseout", (e) => {
    // Only hide if leaving the target element entirely
    if (e.target === currentTarget) {
      const related = e.relatedTarget;
      // Don't hide if moving to tooltip
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

  // Initialize
  loadConfig();
})();
