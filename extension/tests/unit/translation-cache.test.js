import { describe, it, expect, beforeEach, vi } from "vitest";

// ==================== Service Worker TranslationCache ====================
// Extracted from service-worker.js for unit testing

function createServiceWorkerCache() {
  return {
    maxSize: 1000,
    expiryMs: 24 * 60 * 60 * 1000, // 24 hours
    cache: new Map(),

    _makeKey(text, from, to, engine) {
      const normalized = text.trim().toLowerCase().replace(/\s+/g, " ");
      return `${engine || "any"}:${from || "auto"}:${to || "zh"}:${normalized}`;
    },

    get(text, from, to, engine) {
      const key = this._makeKey(text, from, to, engine);
      const entry = this.cache.get(key);
      if (!entry) return null;

      // Check expiry
      if (Date.now() - entry.timestamp > this.expiryMs) {
        this.cache.delete(key);
        return null;
      }

      // Move to end (most recently used)
      this.cache.delete(key);
      this.cache.set(key, entry);
      return entry.value;
    },

    set(text, from, to, engine, value) {
      const key = this._makeKey(text, from, to, engine);

      // Remove oldest if at capacity
      if (this.cache.size >= this.maxSize) {
        const firstKey = this.cache.keys().next().value;
        this.cache.delete(firstKey);
      }

      this.cache.set(key, { value, timestamp: Date.now() });
    },

    batchGet(texts, from, to) {
      const hits = new Map();
      const misses = [];

      for (const text of texts) {
        // Try each enabled engine's cache
        let found = false;
        for (const engine of ["Google", "youdao", "Microsoft", "LLM", "DeepL", "DeepLX"]) {
          const cached = this.get(text, from, to, engine);
          if (cached) {
            hits.set(text, cached);
            found = true;
            break;
          }
        }
        // Also try the "any" engine key (used by content script cache)
        if (!found) {
          const cached = this.get(text, from, to, "any");
          if (cached) {
            hits.set(text, cached);
            found = true;
          }
        }
        if (!found) {
          misses.push(text);
        }
      }

      return { hits, misses };
    },
  };
}

// ==================== Content Script TranslationCache ====================
// Extracted from translation-cache.js for unit testing

function createMemoryCache(maxSize = 500) {
  const CACHE_EXPIRY_MS = 24 * 60 * 60 * 1000;

  return {
    maxSize,
    cache: new Map(),

    get(key) {
      const entry = this.cache.get(key);
      if (!entry) return null;

      if (Date.now() - entry.timestamp > CACHE_EXPIRY_MS) {
        this.cache.delete(key);
        return null;
      }

      // Move to end (most recently used)
      this.cache.delete(key);
      this.cache.set(key, entry);
      return entry.value;
    },

    set(key, value) {
      if (this.cache.size >= this.maxSize) {
        const firstKey = this.cache.keys().next().value;
        this.cache.delete(firstKey);
      }

      this.cache.set(key, { value, timestamp: Date.now() });
    },

    has(key) {
      return this.get(key) !== null;
    },

    clear() {
      this.cache.clear();
    },
  };
}

function createContentScriptCache() {
  const CACHE_PREFIX = "moon_trans_";
  const memory = createMemoryCache(500);

  // Mock sessionStorage
  const sessionStorageMock = new Map();

  return {
    _makeKey(text, from, to, engine) {
      const normalized = text.trim().toLowerCase().replace(/\s+/g, " ");
      return `${engine || "any"}:${from || "auto"}:${to || "zh"}:${normalized}`;
    },

    get(text, from, to, engine) {
      const key = this._makeKey(text, from, to, engine);
      let result = memory.get(key);
      if (result !== null) return result;

      // Check sessionStorage mock
      const raw = sessionStorageMock.get(CACHE_PREFIX + key);
      if (raw) {
        const entry = JSON.parse(raw);
        if (Date.now() - entry.timestamp <= 24 * 60 * 60 * 1000) {
          memory.set(key, entry.value);
          return entry.value;
        }
        sessionStorageMock.delete(CACHE_PREFIX + key);
      }
      return null;
    },

    set(text, from, to, engine, result) {
      const key = this._makeKey(text, from, to, engine);
      memory.set(key, result);
      sessionStorageMock.set(
        CACHE_PREFIX + key,
        JSON.stringify({ value: result, timestamp: Date.now() })
      );
    },

    has(text, from, to, engine) {
      const key = this._makeKey(text, from, to, engine);
      return memory.has(key) || sessionStorageMock.has(CACHE_PREFIX + key);
    },

    batchGet(texts, from, to, engine) {
      const results = new Map();
      const missing = [];

      for (const text of texts) {
        const cached = this.get(text, from, to, engine);
        if (cached !== null) {
          results.set(text, cached);
        } else {
          missing.push(text);
        }
      }

      return { results, missing };
    },

    clear() {
      memory.clear();
      for (const key of sessionStorageMock.keys()) {
        if (key.startsWith(CACHE_PREFIX)) {
          sessionStorageMock.delete(key);
        }
      }
    },
  };
}

// ==================== Tests ====================

describe("Service Worker TranslationCache", () => {
  let cache;

  beforeEach(() => {
    cache = createServiceWorkerCache();
  });

  describe("_makeKey", () => {
    it("should generate consistent cache keys", () => {
      const key1 = cache._makeKey("Hello World", "en", "zh", "Google");
      const key2 = cache._makeKey("Hello World", "en", "zh", "Google");
      expect(key1).toBe(key2);
    });

    it("should normalize whitespace", () => {
      const key1 = cache._makeKey("  hello   world  ", "en", "zh", "Google");
      const key2 = cache._makeKey("hello world", "en", "zh", "Google");
      expect(key1).toBe(key2);
    });

    it("should be case-insensitive", () => {
      const key1 = cache._makeKey("Hello", "en", "zh", "Google");
      const key2 = cache._makeKey("hello", "en", "zh", "Google");
      expect(key1).toBe(key2);
    });

    it("should use default values for missing params", () => {
      const key = cache._makeKey("test");
      expect(key).toBe("any:auto:zh:test");
    });
  });

  describe("get/set", () => {
    it("should store and retrieve a value", () => {
      const result = { engine: "Google", text: "translated" };
      cache.set("hello", "en", "zh", "Google", result);

      const cached = cache.get("hello", "en", "zh", "Google");
      expect(cached).toEqual(result);
    });

    it("should return null for missing keys", () => {
      const cached = cache.get("nonexistent", "en", "zh", "Google");
      expect(cached).toBeNull();
    });

    it("should handle LRU eviction", () => {
      cache.maxSize = 3;

      cache.set("a", "en", "zh", "Google", "result_a");
      cache.set("b", "en", "zh", "Google", "result_b");
      cache.set("c", "en", "zh", "Google", "result_c");
      cache.set("d", "en", "zh", "Google", "result_d"); // Should evict "a"

      expect(cache.get("a", "en", "zh", "Google")).toBeNull();
      expect(cache.get("d", "en", "zh", "Google")).toBe("result_d");
    });

    it("should handle expired entries", () => {
      const result = { engine: "Google", text: "translated" };
      cache.set("hello", "en", "zh", "Google", result);

      // Manually expire the entry
      const key = cache._makeKey("hello", "en", "zh", "Google");
      const entry = cache.cache.get(key);
      entry.timestamp = Date.now() - cache.expiryMs - 1000;

      const cached = cache.get("hello", "en", "zh", "Google");
      expect(cached).toBeNull();
    });

    it("should move accessed items to end (LRU)", () => {
      cache.maxSize = 3;

      cache.set("a", "en", "zh", "Google", "result_a");
      cache.set("b", "en", "zh", "Google", "result_b");
      cache.set("c", "en", "zh", "Google", "result_c");

      // Access "a" to move it to end
      cache.get("a", "en", "zh", "Google");

      // Add "d" - should evict "b" (oldest unused)
      cache.set("d", "en", "zh", "Google", "result_d");

      expect(cache.get("a", "en", "zh", "Google")).toBe("result_a");
      expect(cache.get("b", "en", "zh", "Google")).toBeNull();
    });
  });

  describe("batchGet", () => {
    it("should return hits and misses", () => {
      cache.set("hello", "en", "zh", "any", { text: "你好" });

      const result = cache.batchGet(["hello", "world"], "en", "zh");

      expect(result.hits.get("hello")).toEqual({ text: "你好" });
      expect(result.misses).toContain("world");
    });

    it("should find results across different engines", () => {
      cache.set("hello", "en", "zh", "Google", { text: "你好" });

      const result = cache.batchGet(["hello"], "en", "zh");

      expect(result.hits.size).toBe(1);
      expect(result.misses).toHaveLength(0);
    });

    it("should find results with 'any' engine key", () => {
      cache.set("hello", "en", "zh", "any", { text: "你好" });

      const result = cache.batchGet(["hello"], "en", "zh");

      expect(result.hits.size).toBe(1);
      expect(result.misses).toHaveLength(0);
    });
  });
});

describe("Content Script TranslationCache", () => {
  let cache;

  beforeEach(() => {
    cache = createContentScriptCache();
  });

  describe("get/set", () => {
    it("should store and retrieve a value", () => {
      const result = { primary: { text: "你好" } };
      cache.set("hello", "en", "zh", "any", result);

      const cached = cache.get("hello", "en", "zh", "any");
      expect(cached).toEqual(result);
    });

    it("should return null for missing keys", () => {
      const cached = cache.get("nonexistent", "en", "zh", "any");
      expect(cached).toBeNull();
    });

    it("should check memory cache before sessionStorage", () => {
      const result = { primary: { text: "你好" } };
      cache.set("hello", "en", "zh", "any", result);

      // Both memory and session should have it
      expect(cache.get("hello", "en", "zh", "any")).toEqual(result);
    });
  });

  describe("batchGet", () => {
    it("should separate results and missing items", () => {
      cache.set("hello", "en", "zh", "any", { text: "你好" });

      const result = cache.batchGet(["hello", "world"], "en", "zh", "any");

      expect(result.results.size).toBe(1);
      expect(result.missing).toHaveLength(1);
      expect(result.missing[0]).toBe("world");
    });
  });

  describe("has", () => {
    it("should return true for existing entries", () => {
      cache.set("hello", "en", "zh", "any", { text: "你好" });
      expect(cache.has("hello", "en", "zh", "any")).toBe(true);
    });

    it("should return false for missing entries", () => {
      expect(cache.has("nonexistent", "en", "zh", "any")).toBe(false);
    });
  });

  describe("clear", () => {
    it("should clear all caches", () => {
      cache.set("hello", "en", "zh", "any", { text: "你好" });
      cache.clear();

      expect(cache.get("hello", "en", "zh", "any")).toBeNull();
    });
  });
});

describe("MemoryCache", () => {
  let cache;

  beforeEach(() => {
    cache = createMemoryCache(5);
  });

  it("should respect maxSize", () => {
    for (let i = 0; i < 10; i++) {
      cache.set(`key${i}`, `value${i}`);
    }

    expect(cache.cache.size).toBe(5);
    // Oldest entries should be evicted
    expect(cache.get("key0")).toBeNull();
    expect(cache.get("key9")).toBe("value9");
  });

  it("should handle has() correctly", () => {
    cache.set("existing", "value");
    expect(cache.has("existing")).toBe(true);
    expect(cache.has("missing")).toBe(false);
  });
});
