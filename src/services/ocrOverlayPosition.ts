import type { ScreenshotRegion, ScreenshotSnapshotInfo } from "./ocr";

export interface OverlayRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function calculateOcrResultOverlayRect(
  region: ScreenshotRegion,
  snapshotInfo: ScreenshotSnapshotInfo | null,
  minWidth = 360,
  minHeight = 180
): OverlayRect {
  const scaleX = snapshotInfo ? snapshotInfo.imageWidth / snapshotInfo.screenWidth : 1;
  const scaleY = snapshotInfo ? snapshotInfo.imageHeight / snapshotInfo.screenHeight : 1;
  const screenX = snapshotInfo ? snapshotInfo.screenX * scaleX : 0;
  const screenY = snapshotInfo ? snapshotInfo.screenY * scaleY : 0;
  const imageWidth = snapshotInfo?.imageWidth ?? region.left + region.width + minWidth;

  const width = Math.max(minWidth, Math.min(520, Math.round(region.width)));
  const height = Math.max(minHeight, Math.min(360, Math.round(region.height * 1.2)));
  const gap = 10;
  const rightSideX = screenX + region.left + region.width + gap;
  const leftSideX = screenX + Math.max(0, region.left - width - gap);
  const fitsRight = rightSideX + width <= screenX + imageWidth;

  return {
    x: Math.round(fitsRight ? rightSideX : leftSideX),
    y: Math.round(screenY + region.top),
    width,
    height,
  };
}
