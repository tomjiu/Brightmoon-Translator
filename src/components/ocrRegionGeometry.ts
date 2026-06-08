export interface RegionRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function frameToCaptureRegion(
  frame: RegionRect,
  toolbarHeightLogical: number,
  scaleFactor: number,
): RegionRect {
  const toolbarPhysical = toolbarHeightLogical * scaleFactor;
  return {
    x: Math.round(frame.x),
    y: Math.round(frame.y + toolbarPhysical),
    width: Math.round(frame.width),
    height: Math.max(1, Math.round(frame.height - toolbarPhysical)),
  };
}

export function captureToFrameRegion(
  capture: RegionRect,
  toolbarHeightLogical: number,
  scaleFactor: number,
): RegionRect {
  const toolbarPhysical = toolbarHeightLogical * scaleFactor;
  return {
    x: Math.round(capture.x),
    y: Math.round(capture.y - toolbarPhysical),
    width: Math.round(capture.width),
    height: Math.round(capture.height + toolbarPhysical),
  };
}
