/** Shared OCR geometry — single source of truth (do not redefine per feature). */
export interface OcrRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}
