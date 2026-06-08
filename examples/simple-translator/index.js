/**
 * Simple Translator Example Plugin
 *
 * This plugin demonstrates how to build a translation plugin for Moon Translator
 * using the Plugin SDK's sandboxed subprocess mode.
 *
 * It also starts an HTTP server so it can be used as an HTTP-based plugin
 * (the simpler mode that does not require the sandbox).
 *
 * Usage:
 *   1. Copy this directory to %APPDATA%/moontranslator/plugins/simple-translator/
 *   2. Install dependencies: npm install express
 *   3. Start the plugin: node index.js
 *   4. Enable the plugin in Moon Translator settings
 *
 * The plugin translates text by calling a configurable external API,
 * or falls back to a simple pass-through for testing.
 */

const express = require("express");

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

const PORT = process.env.PORT || 3001;
const UPSTREAM_URL = process.env.UPSTREAM_URL || "";
const UPSTREAM_KEY = process.env.UPSTREAM_KEY || "";

// Language code to human-readable name mapping
const LANG_NAMES = {
  zh: "Chinese",
  en: "English",
  ja: "Japanese",
  ko: "Korean",
  fr: "French",
  de: "German",
  es: "Spanish",
  ru: "Russian",
  pt: "Portuguese",
  it: "Italian",
  ar: "Arabic",
  th: "Thai",
  vi: "Vietnamese",
};

// ---------------------------------------------------------------------------
// Translation Logic
// ---------------------------------------------------------------------------

/**
 * Translate text using an upstream API.
 * If no upstream is configured, returns a mock response for testing.
 */
async function translate(text, from, to) {
  // If an upstream API is configured, forward the request
  if (UPSTREAM_URL) {
    return await translateUpstream(text, from, to);
  }

  // Otherwise, return a mock response for testing
  return mockTranslate(text, from, to);
}

async function translateUpstream(text, from, to) {
  const headers = {
    "Content-Type": "application/json",
  };
  if (UPSTREAM_KEY) {
    headers["Authorization"] = `Bearer ${UPSTREAM_KEY}`;
  }

  const resp = await fetch(UPSTREAM_URL, {
    method: "POST",
    headers,
    body: JSON.stringify({ text, from, to }),
  });

  if (!resp.ok) {
    const body = await resp.text();
    throw new Error(`Upstream API error ${resp.status}: ${body}`);
  }

  const data = await resp.json();

  // Support common response formats
  if (data.translated) return data.translated;
  if (data.translation) return data.translation;
  if (data.result) return data.result;
  if (data.text) return data.text;

  throw new Error("Upstream API returned unexpected format");
}

function mockTranslate(text, from, to) {
  const fromName = LANG_NAMES[from] || from;
  const toName = LANG_NAMES[to] || to;
  return `[${fromName} -> ${toName}] ${text}`;
}

// ---------------------------------------------------------------------------
// HTTP Server (for HTTP-based plugin mode)
// ---------------------------------------------------------------------------

const app = express();
app.use(express.json({ limit: "5mb" }));

// Health check
app.get("/health", (_req, res) => {
  res.json({ status: "ok", name: "Simple Translator Example", version: "1.0.0" });
});

// Translation endpoint
app.post("/translate", async (req, res) => {
  const { text, from, to } = req.body;

  if (!text || !to) {
    return res.status(400).json({ error: "Missing required fields: text, to" });
  }

  try {
    const translated = await translate(text, from || "auto", to);
    res.json({ translated });
  } catch (err) {
    console.error("[SimpleTranslator] Translation error:", err.message);
    res.status(500).json({ error: err.message });
  }
});

// Batch translation endpoint
app.post("/translate/batch", async (req, res) => {
  const { texts, from, to } = req.body;

  if (!texts || !Array.isArray(texts) || !to) {
    return res.status(400).json({ error: "Missing required fields: texts (array), to" });
  }

  try {
    const translations = [];
    for (const text of texts) {
      const translated = await translate(text, from || "auto", to);
      translations.push(translated);
    }
    res.json({ translations });
  } catch (err) {
    console.error("[SimpleTranslator] Batch error:", err.message);
    res.status(500).json({ error: err.message });
  }
});

// Start server
app.listen(PORT, () => {
  console.log(`[SimpleTranslator] HTTP server running on port ${PORT}`);
  console.log(`[SimpleTranslator] Upstream: ${UPSTREAM_URL || "(mock mode)"}`);
  console.log(`[SimpleTranslator] POST /translate  { text, from, to }`);
  console.log(`[SimpleTranslator] POST /translate/batch  { texts[], from, to }`);
});
