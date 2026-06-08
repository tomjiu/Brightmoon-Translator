# Moon Translator Extension Tests

This directory contains unit tests and end-to-end tests for the Moon Translator browser extension.

## Directory Structure

```
extension/tests/
  __mocks__/
    chrome.js          # Chrome API mocks for unit testing
  unit/
    translation-cache.test.js    # TranslationCache tests
    message-handling.test.js     # Message routing and config tests
    hover-translator.test.js     # Hover translation logic tests
    page-translator.test.js      # Page translation and batch processing tests
  e2e/
    playwright.config.js         # Playwright configuration
    extension.spec.js            # E2E tests for the extension
```

## Running Tests

### Unit Tests

Run all extension unit tests:

```bash
npm run test:extension
```

Run unit tests in watch mode:

```bash
npm run test:extension:watch
```

### E2E Tests

**Prerequisites:** Build the extension first:

```bash
cd extension && node build.js
```

Run E2E tests:

```bash
npm run test:e2e
```

Run E2E tests with UI:

```bash
npm run test:e2e:ui
```

### Run All Tests

```bash
npm run test:all
```

## Test Coverage

### Unit Tests

#### TranslationCache (`translation-cache.test.js`)
- Service Worker cache (LRU, expiry, batch operations)
- Content Script cache (memory + sessionStorage)
- Cache key normalization
- Max size enforcement

#### Message Handling (`message-handling.test.js`)
- `translate` message handling
- `getConfig` / `saveConfig` messages
- `translatePageDesktop` batch translation
- Page control messages (`translatePage`, `restorePage`)
- Desktop status messages
- Unknown message types
- Config management and engine selection

#### Hover Translator (`hover-translator.test.js`)
- Skip tags (INPUT, TEXTAREA, BUTTON, etc.)
- Interactive element detection
- Text target selection
- Modifier key matching (alt, ctrl, shift, none)
- Delay validation (clamping, defaults)
- Min text length validation
- Block tag identification
- Config loading defaults

#### Page Translator (`page-translator.test.js`)
- Translation queue management
- Batch translation with cache
- Cache hit/miss handling
- Fallback to individual translation
- Group by parent logic
- Viewport detection (with 200px margin)
- CSS selector generation
- Text node filtering

### E2E Tests

#### Extension Installation (`extension.spec.js`)
- Extension loads correctly
- Content scripts are injected
- Translate button appears on pages
- Popup opens with all UI elements

#### Translation Functionality
- Text translation via popup
- Page translation toggle
- Hover translation tooltip
- Selection translation popup
- Context menu availability

#### Settings Page
- Settings panel toggle
- LLM settings visibility toggle
- Save settings functionality
- Language swap
- Auto language handling

## Writing New Tests

### Unit Tests

Unit tests use Vitest and test the extracted logic from extension scripts. To add a new test:

1. Create a test file in `extension/tests/unit/`
2. Import test utilities from vitest
3. Extract the logic you want to test into a function
4. Write test cases

Example:

```javascript
import { describe, it, expect } from "vitest";

function myFunction(input) {
  // Logic from extension code
  return input.toUpperCase();
}

describe("My Function", () => {
  it("should convert to uppercase", () => {
    expect(myFunction("hello")).toBe("HELLO");
  });
});
```

### E2E Tests

E2E tests use Playwright and require a built extension. To add a new test:

1. Add test cases to `extension/tests/e2e/extension.spec.js`
2. Use Playwright API to interact with the browser
3. Use `page.evaluate()` to check DOM state

Example:

```javascript
test("should do something", async () => {
  const page = await context.newPage();
  await page.goto("https://example.com");

  // Interact with the page
  await page.click("#my-button");

  // Check the result
  const result = await page.evaluate(() => {
    return document.getElementById("result").textContent;
  });

  expect(result).toBe("Expected text");
  await page.close();
});
```

## Chrome API Mocks

The `__mocks__/chrome.js` file provides mock implementations of Chrome extension APIs:

- `chrome.runtime` - Message passing, extension ID
- `chrome.storage` - Local storage
- `chrome.tabs` - Tab management
- `chrome.alarms` - Alarm scheduling
- `chrome.contextMenus` - Context menu creation
- `chrome.commands` - Keyboard shortcuts

Use the mock in tests:

```javascript
import { createChromeMock } from "../__mocks__/chrome.js";

const chrome = createChromeMock();
globalThis.chrome = chrome;

// Now you can use chrome APIs in your tests
```
