# Current focus (2026-07-29)

**Product rule:** all capabilities are **first-party in-repo**. No plugin marketplace / external plugin host.

**Do not** change OCR session lifecycle (`ocr_begin/end_session_*`, selector→result baton) or other already-solid OCR code unless fixing a proven regression.

**Git checkpoint:** `c19611d` — selection/hover UX (hooks, pop button, ClipWait, settings).

---

## Unfinished inventory (work remaining)

### A — 划词 / 悬停 (incremental, do next)

| Item | Status | Notes |
|------|--------|--------|
| Pop / auto / hotkey selection | **Working baseline** | Easydict-style `WH_MOUSE_LL`, process class, ClipWait |
| Hover dictionary | **Partial** | Free dwell; **terminals skip**; typing dismisses card; junk-word filter |
| Overlay card polish | **Partial** | Compact card + theme via `set_overlay_theme`; still improve light/dark feel |
| Dict miss → MT | **Partial** | Real dict only on hover; selection falls through to MT |
| Multi-engine selection text | **Partial** | Show multiple engines when router returns >1 result |
| Manual smoke (browser / notepad / terminal) | **Open** | User still reports terminal OCR junk / short quality |

**Do not:** rewrite OCR selector/session; do not expand free-hover OCR on terminals.

### B — Hook / inject (after 划词 more solid)

| Item | Status | Notes |
|------|--------|--------|
| Hook monitor UI | Exists | Experimental |
| DLL inject → `TranslationService` | **Incomplete** | Messages not fully wired; IAT patch weak |
| Profiles applied on start | **Incomplete** | CRUD exists, apply path weak |
| Bundle hook DLL in release | **Open** | Dev-only path today |

### C — 长文本 / documents / batch

| Item | Status | Notes |
|------|--------|--------|
| Document batch translate | Exists | `BatchManager` → `run_full` per task |
| Batch pre/glossary/TM/cache | **Improved 2026-07-29** | Non-OCR `translate_batch_core`: prepare + TM + cache hit/set + history; OCR path unchanged |
| Long-text chunk quality | **Open** | Numbered LLM batch parse exists; extend carefully |

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

- [MODULE_MAP.md](./MODULE_MAP.md) · [REFERENCE_STUDY.md](./REFERENCE_STUDY.md) · [REFERENCE_SELECTION_UX.md](./REFERENCE_SELECTION_UX.md) · [FEATURES.md](./FEATURES.md) · [OCR_SMOKE.md](./OCR_SMOKE.md)
