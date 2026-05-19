import { describe, it, expect } from "vitest";
import { positionBelowText, positionAtWindowBottom } from "./overlayPosition";

describe("positionBelowText", () => {
  it("centers overlay below text element", () => {
    const pos = positionBelowText(100, 200, 300, 50);
    // overlay width = min(500, 300+60) = 360
    // x = 100 + (300 - 360) / 2 = 70
    expect(pos.x).toBe(70);
    expect(pos.y).toBe(258); // 200 + 50 + 8
    expect(pos.width).toBe(360);
    expect(pos.height).toBe(180);
  });

  it("uses custom overlay dimensions", () => {
    const pos = positionBelowText(0, 0, 100, 20, 400, 100);
    // overlay width = min(400, 100+60) = 160
    expect(pos.width).toBe(160);
    expect(pos.height).toBe(100);
    expect(pos.y).toBe(28); // 0 + 20 + 8
  });

  it("clamps overlay width to text width + padding", () => {
    const pos = positionBelowText(0, 0, 600, 30);
    // overlay width = min(500, 600+60) = 500
    expect(pos.width).toBe(500);
  });
});

describe("positionAtWindowBottom", () => {
  it("positions overlay at bottom center of window", () => {
    const pos = positionAtWindowBottom(100, 100, 800, 600);
    // overlay width = min(500, 800-40) = 500
    // x = 100 + (800 - 500) / 2 = 250
    expect(pos.x).toBe(250);
    expect(pos.y).toBe(500); // 100 + 600 - 180 - 20
    expect(pos.width).toBe(500);
    expect(pos.height).toBe(180);
  });

  it("uses custom overlay dimensions", () => {
    const pos = positionAtWindowBottom(0, 0, 1000, 800, 600, 200);
    // overlay width = min(600, 1000-40) = 600
    // x = 0 + (1000 - 600) / 2 = 200
    expect(pos.x).toBe(200);
    expect(pos.y).toBe(580); // 0 + 800 - 200 - 20
    expect(pos.width).toBe(600);
    expect(pos.height).toBe(200);
  });

  it("clamps overlay width to window width - padding", () => {
    const pos = positionAtWindowBottom(0, 0, 300, 400);
    // overlay width = min(500, 300-40) = 260
    expect(pos.width).toBe(260);
  });
});
