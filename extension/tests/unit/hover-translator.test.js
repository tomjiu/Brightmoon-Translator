import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";

// ==================== Hover Translator Logic ====================
// Tests for the hover translator content script logic

// Skip tags that should not trigger hover translation
const SKIP_TAGS = new Set([
  "INPUT",
  "TEXTAREA",
  "BUTTON",
  "SELECT",
  "A",
  "SCRIPT",
  "STYLE",
  "CODE",
  "PRE",
  "SVG",
  "CANVAS",
  "VIDEO",
  "AUDIO",
  "IFRAME",
  "EMBED",
  "OBJECT",
]);

// Block tags for text grouping
const BLOCK_TAGS = new Set([
  "P",
  "LI",
  "TD",
  "TH",
  "DT",
  "DD",
  "BLOCKQUOTE",
  "DIV",
  "ARTICLE",
  "SECTION",
  "H1",
  "H2",
  "H3",
  "H4",
  "H5",
  "H6",
]);

function isInteractiveElement(tagName, attributes = {}) {
  if (SKIP_TAGS.has(tagName)) return true;
  if (attributes.contentEditable) return true;
  if (attributes.role === "textbox" || attributes.role === "button") return true;
  if (attributes.tabindex !== undefined && tagName !== "BODY") return true;
  return false;
}

function getHoverTextTarget(tagName, attributes = {}, textLength, minTextLength = 2) {
  if (isInteractiveElement(tagName, attributes)) {
    return { text: "", element: null };
  }

  if (textLength >= minTextLength && textLength <= 2000) {
    return { text: "sample text", element: tagName };
  }

  return { text: "", element: null };
}

// Modifier key matching logic
function isModifierMatch(e, modifierKey) {
  switch (modifierKey) {
    case "alt":
      return e.altKey;
    case "ctrl":
      return e.ctrlKey || e.metaKey;
    case "shift":
      return e.shiftKey;
    default:
      return true; // "none" — always triggers
  }
}

// Hover delay validation
function validateHoverDelay(delay) {
  return Math.max(100, Math.min(2000, delay || 300));
}

// Min text length validation
function validateMinTextLength(length) {
  return Math.max(1, Math.min(100, length || 2));
}

// ==================== Tests ====================

describe("Hover Translator - Skip Tags", () => {
  it("should skip INPUT elements", () => {
    expect(isInteractiveElement("INPUT")).toBe(true);
  });

  it("should skip TEXTAREA elements", () => {
    expect(isInteractiveElement("TEXTAREA")).toBe(true);
  });

  it("should skip BUTTON elements", () => {
    expect(isInteractiveElement("BUTTON")).toBe(true);
  });

  it("should skip SELECT elements", () => {
    expect(isInteractiveElement("SELECT")).toBe(true);
  });

  it("should skip A (anchor) elements", () => {
    expect(isInteractiveElement("A")).toBe(true);
  });

  it("should skip SCRIPT elements", () => {
    expect(isInteractiveElement("SCRIPT")).toBe(true);
  });

  it("should skip STYLE elements", () => {
    expect(isInteractiveElement("STYLE")).toBe(true);
  });

  it("should skip CODE elements", () => {
    expect(isInteractiveElement("CODE")).toBe(true);
  });

  it("should skip PRE elements", () => {
    expect(isInteractiveElement("PRE")).toBe(true);
  });

  it("should skip SVG elements", () => {
    expect(isInteractiveElement("SVG")).toBe(true);
  });

  it("should skip contentEditable elements", () => {
    expect(isInteractiveElement("DIV", { contentEditable: true })).toBe(true);
  });

  it("should skip elements with textbox role", () => {
    expect(isInteractiveElement("SPAN", { role: "textbox" })).toBe(true);
  });

  it("should skip elements with button role", () => {
    expect(isInteractiveElement("DIV", { role: "button" })).toBe(true);
  });

  it("should skip elements with tabindex", () => {
    expect(isInteractiveElement("DIV", { tabindex: 0 })).toBe(true);
  });

  it("should not skip BODY with tabindex", () => {
    expect(isInteractiveElement("BODY", { tabindex: 0 })).toBe(false);
  });

  it("should not skip regular DIV", () => {
    expect(isInteractiveElement("DIV")).toBe(false);
  });

  it("should not skip P elements", () => {
    expect(isInteractiveElement("P")).toBe(false);
  });

  it("should not skip SPAN elements", () => {
    expect(isInteractiveElement("SPAN")).toBe(false);
  });
});

describe("Hover Translator - Text Target Selection", () => {
  it("should return empty for interactive elements", () => {
    const result = getHoverTextTarget("INPUT", {}, 100);
    expect(result.text).toBe("");
    expect(result.element).toBeNull();
  });

  it("should return text for valid elements", () => {
    const result = getHoverTextTarget("P", {}, 100, 2);
    expect(result.text).toBe("sample text");
  });

  it("should reject text shorter than minimum", () => {
    const result = getHoverTextTarget("P", {}, 1, 2);
    expect(result.text).toBe("");
  });

  it("should accept text at minimum length", () => {
    const result = getHoverTextTarget("P", {}, 2, 2);
    expect(result.text).toBe("sample text");
  });

  it("should reject text longer than maximum (2000)", () => {
    const result = getHoverTextTarget("P", {}, 2001, 2);
    expect(result.text).toBe("");
  });
});

describe("Hover Translator - Modifier Key", () => {
  it("should always match when modifier is 'none'", () => {
    const event = { altKey: false, ctrlKey: false, metaKey: false, shiftKey: false };
    expect(isModifierMatch(event, "none")).toBe(true);
  });

  it("should match when alt is pressed and modifier is 'alt'", () => {
    const event = { altKey: true, ctrlKey: false, metaKey: false, shiftKey: false };
    expect(isModifierMatch(event, "alt")).toBe(true);
  });

  it("should not match when alt is not pressed and modifier is 'alt'", () => {
    const event = { altKey: false, ctrlKey: false, metaKey: false, shiftKey: false };
    expect(isModifierMatch(event, "alt")).toBe(false);
  });

  it("should match when ctrl is pressed and modifier is 'ctrl'", () => {
    const event = { altKey: false, ctrlKey: true, metaKey: false, shiftKey: false };
    expect(isModifierMatch(event, "ctrl")).toBe(true);
  });

  it("should match when meta is pressed and modifier is 'ctrl'", () => {
    const event = { altKey: false, ctrlKey: false, metaKey: true, shiftKey: false };
    expect(isModifierMatch(event, "ctrl")).toBe(true);
  });

  it("should match when shift is pressed and modifier is 'shift'", () => {
    const event = { altKey: false, ctrlKey: false, metaKey: false, shiftKey: true };
    expect(isModifierMatch(event, "shift")).toBe(true);
  });

  it("should not match when shift is not pressed and modifier is 'shift'", () => {
    const event = { altKey: false, ctrlKey: false, metaKey: false, shiftKey: false };
    expect(isModifierMatch(event, "shift")).toBe(false);
  });
});

describe("Hover Translator - Delay Validation", () => {
  it("should use default delay of 300", () => {
    expect(validateHoverDelay(undefined)).toBe(300);
  });

  it("should use default delay of 300 for null", () => {
    expect(validateHoverDelay(null)).toBe(300);
  });

  it("should clamp delay to minimum of 100", () => {
    expect(validateHoverDelay(50)).toBe(100);
  });

  it("should clamp delay to maximum of 2000", () => {
    expect(validateHoverDelay(3000)).toBe(2000);
  });

  it("should accept valid delay values", () => {
    expect(validateHoverDelay(500)).toBe(500);
  });

  it("should accept minimum delay value", () => {
    expect(validateHoverDelay(100)).toBe(100);
  });

  it("should accept maximum delay value", () => {
    expect(validateHoverDelay(2000)).toBe(2000);
  });
});

describe("Hover Translator - Min Text Length Validation", () => {
  it("should use default of 2", () => {
    expect(validateMinTextLength(undefined)).toBe(2);
  });

  it("should use default of 2 for null", () => {
    expect(validateMinTextLength(null)).toBe(2);
  });

  it("should use default of 2 for 0 (falsy value)", () => {
    // Note: The function uses || operator, so 0 is treated as falsy and gets default value
    expect(validateMinTextLength(0)).toBe(2);
  });

  it("should clamp negative to minimum of 1", () => {
    expect(validateMinTextLength(-1)).toBe(1);
  });

  it("should clamp to maximum of 100", () => {
    expect(validateMinTextLength(200)).toBe(100);
  });

  it("should accept valid values", () => {
    expect(validateMinTextLength(5)).toBe(5);
  });
});

describe("Hover Translator - Block Tags", () => {
  it("should include common block elements", () => {
    expect(BLOCK_TAGS.has("P")).toBe(true);
    expect(BLOCK_TAGS.has("DIV")).toBe(true);
    expect(BLOCK_TAGS.has("LI")).toBe(true);
    expect(BLOCK_TAGS.has("TD")).toBe(true);
    expect(BLOCK_TAGS.has("TH")).toBe(true);
    expect(BLOCK_TAGS.has("BLOCKQUOTE")).toBe(true);
    expect(BLOCK_TAGS.has("ARTICLE")).toBe(true);
    expect(BLOCK_TAGS.has("SECTION")).toBe(true);
  });

  it("should include heading elements", () => {
    expect(BLOCK_TAGS.has("H1")).toBe(true);
    expect(BLOCK_TAGS.has("H2")).toBe(true);
    expect(BLOCK_TAGS.has("H3")).toBe(true);
    expect(BLOCK_TAGS.has("H4")).toBe(true);
    expect(BLOCK_TAGS.has("H5")).toBe(true);
    expect(BLOCK_TAGS.has("H6")).toBe(true);
  });

  it("should not include inline elements", () => {
    expect(BLOCK_TAGS.has("SPAN")).toBe(false);
    expect(BLOCK_TAGS.has("A")).toBe(false);
    expect(BLOCK_TAGS.has("STRONG")).toBe(false);
    expect(BLOCK_TAGS.has("EM")).toBe(false);
  });
});

describe("Hover Translator - Config Loading", () => {
  it("should have correct default hover config", () => {
    const defaultHover = {
      enabled: true,
      delay: 300,
      minTextLength: 2,
      modifierKey: "none",
    };

    expect(defaultHover.enabled).toBe(true);
    expect(defaultHover.delay).toBe(300);
    expect(defaultHover.minTextLength).toBe(2);
    expect(defaultHover.modifierKey).toBe("none");
  });

  it("should handle disabled hover", () => {
    const config = { hover: { enabled: false } };
    const enabled = config.hover?.enabled !== false;
    expect(enabled).toBe(false);
  });

  it("should handle enabled hover", () => {
    const config = { hover: { enabled: true } };
    const enabled = config.hover?.enabled !== false;
    expect(enabled).toBe(true);
  });

  it("should default to enabled when hover config missing", () => {
    const config = {};
    const enabled = config.hover?.enabled !== false;
    expect(enabled).toBe(true);
  });
});
