# Full project audit (2026-07-26)

**Method:** Multi-agent static review covering all major verticals + OSS reference coverage.  
**Not a ship claim:** OCR still needs manual `OCR_SMOKE.md`. Domain test **fixtures fixed** (2026-07-26); `cargo test --lib` may still fail at **run** with `STATUS_ENTRYPOINT_NOT_FOUND` (DLL env), not compile.

**Related:** [CURRENT_FOCUS.md](./CURRENT_FOCUS.md) · [HEALTH_AUDIT.md](./HEALTH_AUDIT.md) · [MODULE_MAP.md](./MODULE_MAP.md) · [ENGINE_FACADE_INVENTORY.md](./ENGINE_FACADE_INVENTORY.md) · [ENGINE_SETTINGS.md](./ENGINE_SETTINGS.md)

---

## 1. Coverage matrix (what was reviewed)

| Area | Depth | Key docs / code |
|------|-------|-----------------|
| OCR rectangle + watch skeleton | **Deep** (multi-round fix + audit) | OCR_* docs, Ocr* components, capture/window |
| Engine façade / channels | **Deep** | ENGINE_FACADE_INVENTORY, translation service |
| Engine settings UI + LLM providers→Router | **Deep** | ENGINE_SETTINGS, plans/* |
| Hook passive / H-Code / selection / replace / clipboard | **Deep** (this audit) | hook_*, selection_*, translate clipboard |
| Document / subtitle / image-file / plugin | **Deep** (this audit) | *_cmd, DocumentsViewer, plugin_* |
| Security / config drift / CI / dead cmds | **Deep** (this audit) | api_server, config, domain tests, lib.rs |
| Extension / TTS / dict / learning / sync / CF | **Deep** (this audit) | extension/, tts, vocabulary, cloudflare-api |
| OSS references | **Medium** (this audit) | tmp/reference coverage matrix |
| Multi-monitor matrix / paid dual-LLM live | **Not done** | Needs machine + keys |
| youdao-dict commercial RE | **Policy skip** | UX only |

---

## 2. P0 rollup (fix / ship / claimed-feature blockers)

| ID | Area | Finding |
|----|------|---------|
| **OCR-M** | OCR | Manual smoke 1–11 not closed in session |
| **S1** | Security | ~~unauthenticated API~~ **fixed 2026-07-26** (Bearer / X-Api-Token; /health open) |
| **T1** | Tests | ~~fixture drift~~ fixed; runtime may still `STATUS_ENTRYPOINT_NOT_FOUND` |
| **H1** | Hook | ~~profiles not applied at start~~ **fixed 2026-07-26** (auto-match + active at start) |
| **H2** | Hook | H-Code IAT wrong modules; inject can yield zero text (**frozen**) |
| **H3** | Hook | H-Code never reaches TranslationService (**frozen**) |
| **H4** | Clipboard | ~~emit-only stub~~ **fixed 2026-07-26** (`AddClipboardFormatListener` + settings wire) |
| **H5** | Selection/Replace | ~~manual INPUT 28B~~ **fixed 2026-07-26** (windows crate INPUT 40B x64) |
| **H6** | Hook stop | ~~orphan JoinHandle~~ **fixed 2026-07-26** (WM_QUIT + join 2s; Drop too) |
| **D1** | Documents | ~~orphan IPC~~ **fixed 2026-07-26** (registered + Documents tabs FE) |
| **D2** | Subtitle | ~~re-read source drops translations~~ **fixed 2026-07-26** (export takes `entries`) |

---

## 3. P1 rollup (by vertical)

### OCR (residual after reliability work)

- First-shot snapshot crop vs refresh GDI coordinate family  
- Session-reset ACK present; hide vs close lifecycle dual  
- Continuous product polish frozen until smoke green  

### Hook / selection / clipboard

- ~~Dual clipboard double-fire~~ mitigated: shared `clipboard_dedupe` claim (800ms)  
- UIA passive = full DocumentRange thrash  
- Shared mem re-read / no FreeLibrary on eject  
- Replace paste “confirm” is fake; UIA full-value false positive  

### Documents / media

- ~~No FE for DOCX/Excel/PPTX/image-file~~ minimal tabs in DocumentsViewer  
- PDF/EPUB export is plain text only  
- Plugin marketplace/sandbox FE missing; marketplace stub  

### Security / config / CI

- `llm.providers[].apiKey` + `edgeTtsToken` not encrypted like top-level keys  
- FE INITIAL omits `caiyun` → save can wipe token  
- Custom CI runner only; lib tests not run by default  

### Extension / TTS / dict / learning / sync

- Extension hardcodes `:60828`; API default off (UX trap)  
- Extension port not configurable; no auth when API on  
- ~~TTS `ttsVoice` unused~~ **TTS-1 done** (2026-07-26): `text_to_speech` uses `voice` arg → `config.ttsVoice` → lang default; `auto` lang still → en-US
- ~~FreeDictionary second source = same URL twice~~ **DICT-1 done** (removed fake duplicate)  

- ECDICT often missing → silent empty local dict  
- Learning quiz distractors fragile; etymology stub  
- Cloudflare package undeployable scaffold (freeze OK)  

### Engines

- EngineSettings still large file  
- Stream multi-endpoint not implemented  
- `youdao.useAi` not in router  

---

## 4. P2 / freeze (do not expand now)

- Full AppState rewrite, plugin marketplace productization  
- Learning expansion, CF multi-end  
- Luna inject deep-dive  
- Extension engine stack unify with desktop  
- Awwwards-style settings redesign  

---

## 5. What is actually end-to-end today

| Flow | Domain | IPC | FE | Façade | Notes |
|------|--------|-----|-----|--------|-------|
| Main translate / stream / embedded | Yes | Yes | Yes | Yes | |
| OCR region (screenshot) | Yes | Yes | Yes | Yes | Smoke pending |
| Selection / replace hotkeys | Yes | Yes | Yes | Yes | INPUT size risk |
| Hook UIA+clipboard | Yes | Yes | Yes | Yes | Profiles unused |
| PDF / EPUB translate | Yes | Yes | Yes | Yes | Export = txt |
| Subtitle translate | Yes | Yes | Yes | Yes | **Export broken** |
| DOCX/XLSX/PPTX | Yes | Yes (tabs) | Yes | Yes | Wired 2026-07-26 |
| Image file translate | Yes | **No** | No | Yes | Orphan |
| H-Code inject | Partial | Yes | Yes | **No** | Experimental |
| Main clipboard monitor | Stub | Yes | Yes | Indirect | Broken product |
| Extension offline | Yes | N/A | Yes | Local engines | |
| Extension desktop bridge | Yes | If API on | Yes | Yes | Default API off |
| WebDAV sync | Yes | Yes | Yes | N/A | |
| Cloudflare | Scaffold | No | No | No | Freeze |

---

## 6. OSS reference coverage

| Project | Studied | Still thin |
|---------|---------|------------|
| **pot-desktop** | Screenshot path + **window/Translate/Recognize/server/tray/engines** (2026-07-26) | Anki/collection, full recognize vendor list |
| **STranslate** | Continuous OCR + **OcrLayout/ImageTranslate/Clipboard/Replace/ExternalCall** (2026-07-26) | Plugin packages marketplace |
| **LunaTranslator** | Product gaps, continuous OCR concept | LunaHook engines, textio, transoptimi (frozen) |
| **immersive-translate** | FEATURES gaps only | **Clone is dist-only** |
| **read-frog** | Plan + **host/translate, prompts, providers, queues** (2026-07-26) | subtitles entrypoints, site-rules |
| **youdao-dict** | Policy (no RE) | resultui/skins if toolbar density needed |

**Deep-study writeup + Moon action map:** [REFERENCE_STUDY.md](./REFERENCE_STUDY.md)

---

## 7. Recommended fix order (post-audit)

1. ~~**T1** domain fixtures~~ done; if needed, fix Windows DLL path for test exe  
2. ~~**D2** subtitle export~~ done (pass in-memory entries)  
3. ~~**H5** Win64 INPUT~~ done (windows crate)  
4. ~~**H4** clipboard listener + settings~~ done  
5. ~~**D1** office/image IPC + FE tabs~~ done  
6. ~~**S1** API auth~~ done (token + extension storage)  
7. **OCR-M** user smoke + coordinate unify; then **S6/S7** layout overlay  
8. ~~**H1** apply profiles at start~~ done (mid-session auto-switch still open)  
9. ~~**DICT-1** fake FreeDictionary~~ done  
10. ~~**TTS-1** ttsVoice~~ done (arg → config → lang default)  
11. ~~**H6** hook stop join~~ done  
12. ~~**K1** quality/speech register~~ done  
13. ~~**pot R5/R8** control routes + tray~~ done (`/control/*` + tray 划词/剪贴板)  
14. **read-frog R*** when extension priority rises (extension product)  
15. **OCR-M** still needs **human** smoke; mid-session profile switch optional  

Do **not** open: CF deploy, H-Code rewrite, learning expansion, immersive re-clone unless source available.

---

## 8. Session work already done (context)

- OCR reliability batch (pot path, GDI refresh, ACK, FP, batch translate, …)  
- Engine façade inventory + `run*` migration  
- Engine settings honesty + routing radios  
- LLM `resolve_endpoints` + Router failover  
- Hook experimental UI label  

---

## 9. Acceptance of this meta-audit

- [x] Hook / selection / clipboard audited  
- [x] Documents / subtitle / image / plugins audited  
- [x] Security / config / tests / CI / orphans audited  
- [x] Extension / TTS / dict / learning / sync audited  
- [x] Reference coverage matrix audited  
- [x] Consolidated P0–P2 + E2E matrix written here  

**Residual unknown (honest):** multi-monitor live matrix, dual paid LLM live failover, real H-Code on third-party games, extension e2e in browser.