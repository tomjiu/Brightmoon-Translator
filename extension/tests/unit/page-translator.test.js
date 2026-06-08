import { describe, it, expect, beforeEach, vi } from "vitest";

// ==================== Page Translator Logic ====================
// Tests for batch translation, queue management, and page translation

// Batch configuration
const BATCH_DELAY_MS = 100;
const MAX_BATCH_SIZE = 10;

// Translation queue manager
function createTranslationQueue() {
  let queue = [];
  let batchTimeout = null;
  let processing = false;

  function addToQueue(item) {
    queue.push(item);

    if (batchTimeout) {
      clearTimeout(batchTimeout);
    }

    if (queue.length >= MAX_BATCH_SIZE) {
      return { shouldProcess: true, batch: queue.splice(0, MAX_BATCH_SIZE) };
    } else {
      return { shouldProcess: false, batch: null };
    }
  }

  function getBatch() {
    if (queue.length === 0) return null;
    return queue.splice(0, MAX_BATCH_SIZE);
  }

  function clear() {
    queue = [];
    if (batchTimeout) {
      clearTimeout(batchTimeout);
      batchTimeout = null;
    }
  }

  function size() {
    return queue.length;
  }

  function setProcessing(value) {
    processing = value;
  }

  function isProcessing() {
    return processing;
  }

  return { addToQueue, getBatch, clear, size, setProcessing, isProcessing };
}

// Batch translation with cache
function createBatchTranslator() {
  const cache = new Map();

  function cacheKey(text, from, to) {
    return `${from}:${to}:${text.trim().toLowerCase()}`;
  }

  function getCached(text, from, to) {
    return cache.get(cacheKey(text, from, to)) || null;
  }

  function setCache(text, from, to, result) {
    cache.set(cacheKey(text, from, to), result);
  }

  async function translateBatch(texts, from, to, sendMessage) {
    if (texts.length === 0) return [];

    const results = new Array(texts.length);
    const uncachedTexts = [];
    const uncachedIndices = [];

    // Check cache for each text
    for (let i = 0; i < texts.length; i++) {
      const cached = getCached(texts[i], from, to);
      if (cached) {
        results[i] = cached;
      } else {
        uncachedTexts.push(texts[i]);
        uncachedIndices.push(i);
      }
    }

    // If all cached, return immediately
    if (uncachedTexts.length === 0) return results;

    // Try batch translation
    try {
      const response = await sendMessage({
        type: "translatePageDesktop",
        segments: uncachedTexts.map((text, i) => ({
          text: text.trim(),
          index: i,
        })),
        from: from,
        to: to,
      });

      if (response.success && response.translations) {
        for (const t of response.translations) {
          const idx = uncachedIndices[t.index];
          if (idx !== undefined && t.translated) {
            results[idx] = {
              primary: { engine: "desktop", text: t.translated },
              results: [{ engine: "desktop", text: t.translated }],
            };
            setCache(texts[idx], from, to, results[idx]);
          }
        }
      }
    } catch (e) {
      // Fallback to individual translations
      for (let i = 0; i < uncachedTexts.length; i++) {
        const idx = uncachedIndices[i];
        if (!results[idx]) {
          try {
            const response = await sendMessage({
              type: "translate",
              text: uncachedTexts[i],
              from: from,
              to: to,
            });
            results[idx] = response;
            setCache(texts[idx], from, to, response);
          } catch (err) {
            results[idx] = null;
          }
        }
      }
    }

    return results;
  }

  return { translateBatch, getCached, setCache };
}

// Group text nodes by parent
function groupByParent(nodes) {
  const groups = new Map();
  nodes.forEach((node) => {
    const parent = node.parent;
    if (!groups.has(parent)) {
      groups.set(parent, []);
    }
    groups.get(parent).push(node);
  });
  return groups;
}

// Check if element is in viewport
function isElementInViewport(rect, viewport) {
  return (
    rect.top < viewport.height + 200 &&
    rect.bottom > -200 &&
    rect.left < viewport.width + 200 &&
    rect.right > -200
  );
}

// CSS selector generation
function getCssSelector(node) {
  const parts = [];
  let el = node.parent;

  while (el && el.tagName !== "BODY") {
    let selector = el.tagName.toLowerCase();

    if (el.id) {
      selector = `#${el.id}`;
      parts.unshift(selector);
      break;
    }

    if (el.className) {
      const cls = el.className
        .trim()
        .split(/\s+/)
        .filter((c) => !c.startsWith("moon-"))
        .slice(0, 2)
        .join(".");
      if (cls) selector += `.${cls}`;
    }

    parts.unshift(selector);
    el = el.parent;
  }

  return parts.join(" > ") || "body";
}

// ==================== Tests ====================

describe("Page Translator - Translation Queue", () => {
  let queue;

  beforeEach(() => {
    queue = createTranslationQueue();
  });

  it("should add items to queue", () => {
    queue.addToQueue({ node: "node1", text: "hello" });
    expect(queue.size()).toBe(1);
  });

  it("should return shouldProcess when batch is full", () => {
    for (let i = 0; i < MAX_BATCH_SIZE; i++) {
      const result = queue.addToQueue({ node: `node${i}`, text: `text${i}` });
      if (i < MAX_BATCH_SIZE - 1) {
        expect(result.shouldProcess).toBe(false);
      } else {
        expect(result.shouldProcess).toBe(true);
        expect(result.batch).toHaveLength(MAX_BATCH_SIZE);
      }
    }
    expect(queue.size()).toBe(0);
  });

  it("should get batch of remaining items", () => {
    queue.addToQueue({ node: "node1", text: "hello" });
    queue.addToQueue({ node: "node2", text: "world" });

    const batch = queue.getBatch();
    expect(batch).toHaveLength(2);
    expect(queue.size()).toBe(0);
  });

  it("should return null when queue is empty", () => {
    const batch = queue.getBatch();
    expect(batch).toBeNull();
  });

  it("should clear queue", () => {
    queue.addToQueue({ node: "node1", text: "hello" });
    queue.addToQueue({ node: "node2", text: "world" });

    queue.clear();
    expect(queue.size()).toBe(0);
  });

  it("should track processing state", () => {
    expect(queue.isProcessing()).toBe(false);
    queue.setProcessing(true);
    expect(queue.isProcessing()).toBe(true);
  });
});

describe("Page Translator - Batch Translation", () => {
  let translator;
  let mockSendMessage;

  beforeEach(() => {
    translator = createBatchTranslator();
    mockSendMessage = vi.fn();
  });

  it("should return empty array for empty input", async () => {
    const results = await translator.translateBatch([], "auto", "zh", mockSendMessage);
    expect(results).toEqual([]);
    expect(mockSendMessage).not.toHaveBeenCalled();
  });

  it("should use cache for known translations", async () => {
    translator.setCache("hello", "auto", "zh", {
      primary: { text: "你好" },
    });

    const results = await translator.translateBatch(
      ["hello"],
      "auto",
      "zh",
      mockSendMessage
    );

    expect(results[0].primary.text).toBe("你好");
    expect(mockSendMessage).not.toHaveBeenCalled();
  });

  it("should translate uncached texts", async () => {
    mockSendMessage.mockResolvedValueOnce({
      success: true,
      translations: [{ index: 0, translated: "你好" }],
    });

    const results = await translator.translateBatch(
      ["hello"],
      "auto",
      "zh",
      mockSendMessage
    );

    expect(results[0].primary.text).toBe("你好");
    expect(mockSendMessage).toHaveBeenCalledWith({
      type: "translatePageDesktop",
      segments: [{ text: "hello", index: 0 }],
      from: "auto",
      to: "zh",
    });
  });

  it("should handle mixed cached and uncached texts", async () => {
    translator.setCache("cached", "auto", "zh", {
      primary: { text: "已缓存" },
    });

    mockSendMessage.mockResolvedValueOnce({
      success: true,
      translations: [{ index: 0, translated: "新翻译" }],
    });

    const results = await translator.translateBatch(
      ["cached", "new"],
      "auto",
      "zh",
      mockSendMessage
    );

    expect(results[0].primary.text).toBe("已缓存");
    expect(results[1].primary.text).toBe("新翻译");
  });

  it("should cache translation results", async () => {
    mockSendMessage.mockResolvedValueOnce({
      success: true,
      translations: [{ index: 0, translated: "你好" }],
    });

    await translator.translateBatch(["hello"], "auto", "zh", mockSendMessage);

    const cached = translator.getCached("hello", "auto", "zh");
    expect(cached).toBeDefined();
    expect(cached.primary.text).toBe("你好");
  });

  it("should fallback to individual translation on batch failure", async () => {
    mockSendMessage
      .mockRejectedValueOnce(new Error("Batch failed"))
      .mockResolvedValueOnce({
        success: true,
        primary: { text: "你好" },
      });

    const results = await translator.translateBatch(
      ["hello"],
      "auto",
      "zh",
      mockSendMessage
    );

    expect(results[0].primary.text).toBe("你好");
    expect(mockSendMessage).toHaveBeenCalledTimes(2);
  });
});

describe("Page Translator - Group By Parent", () => {
  it("should group nodes by parent", () => {
    const nodes = [
      { id: 1, parent: "parent1" },
      { id: 2, parent: "parent1" },
      { id: 3, parent: "parent2" },
    ];

    const groups = groupByParent(nodes);

    expect(groups.size).toBe(2);
    expect(groups.get("parent1")).toHaveLength(2);
    expect(groups.get("parent2")).toHaveLength(1);
  });

  it("should handle single parent", () => {
    const nodes = [
      { id: 1, parent: "parent1" },
      { id: 2, parent: "parent1" },
      { id: 3, parent: "parent1" },
    ];

    const groups = groupByParent(nodes);

    expect(groups.size).toBe(1);
    expect(groups.get("parent1")).toHaveLength(3);
  });

  it("should handle unique parents", () => {
    const nodes = [
      { id: 1, parent: "parent1" },
      { id: 2, parent: "parent2" },
      { id: 3, parent: "parent3" },
    ];

    const groups = groupByParent(nodes);

    expect(groups.size).toBe(3);
  });
});

describe("Page Translator - Viewport Detection", () => {
  it("should detect elements in viewport", () => {
    const rect = { top: 100, bottom: 200, left: 100, right: 200 };
    const viewport = { width: 1024, height: 768 };

    expect(isElementInViewport(rect, viewport)).toBe(true);
  });

  it("should detect elements above viewport with margin", () => {
    const rect = { top: -150, bottom: -50, left: 100, right: 200 };
    const viewport = { width: 1024, height: 768 };

    // Within 200px margin
    expect(isElementInViewport(rect, viewport)).toBe(true);
  });

  it("should reject elements far above viewport", () => {
    const rect = { top: -300, bottom: -250, left: 100, right: 200 };
    const viewport = { width: 1024, height: 768 };

    expect(isElementInViewport(rect, viewport)).toBe(false);
  });

  it("should detect elements below viewport with margin", () => {
    const rect = { top: 900, bottom: 950, left: 100, right: 200 };
    const viewport = { width: 1024, height: 768 };

    // 768 + 200 = 968, so 900 < 968
    expect(isElementInViewport(rect, viewport)).toBe(true);
  });

  it("should reject elements far below viewport", () => {
    const rect = { top: 1100, bottom: 1200, left: 100, right: 200 };
    const viewport = { width: 1024, height: 768 };

    expect(isElementInViewport(rect, viewport)).toBe(false);
  });
});

describe("Page Translator - CSS Selector", () => {
  it("should generate selector from node hierarchy", () => {
    const node = {
      parent: {
        tagName: "DIV",
        className: "container main",
        parent: {
          tagName: "ARTICLE",
          id: "content",
          parent: { tagName: "BODY" },
        },
      },
    };

    const selector = getCssSelector(node);
    expect(selector).toBe("#content > div.container.main");
  });

  it("should use id when available", () => {
    const node = {
      parent: {
        tagName: "DIV",
        id: "my-element",
        parent: { tagName: "BODY" },
      },
    };

    const selector = getCssSelector(node);
    expect(selector).toBe("#my-element");
  });

  it("should filter moon- prefixed classes", () => {
    const node = {
      parent: {
        tagName: "DIV",
        className: "moon-tooltip active",
        parent: { tagName: "BODY" },
      },
    };

    const selector = getCssSelector(node);
    expect(selector).toBe("div.active");
  });

  it("should return 'body' for direct body children", () => {
    const node = {
      parent: { tagName: "BODY" },
    };

    const selector = getCssSelector(node);
    expect(selector).toBe("body");
  });
});

describe("Page Translator - Batch Configuration", () => {
  it("should have correct batch delay", () => {
    expect(BATCH_DELAY_MS).toBe(100);
  });

  it("should have correct max batch size", () => {
    expect(MAX_BATCH_SIZE).toBe(10);
  });
});

describe("Page Translator - Text Node Filtering", () => {
  // Simulate text node filtering logic
  function shouldAcceptNode(tagName, textContent, isHidden, startsWithMoon) {
    const skipTags = ["script", "style", "noscript", "code", "pre", "svg"];
    if (skipTags.includes(tagName.toLowerCase())) return false;
    if (isHidden) return false;
    if (startsWithMoon) return false;
    if (!textContent.trim()) return false;
    return true;
  }

  it("should accept valid text nodes", () => {
    expect(shouldAcceptNode("p", "Hello world", false, false)).toBe(true);
  });

  it("should reject script nodes", () => {
    expect(shouldAcceptNode("script", "var x = 1;", false, false)).toBe(false);
  });

  it("should reject style nodes", () => {
    expect(shouldAcceptNode("style", ".class { color: red; }", false, false)).toBe(false);
  });

  it("should reject hidden elements", () => {
    expect(shouldAcceptNode("div", "Hidden text", true, false)).toBe(false);
  });

  it("should reject moon- prefixed elements", () => {
    expect(shouldAcceptNode("div", "Moon element", false, true)).toBe(false);
  });

  it("should reject empty text", () => {
    expect(shouldAcceptNode("p", "   ", false, false)).toBe(false);
  });

  it("should reject whitespace-only text", () => {
    expect(shouldAcceptNode("p", "\n\t  ", false, false)).toBe(false);
  });
});
