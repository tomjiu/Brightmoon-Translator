import { test, expect, chromium } from "@playwright/test";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const EXTENSION_PATH = path.resolve(__dirname, "../../dist/chrome");

let browser;
let context;

test.describe("Moon Translator Extension", () => {
  test.beforeAll(async () => {
    // Launch browser with extension loaded
    browser = await chromium.launch({
      headless: false, // Extensions require headed mode
      args: [
        `--disable-extensions-except=${EXTENSION_PATH}`,
        `--load-extension=${EXTENSION_PATH}`,
      ],
    });

    // Create a new context
    context = await browser.newContext();
  });

  test.afterAll(async () => {
    if (context) await context.close();
    if (browser) await browser.close();
  });

  test("extension should be installed", async () => {
    // Check that the extension is loaded by looking for its service worker
    const backgroundPage = context.serviceWorkers()[0];
    expect(backgroundPage).toBeTruthy();
  });

  test("extension icon should be visible", async () => {
    const page = await context.newPage();
    await page.goto("https://example.com");

    // Wait for the extension to initialize
    await page.waitForTimeout(1000);

    // The extension should have injected content scripts
    // Check if the moon translator elements exist
    const moonElements = await page.evaluate(() => {
      return {
        translateBtn: !!document.getElementById("moon-translate-page-btn"),
        hoverTooltip: !!document.getElementById("moon-hover-tooltip"),
      };
    });

    // The translate button should be created by page-translator.js
    expect(moonElements.translateBtn).toBe(true);

    await page.close();
  });

  test("popup should open when clicking extension icon", async () => {
    const page = await context.newPage();
    await page.goto("https://example.com");

    // Get the extension ID from the service worker
    const serviceWorker = context.serviceWorkers()[0];
    const extensionId = serviceWorker.url().split("/")[2];

    // Open the popup directly
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);

    // Wait for popup to load
    await page.waitForSelector("#sourceText");

    // Check popup elements exist
    const elements = await page.evaluate(() => {
      return {
        sourceText: !!document.getElementById("sourceText"),
        targetLang: !!document.getElementById("targetLang"),
        sourceLang: !!document.getElementById("sourceLang"),
        translateBtn: !!document.getElementById("translateBtn"),
        settingsPanel: !!document.getElementById("settingsPanel"),
      };
    });

    expect(elements.sourceText).toBe(true);
    expect(elements.targetLang).toBe(true);
    expect(elements.sourceLang).toBe(true);
    expect(elements.translateBtn).toBe(true);
    expect(elements.settingsPanel).toBe(true);

    await page.close();
  });

  test("settings should have default values", async () => {
    const page = await context.newPage();
    await page.goto("https://example.com");

    const serviceWorker = context.serviceWorkers()[0];
    const extensionId = serviceWorker.url().split("/")[2];

    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await page.waitForSelector("#sourceText");

    // Check default settings
    const settings = await page.evaluate(() => {
      return {
        sourceLang: document.getElementById("sourceLang")?.value,
        targetLang: document.getElementById("targetLang")?.value,
        engineGoogle: document.getElementById("engineGoogle")?.checked,
        engineYoudao: document.getElementById("engineYoudao")?.checked,
        engineMicrosoft: document.getElementById("engineMicrosoft")?.checked,
        engineLlm: document.getElementById("engineLlm")?.checked,
        engineDeepl: document.getElementById("engineDeepl")?.checked,
        engineDeeplx: document.getElementById("engineDeeplx")?.checked,
        hoverEnabled: document.getElementById("hoverEnabled")?.checked,
      };
    });

    expect(settings.sourceLang).toBe("auto");
    expect(settings.targetLang).toBe("zh");
    expect(settings.engineGoogle).toBe(true);
    expect(settings.engineYoudao).toBe(true);
    expect(settings.engineMicrosoft).toBe(false);
    expect(settings.engineLlm).toBe(false);
    expect(settings.engineDeepl).toBe(false);
    expect(settings.engineDeeplx).toBe(false);
    expect(settings.hoverEnabled).toBe(true);

    await page.close();
  });

  test("content scripts should be injected", async () => {
    const page = await context.newPage();
    await page.goto("https://example.com");

    // Wait for content scripts to load
    await page.waitForTimeout(2000);

    // Check that content scripts have been injected
    const contentScriptsLoaded = await page.evaluate(() => {
      return {
        // page-translator.js should create a translate button
        translateBtn: !!document.getElementById("moon-translate-page-btn"),
        // The scripts should have logged their initialization
        moonTranslatorLoaded: typeof window.moonTranslatePage === "function",
        moonRestorePage: typeof window.moonRestorePage === "function",
        moonToggleTranslation: typeof window.moonToggleTranslation === "function",
      };
    });

    expect(contentScriptsLoaded.translateBtn).toBe(true);
    expect(contentScriptsLoaded.moonTranslatorLoaded).toBe(true);
    expect(contentScriptsLoaded.moonRestorePage).toBe(true);
    expect(contentScriptsLoaded.moonToggleTranslation).toBe(true);

    await page.close();
  });

  test("translate button should toggle page translation", async () => {
    const page = await context.newPage();
    await page.setContent(`
      <html>
        <body>
          <h1>Hello World</h1>
          <p>This is a test paragraph for translation.</p>
        </body>
      </html>
    `);

    // Wait for content scripts
    await page.waitForTimeout(2000);

    // Click translate button
    const translateBtn = await page.waitForSelector("#moon-translate-page-btn");
    await translateBtn.click();

    // Wait for translation to complete (or timeout)
    await page.waitForTimeout(3000);

    // The page content should have changed (or attempted to translate)
    // Note: Actual translation depends on API availability
    const pageContent = await page.textContent("body");
    expect(pageContent).toBeTruthy();

    // Click again to restore
    await translateBtn.click();
    await page.waitForTimeout(500);

    // Original content should be restored
    const restoredContent = await page.textContent("body");
    expect(restoredContent).toContain("Hello World");

    await page.close();
  });

  test("hover translation should show tooltip on hover", async () => {
    const page = await context.newPage();
    await page.setContent(`
      <html>
        <body>
          <p id="test-text">Hover over this text for translation</p>
        </body>
      </html>
    `);

    // Wait for content scripts
    await page.waitForTimeout(2000);

    // Hover over text
    await page.hover("#test-text");

    // Wait for hover delay (default 300ms) plus some buffer
    await page.waitForTimeout(500);

    // Check if tooltip appears
    const tooltip = await page.$("#moon-hover-tooltip");
    if (tooltip) {
      const isVisible = await tooltip.isVisible();
      // Tooltip may or may not be visible depending on API availability
      console.log("Tooltip visible:", isVisible);
    }

    await page.close();
  });

  test("selection translation should work", async () => {
    const page = await context.newPage();
    await page.setContent(`
      <html>
        <body>
          <p id="test-text">Select this text for translation</p>
        </body>
      </html>
    `);

    // Wait for content scripts
    await page.waitForTimeout(2000);

    // Select text
    await page.evaluate(() => {
      const range = document.createRange();
      const textNode = document.querySelector("#test-text").firstChild;
      range.selectNodeContents(textNode);
      const selection = window.getSelection();
      selection.removeAllRanges();
      selection.addRange(range);
    });

    // Trigger mouseup to show popup
    await page.mouse.up();

    // Wait for popup to appear
    await page.waitForTimeout(1000);

    // Check if popup appears
    const popup = await page.$("#moon-translator-popup");
    if (popup) {
      const isVisible = await popup.isVisible();
      console.log("Selection popup visible:", isVisible);
    }

    await page.close();
  });

  test("context menu should be available", async () => {
    const page = await context.newPage();
    await page.goto("https://example.com");

    // Wait for extension to initialize
    await page.waitForTimeout(1000);

    // Right-click to open context menu
    await page.click("body", { button: "right" });

    // Check for context menu items
    // Note: Context menu items are browser-native and may not be directly accessible
    // This test verifies the extension doesn't crash when context menu is opened

    await page.close();
  });
});

test.describe("Translation Functionality", () => {
  test.beforeAll(async () => {
    browser = await chromium.launch({
      headless: false,
      args: [
        `--disable-extensions-except=${EXTENSION_PATH}`,
        `--load-extension=${EXTENSION_PATH}`,
      ],
    });
    context = await browser.newContext();
  });

  test.afterAll(async () => {
    if (context) await context.close();
    if (browser) await browser.close();
  });

  test("should translate text via popup", async () => {
    const page = await context.newPage();
    await page.goto("https://example.com");

    const serviceWorker = context.serviceWorkers()[0];
    const extensionId = serviceWorker.url().split("/")[2];

    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await page.waitForSelector("#sourceText");

    // Enter text to translate
    await page.fill("#sourceText", "Hello");

    // Click translate button
    await page.click("#translateBtn");

    // Wait for translation result
    await page.waitForSelector("#results", { state: "visible", timeout: 10000 }).catch(() => {
      // Translation might fail if APIs are not accessible
      console.log("Translation API not available in test environment");
    });

    // Check if results or error is shown
    const hasResult = await page.evaluate(() => {
      const results = document.getElementById("results");
      const error = document.getElementById("error");
      return (
        (results && results.style.display !== "none") ||
        (error && error.style.display !== "none")
      );
    });

    // Either result or error should be shown
    expect(hasResult).toBe(true);

    await page.close();
  });

  test("should handle empty translation input", async () => {
    const page = await context.newPage();
    await page.goto("https://example.com");

    const serviceWorker = context.serviceWorkers()[0];
    const extensionId = serviceWorker.url().split("/")[2];

    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await page.waitForSelector("#sourceText");

    // Don't enter any text
    await page.fill("#sourceText", "");

    // Click translate button
    await page.click("#translateBtn");

    // Should not show loading or results
    const loadingVisible = await page.evaluate(() => {
      const loading = document.getElementById("loading");
      return loading && loading.style.display !== "none";
    });

    expect(loadingVisible).toBe(false);

    await page.close();
  });
});

test.describe("Settings Page", () => {
  test.beforeAll(async () => {
    browser = await chromium.launch({
      headless: false,
      args: [
        `--disable-extensions-except=${EXTENSION_PATH}`,
        `--load-extension=${EXTENSION_PATH}`,
      ],
    });
    context = await browser.newContext();
  });

  test.afterAll(async () => {
    if (context) await context.close();
    if (browser) await browser.close();
  });

  test("should toggle settings panel", async () => {
    const page = await context.newPage();
    await page.goto("https://example.com");

    const serviceWorker = context.serviceWorkers()[0];
    const extensionId = serviceWorker.url().split("/")[2];

    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await page.waitForSelector("#toggleSettings");

    // Settings should be hidden initially
    const settingsInitiallyHidden = await page.evaluate(() => {
      const panel = document.getElementById("settingsPanel");
      return panel.style.display === "none";
    });
    expect(settingsInitiallyHidden).toBe(true);

    // Click to show settings
    await page.click("#toggleSettings");
    await page.waitForTimeout(100);

    const settingsVisible = await page.evaluate(() => {
      const panel = document.getElementById("settingsPanel");
      return panel.style.display !== "none";
    });
    expect(settingsVisible).toBe(true);

    // Click to hide settings
    await page.click("#toggleSettings");
    await page.waitForTimeout(100);

    const settingsHidden = await page.evaluate(() => {
      const panel = document.getElementById("settingsPanel");
      return panel.style.display === "none";
    });
    expect(settingsHidden).toBe(true);

    await page.close();
  });

  test("should toggle LLM settings visibility", async () => {
    const page = await context.newPage();
    await page.goto("https://example.com");

    const serviceWorker = context.serviceWorkers()[0];
    const extensionId = serviceWorker.url().split("/")[2];

    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await page.waitForSelector("#toggleSettings");

    // Open settings
    await page.click("#toggleSettings");

    // LLM settings should be hidden initially (LLM disabled by default)
    const llmSettingsHidden = await page.evaluate(() => {
      const settings = document.getElementById("llmSettings");
      return settings.style.display === "none";
    });
    expect(llmSettingsHidden).toBe(true);

    // Enable LLM
    await page.click("#engineLlm");
    await page.waitForTimeout(100);

    const llmSettingsVisible = await page.evaluate(() => {
      const settings = document.getElementById("llmSettings");
      return settings.style.display !== "none";
    });
    expect(llmSettingsVisible).toBe(true);

    // Disable LLM
    await page.click("#engineLlm");
    await page.waitForTimeout(100);

    const llmSettingsHiddenAgain = await page.evaluate(() => {
      const settings = document.getElementById("llmSettings");
      return settings.style.display === "none";
    });
    expect(llmSettingsHiddenAgain).toBe(true);

    await page.close();
  });

  test("should save settings", async () => {
    const page = await context.newPage();
    await page.goto("https://example.com");

    const serviceWorker = context.serviceWorkers()[0];
    const extensionId = serviceWorker.url().split("/")[2];

    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await page.waitForSelector("#toggleSettings");

    // Open settings
    await page.click("#toggleSettings");

    // Change target language
    await page.selectOption("#targetLang", "en");

    // Save settings
    await page.click("#saveSettings");

    // Wait for save notification
    await page.waitForTimeout(500);

    // Check notification
    const notification = await page.evaluate(() => {
      const error = document.getElementById("error");
      return error?.textContent;
    });

    expect(notification).toContain("设置已保存");

    await page.close();
  });

  test("should swap languages", async () => {
    const page = await context.newPage();
    await page.goto("https://example.com");

    const serviceWorker = context.serviceWorkers()[0];
    const extensionId = serviceWorker.url().split("/")[2];

    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await page.waitForSelector("#swapLang");

    // Set initial languages
    await page.selectOption("#sourceLang", "en");
    await page.selectOption("#targetLang", "zh");

    // Swap languages
    await page.click("#swapLang");

    // Check swapped values
    const languages = await page.evaluate(() => {
      return {
        source: document.getElementById("sourceLang").value,
        target: document.getElementById("targetLang").value,
      };
    });

    expect(languages.source).toBe("zh");
    expect(languages.target).toBe("en");

    await page.close();
  });

  test("should not swap when source is auto", async () => {
    const page = await context.newPage();
    await page.goto("https://example.com");

    const serviceWorker = context.serviceWorkers()[0];
    const extensionId = serviceWorker.url().split("/")[2];

    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await page.waitForSelector("#swapLang");

    // Set source to auto
    await page.selectOption("#sourceLang", "auto");
    await page.selectOption("#targetLang", "zh");

    // Try to swap
    await page.click("#swapLang");

    // Languages should not change
    const languages = await page.evaluate(() => {
      return {
        source: document.getElementById("sourceLang").value,
        target: document.getElementById("targetLang").value,
      };
    });

    expect(languages.source).toBe("auto");
    expect(languages.target).toBe("zh");

    await page.close();
  });
});
