// Content script: Selection translator for Moon Translator

(function() {
  let popup = null;
  let isTranslating = false;

  // Create popup element
  function createPopup() {
    const el = document.createElement("div");
    el.id = "moon-translator-popup";
    el.innerHTML = `
      <div class="mt-header">
        <span class="mt-title">Moon Translator</span>
        <button class="mt-close" title="关闭">&times;</button>
      </div>
      <div class="mt-body">
        <div class="mt-loading">翻译中...</div>
        <div class="mt-result" style="display:none"></div>
        <div class="mt-error" style="display:none"></div>
      </div>
      <div class="mt-footer">
        <button class="mt-copy" title="复制翻译结果">复制</button>
      </div>
    `;
    document.body.appendChild(el);
    return el;
  }

  // Show popup near selection
  function showPopup(text, x, y) {
    if (!popup) {
      popup = createPopup();

      // Close button
      popup.querySelector(".mt-close").addEventListener("click", hidePopup);

      // Copy button
      popup.querySelector(".mt-copy").addEventListener("click", () => {
        const result = popup.querySelector(".mt-result").textContent;
        navigator.clipboard.writeText(result).then(() => {
          const btn = popup.querySelector(".mt-copy");
          btn.textContent = "已复制";
          setTimeout(() => { btn.textContent = "复制"; }, 1500);
        });
      });
    }

    // Position popup
    const scrollX = window.scrollX;
    const scrollY = window.scrollY;
    popup.style.left = `${scrollX + x}px`;
    popup.style.top = `${scrollY + y + 10}px`;
    popup.style.display = "block";

    // Show loading
    popup.querySelector(".mt-loading").style.display = "block";
    popup.querySelector(".mt-result").style.display = "none";
    popup.querySelector(".mt-error").style.display = "none";

    // Translate
    isTranslating = true;
    browser.runtime.sendMessage({
      type: "translate",
      text: text,
      from: "auto",
      to: "zh"
    }).then(response => {
      if (!isTranslating) return;

      popup.querySelector(".mt-loading").style.display = "none";

      if (response.success) {
        popup.querySelector(".mt-result").textContent = response.result;
        popup.querySelector(".mt-result").style.display = "block";
      } else {
        popup.querySelector(".mt-error").textContent = response.error;
        popup.querySelector(".mt-error").style.display = "block";
      }
    }).catch(err => {
      if (!isTranslating) return;
      popup.querySelector(".mt-loading").style.display = "none";
      popup.querySelector(".mt-error").textContent = "翻译请求失败";
      popup.querySelector(".mt-error").style.display = "block";
    });
  }

  // Hide popup
  function hidePopup() {
    if (popup) {
      popup.style.display = "none";
      isTranslating = false;
    }
  }

  // Listen for mouseup to detect text selection
  document.addEventListener("mouseup", (e) => {
    // Ignore if clicking inside popup
    if (popup && popup.contains(e.target)) return;

    const selection = window.getSelection();
    const text = selection.toString().trim();

    if (text.length > 0 && text.length < 5000) {
      // Get selection position
      const range = selection.getRangeAt(0);
      const rect = range.getBoundingClientRect();
      showPopup(text, rect.left, rect.bottom);
    }
  });

  // Hide popup on click outside
  document.addEventListener("mousedown", (e) => {
    if (popup && !popup.contains(e.target)) {
      hidePopup();
    }
  });

  // Hide popup on Escape
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      hidePopup();
    }
  });

  // Listen for context menu translation request
  browser.runtime.onMessage.addListener((message) => {
    if (message.type === "translate-selection") {
      const selection = window.getSelection();
      const range = selection.getRangeAt(0);
      const rect = range.getBoundingClientRect();
      showPopup(message.text, rect.left, rect.bottom);
    }
  });
})();
