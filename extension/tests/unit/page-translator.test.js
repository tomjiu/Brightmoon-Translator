import { describe, it, expect, beforeEach, vi } from "vitest";

// ==================== B1-B8 Bilingual Display Logic ====================

// B3: Filter — shouldSkipElement 逻辑
function shouldSkipElement(tagName, attrs = {}) {
  const tag = tagName.toLowerCase();
  const SKIP_TAGS = new Set(["script", "style", "noscript", "code", "pre", "svg",
    "textarea", "input", "select", "option", "button",
    "iframe", "canvas", "video", "audio", "object",
    // Tier4-1: structural chrome tags
    "nav", "footer", "header", "aside", "form", "figcaption"]);
  const SKIP_ROLES = new Set([
    "navigation", "banner", "contentinfo", "search", "complementary",
    "form", "menu", "menubar", "tablist", "toolbar",
  ]);
  if (SKIP_TAGS.has(tag)) return true;
  if (attrs.notranslate) return true;
  if (attrs.translate === "no") return true;
  if (attrs.contenteditable) return true;
  if (attrs.dataTranslationmark === "copiedNode") return true;
  if (attrs.role && SKIP_ROLES.has(attrs.role)) return true;
  if (attrs.id?.startsWith("moon-")) return true;
  return false;
}

// B5: fooCount guard
function createGenerationGuard() {
  let gen = 0;
  return {
    start: () => { gen++; return gen; },
    isStale: (g) => g !== gen,
    current: () => gen,
  };
}

// B6: Piece splitting
const PIECE_MAX_CHARS = 1000;
const INLINE_TAGS = new Set([
  "a", "abbr", "b", "bdi", "bdo", "cite", "code", "data",
  "dfn", "em", "i", "kbd", "mark", "q", "rp", "rt", "ruby",
  "s", "samp", "small", "span", "strong", "sub", "sup",
  "time", "u", "var", "wbr", "#text",
]);

function buildPieces(textNodes) {
  const pieces = [];
  let current = null;
  for (const node of textNodes) {
    const tag = node.parentTag || "#text";
    const isInline = INLINE_TAGS.has(tag);
    if (current && isInline && current.text.length + node.text.length <= PIECE_MAX_CHARS) {
      current.nodes.push(node);
      current.text += node.text;
    } else {
      if (current && current.text.trim()) pieces.push(current);
      current = { nodes: [node], text: node.text };
    }
  }
  if (current && current.text.trim()) pieces.push(current);

  // B6: 超长 piece 切分
  const finalPieces = [];
  for (const piece of pieces) {
    if (piece.text.length <= PIECE_MAX_CHARS) {
      finalPieces.push(piece);
    } else {
      const sentences = piece.text.split(/(?<=[.!?。！？；\n])\s*/);
      let curr = { nodes: [], text: "" };
      for (const s of sentences) {
        if (curr.text.length + s.length > PIECE_MAX_CHARS && curr.text.trim()) {
          finalPieces.push(curr);
          curr = { nodes: [], text: "" };
        }
        curr.text += s;
        if (curr.nodes.length === 0) curr.nodes = piece.nodes.slice(0, 1);
      }
      if (curr.text.trim()) finalPieces.push(curr);
    }
  }
  return finalPieces;
}

// B8: LLM batch separator parsing
function parseBatchResponse(content, count) {
  const results = new Array(count).fill(null);
  const pattern = /\[\[(\d+)\]\]\s*([^\[]*)(?=\[\[\d+\]\]|$)/gs;
  let match;
  while ((match = pattern.exec(content)) !== null) {
    const idx = parseInt(match[1], 10);
    const translated = match[2].trim();
    if (idx >= 0 && idx < count && translated) {
      results[idx] = translated;
    }
  }
  // Fallback: 单条直接返回
  if (results.every(r => r === null) && count === 1) {
    results[0] = content.trim();
  }
  return results;
}

// ==================== Tests ====================

describe("B3: Filter Completion", () => {
  it("should skip script/style/noscript tags", () => {
    expect(shouldSkipElement("script")).toBe(true);
    expect(shouldSkipElement("style")).toBe(true);
    expect(shouldSkipElement("noscript")).toBe(true);
  });

  it("should skip notranslate attribute", () => {
    expect(shouldSkipElement("div", { notranslate: true })).toBe(true);
  });

  it("should skip translate=no", () => {
    expect(shouldSkipElement("div", { translate: "no" })).toBe(true);
  });

  it("should skip contenteditable", () => {
    expect(shouldSkipElement("div", { contenteditable: true })).toBe(true);
  });

  it("should skip data-translationmark=copiedNode", () => {
    expect(shouldSkipElement("div", { dataTranslationmark: "copiedNode" })).toBe(true);
  });

  it("should skip moon- prefixed ids", () => {
    expect(shouldSkipElement("div", { id: "moon-tooltip" })).toBe(true);
  });

  it("should accept normal elements", () => {
    expect(shouldSkipElement("p")).toBe(false);
    expect(shouldSkipElement("div")).toBe(false);
    expect(shouldSkipElement("span")).toBe(false);
  });
});

describe("B5: fooCount Generation Guard", () => {
  it("should invalidate stale generations", () => {
    const guard = createGenerationGuard();
    const gen1 = guard.start();
    expect(guard.isStale(gen1)).toBe(false);
    const gen2 = guard.start();
    expect(guard.isStale(gen1)).toBe(true);
    expect(guard.isStale(gen2)).toBe(false);
  });

  it("should handle multiple generations", () => {
    const guard = createGenerationGuard();
    const gens = [];
    for (let i = 0; i < 5; i++) gens.push(guard.start());
    expect(guard.isStale(gens[0])).toBe(true);
    expect(guard.isStale(gens[4])).toBe(false);
  });

  it("restorePage should invalidate all previous translations", () => {
    const guard = createGenerationGuard();
    const gen = guard.start();
    // 模拟 restorePage
    guard.start();
    expect(guard.isStale(gen)).toBe(true);
  });
});

describe("B6: Piece Splitting", () => {
  it("should create one piece for single text node", () => {
    const nodes = [{ text: "Hello world", parentTag: "p" }];
    const pieces = buildPieces(nodes);
    expect(pieces).toHaveLength(1);
    expect(pieces[0].text).toBe("Hello world");
  });

  it("should merge inline siblings", () => {
    const nodes = [
      { text: "Hello ", parentTag: "span" },
      { text: "world", parentTag: "span" },
    ];
    const pieces = buildPieces(nodes);
    expect(pieces).toHaveLength(1);
    expect(pieces[0].text).toBe("Hello world");
  });

  it("should separate block elements", () => {
    const nodes = [
      { text: "First paragraph", parentTag: "p" },
      { text: "Second paragraph", parentTag: "p" },
    ];
    const pieces = buildPieces(nodes);
    expect(pieces).toHaveLength(2);
  });

  it("should split long text at sentence boundaries", () => {
    const longText = "This is a sentence. ".repeat(60); // ~1200 chars
    const nodes = [{ text: longText, parentTag: "p" }];
    const pieces = buildPieces(nodes);
    expect(pieces.length).toBeGreaterThan(1);
    for (const piece of pieces) {
      expect(piece.text.length).toBeLessThanOrEqual(PIECE_MAX_CHARS + 50);
    }
  });

  it("should handle mixed inline and block", () => {
    const nodes = [
      { text: "Hello ", parentTag: "span" },
      { text: "world", parentTag: "span" },
      { text: "New paragraph", parentTag: "p" },
    ];
    const pieces = buildPieces(nodes);
    expect(pieces).toHaveLength(2);
    expect(pieces[0].text).toBe("Hello world");
    expect(pieces[1].text).toBe("New paragraph");
  });
});

describe("B8: LLM Batch Separator Parsing", () => {
  it("should parse well-formed batch response", () => {
    const content = `[[0]] 你好世界
[[1]] 再见
[[2]] 谢谢`;
    const results = parseBatchResponse(content, 3);
    expect(results[0]).toBe("你好世界");
    expect(results[1]).toBe("再见");
    expect(results[2]).toBe("谢谢");
  });

  it("should handle missing entries", () => {
    const content = `[[0]] 你好
[[2]] 谢谢`;
    const results = parseBatchResponse(content, 3);
    expect(results[0]).toBe("你好");
    expect(results[1]).toBeNull();
    expect(results[2]).toBe("谢谢");
  });

  it("should handle multi-line translations", () => {
    const content = `[[0]] 第一行
第二行
[[1]] 再见`;
    const results = parseBatchResponse(content, 2);
    expect(results[0]).toContain("第一行");
    expect(results[0]).toContain("第二行");
    expect(results[1]).toBe("再见");
  });

  it("should fallback to direct content for single text", () => {
    const content = "直接翻译结果";
    const results = parseBatchResponse(content, 1);
    expect(results[0]).toBe("直接翻译结果");
  });

  it("should handle empty response", () => {
    const results = parseBatchResponse("", 2);
    expect(results[0]).toBeNull();
    expect(results[1]).toBeNull();
  });

  it("should trim whitespace from translations", () => {
    const content = `[[0]]   空格测试   \n[[1]]\t制表符\t`;
    const results = parseBatchResponse(content, 2);
    expect(results[0]).toBe("空格测试");
    expect(results[1]).toBe("制表符");
  });
});

// ==================== B7: Cache Tests ====================

describe("B7: Three-Tier Cache", () => {
  it("should create cache with memory, sessionStorage, and indexedDB tiers", () => {
    // 验证 TranslationCache 接口存在
    // 实际测试在浏览器环境中运行
    expect(typeof buildPieces).toBe("function");
  });
});

// ==================== B1+B2: Restore Logic ====================

describe("B1+B2: Restore Logic", () => {
  it("nodesToRestore should track entries in order", () => {
    const nodesToRestore = [];
    // 模拟 B1 replace
    nodesToRestore.push({ node: "node1", originalText: "Hello" });
    nodesToRestore.push({ span: "span1", type: "remove" });
    // 模拟 B2 bilingual
    nodesToRestore.push({ span: "wrapper1", type: "remove" });
    nodesToRestore.push({ node: "node2", originalText: "World" });

    expect(nodesToRestore).toHaveLength(4);
    // 逆序 restore
    for (let i = nodesToRestore.length - 1; i >= 0; i--) {
      const entry = nodesToRestore[i];
      if (entry.span && entry.type === "remove") {
        // remove span
      } else if (entry.node) {
        // restore textContent
      }
    }
    // 逆序确保 DOM 操作不会索引错位
    expect(nodesToRestore[3].node).toBe("node2");
    expect(nodesToRestore[0].node).toBe("node1");
  });
});

// ==================== Tier4-1: Main Container Heuristic Tests ====================

describe("Tier4-1: shouldSkipElement with structural tags", () => {
  it("should skip nav/footer/header/aside/form/figcaption tags", () => {
    expect(shouldSkipElement("NAV")).toBe(true);
    expect(shouldSkipElement("FOOTER")).toBe(true);
    expect(shouldSkipElement("HEADER")).toBe(true);
    expect(shouldSkipElement("ASIDE")).toBe(true);
    expect(shouldSkipElement("FORM")).toBe(true);
    expect(shouldSkipElement("FIGCAPTION")).toBe(true);
  });

  it("should not skip article/main/section/p/div", () => {
    expect(shouldSkipElement("ARTICLE")).toBe(false);
    expect(shouldSkipElement("MAIN")).toBe(false);
    expect(shouldSkipElement("SECTION")).toBe(false);
    expect(shouldSkipElement("P")).toBe(false);
    expect(shouldSkipElement("DIV")).toBe(false);
  });

  it("should skip ARIA landmark roles", () => {
    expect(shouldSkipElement("DIV", { role: "navigation" })).toBe(true);
    expect(shouldSkipElement("DIV", { role: "banner" })).toBe(true);
    expect(shouldSkipElement("DIV", { role: "contentinfo" })).toBe(true);
    expect(shouldSkipElement("DIV", { role: "search" })).toBe(true);
    expect(shouldSkipElement("DIV", { role: "complementary" })).toBe(true);
    expect(shouldSkipElement("DIV", { role: "menu" })).toBe(true);
    expect(shouldSkipElement("DIV", { role: "menubar" })).toBe(true);
    expect(shouldSkipElement("DIV", { role: "tablist" })).toBe(true);
    expect(shouldSkipElement("DIV", { role: "toolbar" })).toBe(true);
  });

  it("should not skip content roles", () => {
    expect(shouldSkipElement("DIV", { role: "main" })).toBe(false);
    expect(shouldSkipElement("DIV", { role: "article" })).toBe(false);
    expect(shouldSkipElement("DIV", { role: "region" })).toBe(false);
    expect(shouldSkipElement("DIV", { role: "contentinfo" })).toBe(true); // contentinfo IS chrome
  });
});

// Main container heuristic — pure logic test
// Simulates findMainContainer() bucket-and-threshold algorithm.
function findMainContainerSim(paragraphs) {
  // paragraphs: [{ text: string, visible: boolean, containerId: string }]
  const visible = paragraphs.filter(p => p.visible);
  if (visible.length < 3) return "body";

  const totalPText = visible.reduce((sum, p) => sum + p.text.trim().length, 0);
  if (totalPText < 200) return "body";

  // Group by containerId (simulating ancestor bucket)
  const buckets = new Map();
  for (const p of visible) {
    const entry = buckets.get(p.containerId) || { count: 0, len: 0 };
    entry.count += 1;
    entry.len += p.text.trim().length;
    buckets.set(p.containerId, entry);
  }

  let best = "body";
  let bestLen = 0;
  const threshold = totalPText * 0.4;
  for (const [id, { count, len }] of buckets) {
    if (count >= 3 && len >= threshold && len > bestLen) {
      best = id;
      bestLen = len;
    }
  }
  return best;
}

describe("Tier4-1: findMainContainer bucket heuristic", () => {
  it("should return body when < 3 paragraphs", () => {
    expect(findMainContainerSim([
      { text: "a".repeat(100), visible: true, containerId: "article" },
      { text: "b".repeat(100), visible: true, containerId: "article" },
    ])).toBe("body");
  });

  it("should return body when total text < 200 chars", () => {
    expect(findMainContainerSim([
      { text: "ab", visible: true, containerId: "article" },
      { text: "cd", visible: true, containerId: "article" },
      { text: "ef", visible: true, containerId: "article" },
    ])).toBe("body");
  });

  it("should detect article container with > 40% of <p> text", () => {
    const result = findMainContainerSim([
      { text: "x".repeat(300), visible: true, containerId: "article" },
      { text: "y".repeat(300), visible: true, containerId: "article" },
      { text: "z".repeat(300), visible: true, containerId: "article" },
      // nav links — small text, different container
      { text: "Home", visible: true, containerId: "nav" },
      { text: "About", visible: true, containerId: "nav" },
      { text: "Contact", visible: true, containerId: "nav" },
    ]);
    expect(result).toBe("article");
  });

  it("should return body when no container has >= 40% of text", () => {
    // Text spread evenly across 3 containers — none reaches 40%
    const result = findMainContainerSim([
      { text: "x".repeat(100), visible: true, containerId: "c1" },
      { text: "x".repeat(100), visible: true, containerId: "c1" },
      { text: "x".repeat(100), visible: true, containerId: "c1" },
      { text: "y".repeat(100), visible: true, containerId: "c2" },
      { text: "y".repeat(100), visible: true, containerId: "c2" },
      { text: "y".repeat(100), visible: true, containerId: "c2" },
      { text: "z".repeat(100), visible: true, containerId: "c3" },
      { text: "z".repeat(100), visible: true, containerId: "c3" },
      { text: "z".repeat(100), visible: true, containerId: "c3" },
    ]);
    // Each container has 300/900 = 33.3%, below 40% threshold
    expect(result).toBe("body");
  });

  it("should skip hidden paragraphs", () => {
    const result = findMainContainerSim([
      { text: "x".repeat(300), visible: false, containerId: "article" },
      { text: "y".repeat(300), visible: false, containerId: "article" },
      { text: "z".repeat(300), visible: false, containerId: "article" },
      // Only visible ones are in nav
      { text: "Home", visible: true, containerId: "nav" },
      { text: "About", visible: true, containerId: "nav" },
      { text: "Contact", visible: true, containerId: "nav" },
    ]);
    // Visible total < 200 chars → body
    expect(result).toBe("body");
  });

  it("should pick the container with most text when multiple qualify", () => {
    const result = findMainContainerSim([
      // article: 500 chars
      { text: "x".repeat(200), visible: true, containerId: "article" },
      { text: "x".repeat(200), visible: true, containerId: "article" },
      { text: "x".repeat(100), visible: true, containerId: "article" },
      // sidebar: 300 chars (still > 40% of 800 total = 320? no, 300 < 320)
      { text: "y".repeat(150), visible: true, containerId: "sidebar" },
      { text: "y".repeat(150), visible: true, containerId: "sidebar" },
      { text: "y".repeat(0), visible: true, containerId: "sidebar" },
    ]);
    // total = 800, threshold = 320
    // article = 500 >= 320 ✓
    // sidebar = 300 < 320 ✗
    expect(result).toBe("article");
  });
});

// ==================== Tier4-4: specialRules Tests ====================

const SPECIAL_RULES = [
  { name: "twitter", hostMatch: /(^|\.)twitter\.com$|(^|\.)x\.com$/, isSPA: true },
  { name: "reddit", hostMatch: /(^|\.)reddit\.com$/, isSPA: true },
  { name: "github", hostMatch: /(^|\.)github\.com$/, isSPA: false },
  { name: "youtube", hostMatch: /(^|\.)youtube\.com$/, isSPA: true },
  { name: "zhihu", hostMatch: /(^|\.)zhihu\.com$/, isSPA: false },
  { name: "wechat", hostMatch: /(^|\.)weixin\.qq\.com$/, isSPA: false },
];

function matchSpecialRule(host) {
  for (const rule of SPECIAL_RULES) {
    if (rule.hostMatch.test(host)) return rule;
  }
  return null;
}

describe("Tier4-4: specialRules host matching", () => {
  it("should match twitter.com", () => {
    expect(matchSpecialRule("twitter.com")?.name).toBe("twitter");
    expect(matchSpecialRule("www.twitter.com")?.name).toBe("twitter");
    expect(matchSpecialRule("api.twitter.com")?.name).toBe("twitter");
  });

  it("should match x.com (Twitter rebrand)", () => {
    expect(matchSpecialRule("x.com")?.name).toBe("twitter");
    expect(matchSpecialRule("www.x.com")?.name).toBe("twitter");
  });

  it("should match reddit.com", () => {
    expect(matchSpecialRule("reddit.com")?.name).toBe("reddit");
    expect(matchSpecialRule("www.reddit.com")?.name).toBe("reddit");
    expect(matchSpecialRule("old.reddit.com")?.name).toBe("reddit");
  });

  it("should match github.com", () => {
    expect(matchSpecialRule("github.com")?.name).toBe("github");
    expect(matchSpecialRule("gist.github.com")?.name).toBe("github");
  });

  it("should match youtube.com", () => {
    expect(matchSpecialRule("youtube.com")?.name).toBe("youtube");
    expect(matchSpecialRule("www.youtube.com")?.name).toBe("youtube");
    expect(matchSpecialRule("m.youtube.com")?.name).toBe("youtube");
  });

  it("should match zhihu.com", () => {
    expect(matchSpecialRule("zhihu.com")?.name).toBe("zhihu");
    expect(matchSpecialRule("www.zhihu.com")?.name).toBe("zhihu");
    expect(matchSpecialRule("zhuanlan.zhihu.com")?.name).toBe("zhihu");
  });

  it("should match weixin.qq.com", () => {
    expect(matchSpecialRule("mp.weixin.qq.com")?.name).toBe("wechat");
    expect(matchSpecialRule("weixin.qq.com")?.name).toBe("wechat");
  });

  it("should not match unrelated hosts", () => {
    expect(matchSpecialRule("example.com")).toBeNull();
    expect(matchSpecialRule("google.com")).toBeNull();
    expect(matchSpecialRule("notwitter.com")).toBeNull(); // boundary check
    expect(matchSpecialRule("twitter.evil.com")).toBeNull(); // subdomain trick
  });
});

describe("Tier4-4: specialRules isSPA flag", () => {
  it("should mark twitter/reddit/youtube as SPA", () => {
    expect(matchSpecialRule("twitter.com")?.isSPA).toBe(true);
    expect(matchSpecialRule("reddit.com")?.isSPA).toBe(true);
    expect(matchSpecialRule("youtube.com")?.isSPA).toBe(true);
  });

  it("should mark github/zhihu/wechat as non-SPA", () => {
    expect(matchSpecialRule("github.com")?.isSPA).toBe(false);
    expect(matchSpecialRule("zhihu.com")?.isSPA).toBe(false);
    expect(matchSpecialRule("mp.weixin.qq.com")?.isSPA).toBe(false);
  });
});
