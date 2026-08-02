// Translation Cache Module for Moon Translator
// Provides in-memory and sessionStorage caching for translation results

(function() {
  "use strict";

  const CACHE_PREFIX = "moon_trans_";
  const MAX_CACHE_SIZE = 500;
  const CACHE_EXPIRY_MS = 24 * 60 * 60 * 1000; // 24 hours

  // In-memory LRU cache for fast access
  class MemoryCache {
    constructor(maxSize) {
      this.maxSize = maxSize;
      this.cache = new Map();
    }

    get(key) {
      const entry = this.cache.get(key);
      if (!entry) return null;

      // Check expiry
      if (Date.now() - entry.timestamp > CACHE_EXPIRY_MS) {
        this.cache.delete(key);
        return null;
      }

      // Move to end (most recently used)
      this.cache.delete(key);
      this.cache.set(key, entry);
      return entry.value;
    }

    set(key, value) {
      // Remove oldest if at capacity
      if (this.cache.size >= this.maxSize) {
        const firstKey = this.cache.keys().next().value;
        this.cache.delete(firstKey);
      }

      this.cache.set(key, { value, timestamp: Date.now() });
    }

    has(key) {
      return this.get(key) !== null;
    }

    clear() {
      this.cache.clear();
    }
  }

  // sessionStorage adapter for persistence across page reloads
  class SessionStorageCache {
    constructor(prefix) {
      this.prefix = prefix;
    }

    _getKey(key) {
      return this.prefix + key;
    }

    get(key) {
      try {
        const raw = sessionStorage.getItem(this._getKey(key));
        if (!raw) return null;

        const entry = JSON.parse(raw);
        if (Date.now() - entry.timestamp > CACHE_EXPIRY_MS) {
          sessionStorage.removeItem(this._getKey(key));
          return null;
        }

        return entry.value;
      } catch {
        return null;
      }
    }

    set(key, value) {
      try {
        const entry = { value, timestamp: Date.now() };
        sessionStorage.setItem(this._getKey(key), JSON.stringify(entry));
      } catch {
        // sessionStorage might be full or disabled
      }
    }

    has(key) {
      return this.get(key) !== null;
    }
  }

  // Combined cache with memory-first strategy
  class TranslationCache {
    constructor() {
      this.memory = new MemoryCache(MAX_CACHE_SIZE);
      this.session = new SessionStorageCache(CACHE_PREFIX);
    }

    // Generate cache key from translation parameters
    _makeKey(text, from, to, engine) {
      // Normalize text for cache key
      const normalized = text.trim().toLowerCase().replace(/\s+/g, " ");
      return `${engine || "any"}:${from || "auto"}:${to || "zh"}:${normalized}`;
    }

    // Get cached translation
    get(text, from, to, engine) {
      const key = this._makeKey(text, from, to, engine);

      // Try memory first (fastest)
      let result = this.memory.get(key);
      if (result !== null) return result;

      // Try sessionStorage (survives page reload)
      result = this.session.get(key);
      if (result !== null) {
        // Backfill memory cache
        this.memory.set(key, result);
        return result;
      }

      return null;
    }

    // P0 fix (audit B7 gap): the page translator calls getSync for the fast
    // synchronous layers (L1+L2). Add the alias so it does not throw
    // TypeError; stays in sync with `get` (no IndexedDB tier in HEAD).
    getSync(text, from, to, engine) {
      return this.get(text, from, to, engine);
    }

    // Store translation result
    set(text, from, to, engine, result) {
      const key = this._makeKey(text, from, to, engine);
      this.memory.set(key, result);
      this.session.set(key, result);
    }

    // P0 fix (audit B7 gap): synchronous write alias used by the page
    // translator after each batch/failure-path translation.
    setSync(text, from, to, engine, result) {
      this.set(text, from, to, engine, result);
    }

    // Check if translation exists in cache
    has(text, from, to, engine) {
      const key = this._makeKey(text, from, to, engine);
      return this.memory.has(key) || this.session.has(key);
    }

    // Batch get multiple translations
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
    }

    // Batch set multiple translations
    batchSet(texts, from, to, engine, results) {
      for (let i = 0; i < texts.length; i++) {
        if (results[i] !== undefined) {
          this.set(texts[i], from, to, engine, results[i]);
        }
      }
    }

    // Clear all caches
    clear() {
      this.memory.clear();
      // Clear sessionStorage entries
      const keysToRemove = [];
      for (let i = 0; i < sessionStorage.length; i++) {
        const key = sessionStorage.key(i);
        if (key.startsWith(CACHE_PREFIX)) {
          keysToRemove.push(key);
        }
      }
      keysToRemove.forEach(key => sessionStorage.removeItem(key));
    }
  }

  // Export as global
  window.MoonTranslationCache = new TranslationCache();
})();
