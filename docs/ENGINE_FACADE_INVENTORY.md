# Engine façade inventory (E4+)

**Date:** 2026-07-26  
**Goal:** Map every translate entry → `TranslationService` method / gaps.

Target shape (from [MODULE_MAP.md](./MODULE_MAP.md)):

```
TranslateRequest { channel, mode, text|segments, options }
  → TranslationService::run / run_full / run_primary / run_batch
  → presenters / sources stay outside
```

---

## A. `TranslationService` public surface

| Method | Pipeline | Engine path | Returns |
|--------|----------|-------------|---------|
| `run` | by mode | dispatch | `TranslateOutcome` |
| `run_full` / `run_primary` / `run_batch` | convenience | same | typed |
| `translate` | **Full** (pre/glossary/blacklist/TM/cache/history) | router | `TranslateResponse` |
| `translate_primary` | prepare + TM + finalize | primary | `String` |
| `translate_stream` | prepare; stream | LLM stream | channel |
| `translate_with_context` | **prepare + finalize** (parity) | primary + context | `String` |
| `translate_batch` / embedded | **prepare + finalize per segment** (parity) | primary_with_context | batch |
| `router()` | escape hatch | — | `Router` |

---

## B. Call-site migration status

### Migrated (channel-aware)

| Channel | Sites |
|---------|--------|
| **Ui** | `translate` (default), `back_translate`, `compare`, `polish`, `translate_embedded`, batch queue |
| **Ocr** | FE `OcrScreenshotTranslator` passes `channel: "ocr"` |
| **Selection** | selection cap, `translate_selection_with_text`, window overlay translate |
| **Replace** | input_replacement |
| **Hook** | hook_monitor |
| **Http** | api_server full + primary |
| **Browser** | browser_translation selection / full-page / hover |
| **Document** | pdf, epub, docx×2, excel×2, pptx×2 |
| **Subtitle** | subtitle batch + subtitle_text |
| **Image** | image_translate per line |

### Still open / deferred

| Item | Notes |
|------|--------|
| Stream via `run` | still `translate_stream` direct (needs token channel) |
| TM/cache **write** on every batch segment | prepare done; full cache hit per line optional later |
| Compare still skips glossary-heavy full path | uses parallel router (acceptable for compare UX) |
| Extension **local** engines | after desktop stable |
| OCR multi-line → one batch command | still N× `translate` when ≤5 lines (channel=ocr) |

---

## C. Tauri `translate` request

```json
{ "text": "...", "from": "en", "to": "zh", "channel": "ocr" }
```

`channel` optional; default `ui`. Parsed in `commands/translate.rs`.

---

## D. Next (if any)

1. Optional: batch TM/cache per segment for docs  
2. Optional: single OCR batch IPC  
3. Extension local engines after smoke + façade soak  

---

## E. Acceptance

- [x] Inventory  
- [x] `TranslateRequest` + `run*`  
- [x] First wave (UI/selection/replace/hook/HTTP)  
- [x] Document/subtitle/browser/image/batch queue migration  
- [x] Batch + context **prepare/finalize** parity  
- [x] OCR FE channel tag  
- [x] Hook H-Code experimental label (i18n + badge)  
