# Module map (refactor handoff)

Generated 2026-07-25 from parallel codebase exploration. **Code is truth**; older status docs may lag.

## Priority (unchanged)

1. OCR vertical → 2. Engine façade → 3. Hook verdict → 4. Extension  
Learning / Cloudflare / AppState rewrite: **frozen**.

---

## A. OCR path (screenshot product)

**Not** Rust `overlay/*` or `ocr_engine.rs` (those serve hook/selection). Screenshot OCR paints **inside** `OcrRegionFrame`.

```
MainTranslator / tray
  → OcrScreenshotTranslator (main webview, ~god orchestrator)
  → prepare_screenshot_snapshot → ocr-screenshot selector
  → ocr-screenshot-selected → crop → create_ocr_region_frame
  → ocrWithEngine → invoke(translate) → ocr-region-update-data
  → OcrRegionFrame overlays (ocrLineToCssRect)
```

| Problem | Fix slice first |
|---------|-----------------|
| God orchestrator in `OcrScreenshotTranslator.tsx` | Session state machine |
| Toolbar 32 / min 280 triplicated TS+Rust | Geometry kernel + shared constants |
| Snapshot (image px) vs GDI (screen) dual capture | Memory-first snapshot only |
| ~12 ad-hoc cross-window events + timeouts | Typed session protocol |
| Flicker: hide 120ms every refresh | Capture without self-feed; reduce hide |
| Line/translate split heuristics | After geometry stable |

**Invariants:** `OCR_INVARIANTS.md` I1–I7. Smoke there before “OCR fixed”.

**Isolate order:** Geometry → Snapshot capture → Session → Recognize+map → Frame view → Follow.

---

## B. Engines (chaos sources)

| Symptom | Cause |
|---------|--------|
| Many public APIs | Tauri cmds + HTTP + capabilities + extension local engines |
| Same intent, different quality | replace→primary; OCR/hook→full; UI→stream/embedded; docs→batch |
| Pipeline uneven | ~~batch skip TM/cache~~ **mitigated 2026-07-29** (`translate_batch_core` non-OCR: prepare + TM + cache + finalize); compare multi still multi-result |
| Config drift | Rust default google+youdao ON; TS INITIAL all off; extension own defaults |
| Dead types | `TranslationJob` unused; `OcrTranslation` trait no impl |
| FE store drift | product uses `translateStore`; clipboard/stream stores mostly dead |

**Façade direction:** one `TranslateRequest { channel, mode, text|segments, options }` → `TranslationService` always; presenters/sources separate. Adapters only for selection/OCR/hook/HTTP/extension.

**Do not** unify extension local engines until desktop façade exists.

---

## C. Hook (two systems)

| System | Status |
|--------|--------|
| **Passive monitor** (UIA + clipboard → translate) | Production-ish; default sources OK |
| **DLL inject / H-Code** (`moon_hook.dll`) | Skeleton only |

Inject reality: Win32 LoadLibrary path works in **dev** if DLL found under `hook-dll/build/Release`. **IAT patch almost certainly wrong** → may inject with **zero text**. Messages **not** wired to `TranslationService`. Eject incomplete. Not in Tauri bundle. Profiles CRUD **not** applied on monitor start.

**Verdict:** gate as experimental; **do not invest** until OCR+engines green. Supported story = UIA/clipboard monitor only.

---

## D. Repo modularity

| Hot core | Freeze / demote |
|----------|-----------------|
| MainTranslator, OCR components, engine, overlay/selection, capture/window cmds | Vocabulary/learning expansion, cloudflare-api |
| config + engine settings | Project manager (UI deleted, BE remains); **no plugin marketplace** (first-party only) |

**God surfaces:** `lib.rs` ~268 handlers · `AppState` ~136 State grabs · `capture.rs` / `window.rs` fat · `App.tsx` event bus · learning bolted on same binary.

**After OCR+engine only:** (1) label/group command surface (2) peel capture/window into services (3) demote learning/project in UI (4) AppState split **last / optional**.

**Avoid big-bang** multi-crate: untyped `invoke` strings + OCR geometry regressions.

---

## E. Next concrete work (agents done; implement next)

| Step | Owner task | Done when |
|------|------------|-----------|
| E1 | OCR geometry constants single source (TS+Rust) | unit tests + no magic 32/280 drift |
| E2 | OCR session: one capture path, no double flash on first select | I1 smoke |
| E3 | Line box alignment audit vs pot/STranslate | I5 + manual smoke 5 |
| E4 | Inventory translate call sites → façade adapters table | **Done** → [ENGINE_FACADE_INVENTORY.md](./ENGINE_FACADE_INVENTORY.md) |
| E5 | Hook UI: label experimental; default H-Code collapsed | **Done** (badge + i18n; collapsed already) |

Reference clones: `tmp/reference/oss/` (gitignored).
