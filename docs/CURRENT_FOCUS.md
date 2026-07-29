# Current focus (2026-07-29)

**Product rule:** all capabilities are **first-party in-repo**. No plugin marketplace / external plugin host.

**Do not** change OCR session lifecycle (`ocr_begin/end_session_*`, selector→result baton) or other already-solid OCR code unless fixing a proven regression.

**Git checkpoint:** local master ahead origin (~15+); tip includes leave-dismiss / hover unit / hook pump.

---

## Unfinished inventory (work remaining)

### A — 划词 / 悬停 (incremental, do next)

| Item | Status | Notes |
|------|--------|--------|
| Pop / auto / hotkey selection | **Working baseline** | `WH_MOUSE_LL`; min_drag_px; ocr_modifier_key |
| Hover dictionary | **Improved** | word/sentence; Alt→句; edit-focus block; leave 120ms |
| UIA TextPattern word-at-point | **Improved** | RangeFromPoint + Word/Paragraph expand; ratio fallback |
| Multi-monitor clamp (overlay/pop) | **Improved** | `clamp_rect_to_cursor_monitor` work area |
| Dict-only hotkey (QTranslate D) | **Improved** | Optional `dictionaryLookup` → `trigger_dictionary_lookup` |
| Overlay light/dark feel | **Improved** | elevated card + shadow tokens light/dark |
| Multi-engine selection text | **Improved** | `display_text` shared |
| Manual smoke | **Open** | User acceptance |

**Do not:** rewrite OCR selector/session; do not expand free-hover OCR on terminals.

### B — Hook / inject (after 划词 more solid)

| Item | Status | Notes |
|------|--------|--------|
| Hook monitor UI | Exists | Experimental |
| Passive UIA/clipboard monitor | **Production-ish** | `start_hook_monitor` applies active/auto profile |
| DLL inject → `TranslationService` | **Improved** | multi-module IAT + host pump + **dedup/noise filter** |
| Profiles applied on start | **Done (passive path)** | `hook_cmd::start_hook_monitor` already applies profile |
| Bundle hook DLL in release | **Improved 2026-07-29** | `tauri.conf.json` resources + richer `find_hook_dll` (exe-dir, CARGO_MANIFEST_DIR, Debug/Release); DLL copied to `src-tauri/bin/` when built |

### C — 长文本 / documents / batch

| Item | Status | Notes |
|------|--------|--------|
| Document batch translate | **Improved** | `BatchManager::process` → `run_batch` waves (TM/cache/LLM pack); cancel/pause between waves |
| Batch pre/glossary/TM/cache | **Improved 2026-07-29** | Non-OCR `translate_batch_core`: prepare + TM + cache hit/set + history; OCR path unchanged |
| Long-text chunk quality | **Improved** | LLM numbered pack: TM/cache pre-hit + pack-fail fallback |
| BatchConfig.engine honor | **Improved** | named engine + settings `batchPreferredEngine` |
| Sentence split quality | **Improved** | abbrev/decimal-aware punct bounds |
| **Code-side ready for user smoke** | **Yes** | Manual checklist open for owner |

### D — Other (later)

| Item | Status |
|------|--------|
| Browser extension depth | Tier 1 — not blocking desktop |
| ExternalCall HTTP extras | Low / last |
| Plugin marketplace | Removed (non-goal) |

---

## Roadmap tiers (owner intent)

### Tier 0 — Stabilize desktop 划词 (current)

- Selection solid enough for daily use (gesture + clipboard + pop)
- Hover safe (no terminal spam, no stuck cards while typing)
- Overlay readable + theme-aware

### Tier 1 — After 划词 good enough

- Browser extension depth
- OCR layout / polish (**only polish**, no session rewrite)

### Tier 2 — Hook / inject

- Luna-class inject **after** OCR + selection stable

### Tier 3 — Last

- Extra HTTP control routes
- No second translate/OCR orchestrator

---

## Done recently

| Slice | Where |
|-------|--------|
| Multi-engine selection overlay | `TranslateResponse::display_text`; hotkey path + auto_watch |
| BatchManager → run_batch | `batch.rs` wave process (not per-task run_full) |
| min_drag_px + hover dict→MT | `mouse_hook::set_min_drag_px`; hover miss falls through |
| Hover cursor-ratio word | `extract_word_candidate_with_hint` + UIA bounds |
| LLM batch TM/cache + fallback | `translate_batch_core` numbered path |
| OCR force modifier | `ocr_modifier_key` + `ocr_force_allowed()` |
| Hook multi-module IAT | `hook-dll/src/hook_text.cpp` EnumProcessModules |
| Hook host pump | `hook_inject_cmd` start on inject / stop on eject |
| Hover unit + edit-focus | `hover_unit` + `is_editable_control_focused` |
| Hook noise/dedup | `hook_text_is_noise` + 8s dedup window |
| TextPattern hover word | `try_text_pattern_at_point` |
| Multi-monitor clamp | `overlay/positioner.rs` |
| Batch engine override | `BatchConfig.engine` → named engine |
| Dictionary hotkey | `hotkeys.dictionaryLookup` |
| Selection UX sprint | `selection/{mouse_hook,auto_watch,pop_button,process_class,clipboard,hover_pick}` |
| Selection settings | `SelectionSettings.tsx` |
| Hotkey live re-register | `hotkey.rs` + `save_config` |
| Compact overlay + theme hook | `overlay/html_builder.rs`, `set_overlay_theme` |
| OCR session lifecycle | **Frozen** — do not touch |
| Plugin marketplace removed | first-party only |

## Architecture (unchanged)

```
UI / tray / extension / HTTP  →  commands / api_server
  → translate · collection · capture · post_process
  → engine/* · collection/* · Win32
```

New work = first-party module + one command/settings key + CI green.

## References

- [MODULE_MAP.md](./MODULE_MAP.md) · [REFERENCE_STUDY.md](./REFERENCE_STUDY.md) · [REFERENCE_SELECTION_UX.md](./REFERENCE_SELECTION_UX.md) · [REFERENCE_OCR_CAPTURE.md](./REFERENCE_OCR_CAPTURE.md) · [FEATURES.md](./FEATURES.md) · [OCR_SMOKE.md](./OCR_SMOKE.md)
