import { describe, expect, it } from "vitest";
import { normalizeTranslatorInput } from "./translatorText";

describe("normalizeTranslatorInput", () => {
  it("preserves input when newline deletion is disabled", () => {
    expect(normalizeTranslatorInput("hello\nworld", false)).toBe("hello\nworld");
  });

  it("replaces consecutive newlines and surrounding spaces with one space", () => {
    expect(normalizeTranslatorInput("hello \n\n  world\r\nagain", true)).toBe("hello world again");
  });
});
