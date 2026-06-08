import { describe, expect, it } from "vitest";
import type { TranslateResponse } from "./index";

describe("TranslateResponse", () => {
  it("uses the camelCase detectedLanguage field produced by the desktop API", () => {
    const response: TranslateResponse = {
      results: [{ engine: "test", text: "你好" }],
      detectedLanguage: "en",
    };

    expect(response.detectedLanguage).toBe("en");
  });
});
