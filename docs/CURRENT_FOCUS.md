# Current focus (2026-07-26)

Stabilization before broad features. **Freeze** new learning/cloud/multi-end work until OCR + engine façade are healthy.

## Priority order

1. **Repo hygiene** — done (temps cleared; docs index + archive).
2. **Module map** — done → [MODULE_MAP.md](./MODULE_MAP.md).
2b. **Trust base (partial)** — `loadConfig` + save guard; pnpm; CI; icons; extension hosts; caiyun encrypt. [HEALTH_AUDIT.md](./HEALTH_AUDIT.md).
2c. **Dead FE/docs** — orphan stores/pages archived; FEATURES rewritten; stale status docs → `docs/archive/`.
3. **OCR vertical** — smoke checklist items 1–11 addressed in code; test via [OCR_SMOKE.md](./OCR_SMOKE.md).  
   **Later:** pinned region watch product polish (`OCR_STRATEGY.md`) only after manual smoke green.
4. **Engine façade** — inventory + `run*` + document/browser/OCR channel + batch/context prepare **done** → [ENGINE_FACADE_INVENTORY.md](./ENGINE_FACADE_INVENTORY.md).  
   **Settings cleanup (2026-07-26):** contract + meta + 5 routing radios + honest LLM/credentials + AiSettings safe sync → [ENGINE_SETTINGS.md](./ENGINE_SETTINGS.md).  
   **LLM providers→Router (2026-07-26):** `resolve_endpoints` + `LlmEngine::with_endpoints` failover + Router wired → plan `docs/superpowers/plans/2026-07-26-llm-providers-router.md`.  
   **Optional later:** split EngineSettings further; stream multi-endpoint; per-segment batch cache/TM.
5. **Hook verdict** — passive UIA/clipboard = supported; DLL inject = experimental (**UI labeled**); no Luna deep-dive now.
6. **Extension** — after desktop OCR+engines stable.

## Non-goals (for now)

- Full AppState rewrite, multi-platform Cloudflare, vocabulary expansion, plugin marketplace.

## Acceptance (OCR)

Manual smoke in OCR_INVARIANTS must pass before claiming OCR fixed.

## Full audit

2026-07-26 multi-agent coverage → [FULL_PROJECT_AUDIT.md](./FULL_PROJECT_AUDIT.md) (P0–P2, E2E matrix, OSS gaps, fix order).  
Reference deep-read (STranslate layout/clipboard, pot window/API/tray, read-frog batch) → [REFERENCE_STUDY.md](./REFERENCE_STUDY.md).
