# Current focus (2026-07-28)

**Product rule:** all capabilities are **first-party in-repo**. No plugin marketplace / external plugin host.

**Do not** change OCR session lifecycle (`ocr_begin/end_session_*`, selector→result baton).

---

## Roadmap tiers (owner intent)

### Tier 0 — Stabilize now (before new verticals)

| Item | Notes |
|------|--------|
| Desktop OCR + 划词 (selection) solid | Session lifecycle done; **manual smoke later OK** |
| Engines / collection / control / settings wiring | Wave A/B landed |
| Keep façades modular | No second translate/OCR orchestrator |

### Tier 1 — After core desktop is “good enough”

| Item | Priority | Intent |
|------|----------|--------|
| **Browser extension depth** | **Need** | Official extension already exists; deep batch/context/queue when **fine-tuning browser side** (not blocking desktop) |
| **OCR layout analysis** (S6-style merge/split) | **Medium** | Needed for messy lines; not urgent |
| **OCR overlay look** (S7 font/theme/paint order) | **Low** | Do eventually; polish only |
| **Recognize workspace page** (pot P7-like) | **Maybe later** | Mark as possible; **unlikely to copy pot UI**. Prefer **mature OCR products / stronger OCR backends** for “pro OCR” capability rather than building a full recognize IDE in-app |

### Tier 2 — After OCR + 划词 are done

| Item | Priority | Intent |
|------|----------|--------|
| **Inject / Hook (Luna-class)** | **Next major vertical** | App-injection translation for **shell browsers / packaged web apps** → nearer-native in-app text. **Only after** OCR + selection translate are stable. Passive remains experimental until then. |

### Tier 3 — Last / optional

| Item | Priority | Intent |
|------|----------|--------|
| **ExternalCall / more local HTTP control** | **Low — last** | Extra scriptable routes + mutex; only if **no architecture churn** |
| Floating multi-engine popup (pot P3–P6) | **Skip** | Main window multi-engine is enough |
| Plugin marketplace | **Removed** | First-party only |
| Multi-end Cloudflare / FSRS product boom / AppState rewrite | **Non-goal** for now | |

---

## Done recently

| Slice | Where |
|-------|--------|
| OCR session lifecycle (main out of OCR) | `ocr_begin/end_session_*` |
| Feature sprint Wave A/B | collection, control/bridge, tray, numbered LLM, OCR UI feedback, post-process UI, engine sections |
| CI + repo hygiene | pnpm, demos archive, Claude trailers stripped |
| Plugin marketplace stack removed | no `plugin.rs` / sandbox / PluginSettings / plugin-sdk |

## Architecture (unchanged)

```
UI / tray / extension / HTTP  →  commands / api_server
  → translate · collection · capture · post_process
  → engine/* · collection/* · Win32
```

New work = first-party module + one command/settings key + CI green.

## References

- [MODULE_MAP.md](./MODULE_MAP.md) · [REFERENCE_STUDY.md](./REFERENCE_STUDY.md) · [FEATURES.md](./FEATURES.md) · [OCR_SMOKE.md](./OCR_SMOKE.md)
