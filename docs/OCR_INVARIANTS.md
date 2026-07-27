# OCR Region Frame — Invariants (do not regress)

These rules come from repeated fix/break cycles (see commits `2a95a49`, `4347223`,
`71eb25c`, `cfb5b8d`, and later). **Any OCR change that violates them is a bug.**

## I1 — Continuous refresh defaults OFF

- After screenshot selection, `continuous` must start **false**.
- Before capture, the region frame must **not appear in the grab**:
  prefer `set_ocr_region_frame_sampling(true)` (`WDA_EXCLUDEFROMCAPTURE`);
  fall back to hide/show only if affinity fails.
- Never sample while the frame’s chrome is included in the bitmap
  (feeds OCR its own overlay text → flicker + drift).

Files: `OcrScreenshotTranslator.tsx`, `window.rs` (`set_ocr_region_frame_sampling`)

## I2 — Fixed toolbar height for capture math

- Toolbar CSS height is a **constant** (`TOOLBAR_HEIGHT = 32`).
- Rust `create_ocr_region_frame` / `move_ocr_region_frame` use `32.0 * scale_factor`.
- Do **not** grow the toolbar by wrapping content; that desyncs capture Y.

Files: `OcrRegionFrame.tsx`, `window.rs`, `ocrRegionGeometry.ts`

## I3 — Minimum window width for controls

- Region frame min logical width ≥ **380px** (full toolbar: icons + language selects).
- Root container must not `overflow: hidden` clip the toolbar.
- Resize handle must respect the same minimum.
- Buttons never compress; small selections expand **window chrome only** (capture crop stays true size).
- Action buttons **disabled** when data missing (copy/save need OCR payload; refresh disabled while loading).

Files: `window.rs`, `OcrRegionFrame.tsx`

## I4 — Empty OCR does not destroy the frame

- Empty OCR / translate failure: keep frame open, show in-frame error + retry.
- Do **not** `close_ocr_region_frame` solely because recognition returned empty.

Files: `OcrScreenshotTranslator.tsx`, `OcrRegionFrame.tsx` (`ocr-region-error`)

## I5 — Line layout uses image natural size

- Map OCR boxes with `ocrLineToCssRect(contentCss, imageNatural)`, not raw DPR alone.
- DPR is fallback only when image size is unknown.
- Payload should include `imageWidth`/`imageHeight` so the frame does not wait for `<img onLoad>` (avoids one-frame misaligned flash).

Files: `ocrRegionGeometry.ts`, `OcrScreenshotTranslator` (`probeDataUrlImageSize`), `OcrRegionFrame`

## I6 — Follow tracks window, not OCR chrome

- Bind via `hwnd_from_point` after click-through (`set_ocr_region_frame_click_through`)
  or brief hide — never bind to OCR chrome titles.
- Drag/resize updates offset with `refreshOffset`, not rebinding to foreground
  (which would attach to the OCR window itself).
- Follow poll ≤ **50ms** when enabled.

Files: `ocrWindowBinding.ts`, `OcrScreenshotTranslator.tsx`, `hwnd_from_point`

## I7 — Similarity gate on OCR updates

- When normalized OCR text similarity ≥ 0.92: **skip translate API**; still push geometry/boxes + last translation to the frame.
- Image fingerprint match: skip entire OCR+translate for continuous ticks.
- Prevents 1–2 character jitter from re-hitting engines every tick.

Files: `ocrQuality.ts`, `OcrScreenshotTranslator.tsx`

## Manual smoke (must pass before claiming OCR fixed)

Full 11-step list: [OCR_SMOKE.md](./OCR_SMOKE.md). **Recommended order: 3 → 4 → 1 → 2 → 5–11.**

Condensed gate (same intent):

1. Narrow region → toolbar usable (smoke **#3**).
2. Empty / garbage → frame stays, error + retry (smoke **#4**).
3. Select once → one OCR cycle; no auto-flash (smoke **#1**).
4. Overlay lines aligned (smoke **#2**).
5. Follow + drag/resize (smoke **#8–9**, **#5–6**).

## Deferred: pinned region watch

Product intent (scroll/change → re-OCR+translate) is specified in
`OCR_STRATEGY.md` § Pinned region watch. **Do not expand continuous-mode
features until smoke 1–5 pass.** Skeleton (continuous + fingerprint + I7)
may stay; treat regressions there as bugs, not as a green light for new work.
