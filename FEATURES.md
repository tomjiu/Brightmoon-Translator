# Moon Translator — feature checklist

References: immersive-translate, LunaTranslator, read-frog, pot-desktop, STranslate  
Integration matrix: `docs/integration-plan.md`. Live compass: `docs/CURRENT_FOCUS.md`, `docs/MODULE_MAP.md`.

Status is **code-truth as of 2026-07-28** (not marketing). OCR GUI smoke still manual.

**Product:** first-party only — no plugin marketplace / external plugin host.

---

## 1. Desktop

### 1.1 Main translator
| Feature | Status | Notes |
|---------|--------|-------|
| Multi-line input + debounce | ✅ | ~500ms |
| Auto detect / lang swap / copy | ✅ | |
| Multi-engine results | ✅ | Via routing strategy |
| Stream (LLM token stream) | ✅ | `translate_stream` + MainTranslator |
| History | ✅ | |
| Global hotkeys | ✅ | Config-driven; needs `loadConfig` (fixed) |
| System tray | ✅ | Tray menu in `lib.rs` |

### 1.2 OCR screenshot
| Feature | Status | Notes |
|---------|--------|-------|
| Fullscreen snapshot + region | ✅ | Single path: selector → region frame |
| WinRT / Youdao / tesseract.js | ✅ | |
| Continuous refresh | ✅ | **Default OFF** (I1) |
| Follow window | ✅ | I6 |
| Box/flicker residuals | ⚠️ | See `OCR_INVARIANTS.md` — active work |

### 1.3 Engines
| Feature | Status | Notes |
|---------|--------|-------|
| LLM / Google / Baidu / Youdao / DeepL / … | ✅ | Multiple providers |
| Parallel / fallback strategies | ✅ | Config `routingStrategy` |
| Translation cache / TM | ✅ | Cache + TM commands/UI |
| Unified façade | ⚠️ | Multiple entrypoints — `MODULE_MAP` B |
| External plugins / marketplace | ❌ removed | First-party engines only |

### 1.4 Hook
| Feature | Status | Notes |
|---------|--------|-------|
| UIA + clipboard monitor | ✅ | Default sources |
| DLL inject (H-Code) | ⚠️ experimental | IAT likely broken; not product |
| Profiles UI | ✅ | Not fully applied at monitor start |

---

## 2. Browser extension

| Feature | Status | Notes |
|---------|--------|-------|
| Selection popup | ✅ | |
| Page translate | ✅ | |
| Hover translate | ✅ | `hover-translator.js` (was wrongly ❌) |
| Desktop bridge `:60828` | ⚠️ | Server **default off** — enable in settings |
| Bilingual / PDF / YT | ❌ | vs immersive-translate |

---

## 3. Explicitly frozen / secondary

Vocabulary learning, Cloudflare multi-end, plugin marketplace — see `CURRENT_FOCUS` non-goals until OCR+engines stable.

---

## 4. Priority (now)

1. OCR vertical (invariants smoke)  
2. Engine façade  
3. Extension bridge UX  
4. Doc/status hygiene (this file + archive)
