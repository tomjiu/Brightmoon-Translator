import { describe, expect, it } from "vitest";
import { calculateOcrResultOverlayRect } from "./ocrOverlayPosition";

describe("calculateOcrResultOverlayRect", () => {
  it("places the result overlay to the right of the selected OCR region when space allows", () => {
    const rect = calculateOcrResultOverlayRect(
      { left: 100, top: 80, width: 220, height: 90 },
      {
        screenX: 0,
        screenY: 0,
        screenWidth: 1280,
        screenHeight: 720,
        scaleFactor: 1,
        imageWidth: 1280,
        imageHeight: 720,
      }
    );

    expect(rect).toEqual({ x: 330, y: 80, width: 360, height: 180 });
  });

  it("places the result overlay to the left when the right side would overflow", () => {
    const rect = calculateOcrResultOverlayRect(
      { left: 1080, top: 120, width: 160, height: 80 },
      {
        screenX: 0,
        screenY: 0,
        screenWidth: 1280,
        screenHeight: 720,
        scaleFactor: 1,
        imageWidth: 1280,
        imageHeight: 720,
      }
    );

    expect(rect).toEqual({ x: 710, y: 120, width: 360, height: 180 });
  });
});
