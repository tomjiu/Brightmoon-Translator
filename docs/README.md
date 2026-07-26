# Docs index (canonical)

**Rule:** live specs live here. Session notes, “COMPLETE/FINAL” dumps, and old OCR progress reports go under `archive/` only.

## Start here

| Doc | Role |
|-----|------|
| [QUICK_START.md](./QUICK_START.md) | Run desktop + extension |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Layout, stacks, modules |
| [FEATURES.md](../FEATURES.md) | Feature checklist (root) |
| [ROADMAP.md](./ROADMAP.md) | Multi-platform plan (long-range) |
| [../ROADMAP.md](../ROADMAP.md) | Architecture debt vs Luna (near-term) |
| [CURRENT_FOCUS.md](./CURRENT_FOCUS.md) | **Active refactor focus** |
| [MODULE_MAP.md](./MODULE_MAP.md) | Call graphs + god objects + slice order |
| [HEALTH_AUDIT.md](./HEALTH_AUDIT.md) | Full-repo health (CI, config, security, dead code) |
| [FULL_PROJECT_AUDIT.md](./FULL_PROJECT_AUDIT.md) | **2026-07-26** multi-agent full coverage (all verticals + OSS matrix) |
| [../FEATURES.md](../FEATURES.md) | Feature checklist (code-truth) |

Stale status dumps (`IMPLEMENTATION_STATUS`, `project-triage`, old MODULE_CHECKLIST, …) live under `archive/`.

## Product verticals (reference → Moon)

| Area | Canonical docs | Study first |
|------|----------------|-------------|
| OCR / region frame | [OCR_INVARIANTS.md](./OCR_INVARIANTS.md), [OCR_STRATEGY.md](./OCR_STRATEGY.md), [OCR_SMOKE.md](./OCR_SMOKE.md) | pot-desktop, STranslate |
| Engines / modes | [translation-modes.md](./translation-modes.md), [ENGINE_FACADE_INVENTORY.md](./ENGINE_FACADE_INVENTORY.md), [API.md](./API.md) | pot-desktop, STranslate |
| Hook / inject | (code + FEATURES; smoke before trust) | LunaTranslator |
| Extension | [browser-vs-desktop.md](./browser-vs-desktop.md), [extension-hover-translation.md](./extension-hover-translation.md) | immersive-translate, read-frog |
| Integration matrix | [integration-plan.md](./integration-plan.md) | five-source summary |
| Local clones | `tmp/reference/` (gitignored) | see `tmp/reference/README.md` |
| Reference deep study | [REFERENCE_STUDY.md](./REFERENCE_STUDY.md) | pot / STranslate / read-frog steal map (2026-07-26) |

## Other active docs

- Plugins: [PLUGIN_API.md](./PLUGIN_API.md), [PLUGIN_DEV.md](./PLUGIN_DEV.md)
- Dictionary / learning: [MULTI_SOURCE_DICTIONARY.md](./MULTI_SOURCE_DICTIONARY.md), [VOCABULARY_SYSTEM.md](./VOCABULARY_SYSTEM.md), [LEARNING_SYSTEM_ARCHITECTURE.md](./LEARNING_SYSTEM_ARCHITECTURE.md)
- Ops: [TROUBLESHOOTING.md](./TROUBLESHOOTING.md), [PERFORMANCE.md](./PERFORMANCE.md), [CONTRIBUTING.md](./CONTRIBUTING.md), [unsafe-guidelines.md](./unsafe-guidelines.md)
- Cloud / multi-end (planned): [API_SPEC.md](./API_SPEC.md), [DATABASE.md](./DATABASE.md), [CROSS_PLATFORM.md](./CROSS_PLATFORM.md)

## Archive

`docs/archive/` — historical session summaries, superseded OCR notes, one-off test reports. **Do not treat as current truth.**

## Hygiene

- Do not add new `SESSION_*` / `*_COMPLETE_*` / `FINAL_*` docs at `docs/` root.
- Agent cwd files (`tmpclaude-*`), `pi-session-*.html`, Windows `nul`: ignored; run `scripts/cleanup-temps.ps1` if they reappear.
