# Current focus (2026-07-27)

Stabilization before broad features. **Freeze** new learning/cloud/multi-end work until OCR + engine façade are healthy.

## Done recently (2026-07-27)

| Slice | Where |
|-------|--------|
| **AiNiee ports** | `post_process` symbol repair + `response_check` batch validation; study map in [REFERENCE_STUDY.md](./REFERENCE_STUDY.md) §6 |
| **FE chrome** | `.ui-*` typography in `src/index.css`; `Icon` + `PageHeader`; page titles unified |
| **Collection thin slice** | Eudic / AnkiConnect / Shanbay — [COLLECTION.md](./COLLECTION.md) |
| **Engine settings / LLM router** | [ENGINE_SETTINGS.md](./ENGINE_SETTINGS.md), [LLM_PROVIDERS.md](./LLM_PROVIDERS.md) |

## Priority order

1. **Repo hygiene** — done baseline; **now:** keep `docs/` index honest, don’t commit `tmp/reference` (gitignored); prefer `AiNiee-extract` over failed empty clones.
2. **Module map** — done → [MODULE_MAP.md](./MODULE_MAP.md).
3. **Trust base (partial)** — `loadConfig` + save guard; pnpm; CI; icons; extension hosts; caiyun encrypt. [HEALTH_AUDIT.md](./HEALTH_AUDIT.md).
4. **OCR vertical** — smoke checklist in code; **manual** green via [OCR_SMOKE.md](./OCR_SMOKE.md) still required before “OCR fixed”.  
   **Later:** pinned region watch polish (`OCR_STRATEGY.md`) only after smoke green.
5. **Engine façade** — inventory + `run*` + channels + batch prepare **done** → [ENGINE_FACADE_INVENTORY.md](./ENGINE_FACADE_INVENTORY.md).  
   **Optional later:** EngineSettings split; stream multi-endpoint; per-segment batch cache/TM; wire `parse_numbered_response` into LLM multi-seg path if product needs it.
6. **Hook verdict** — passive UIA/clipboard = supported; DLL inject = experimental (**UI labeled**); no Luna deep-dive now.
7. **Extension** — after desktop OCR+engines stable.

## Non-goals (for now)

- Full AppState rewrite, multi-platform Cloudflare, vocabulary expansion (FSRS/learning product), plugin marketplace.
- AiNiee game extractors (Mtool/T++/Renpy) and Qt UI — out of product scope.

## Allowed thin slices

- **Collection adapters** first-party only — [COLLECTION.md](./COLLECTION.md).
- **Post-process / quality** polish that stays on the translation façade (symbol repair, response checks) — already landed; no new verticals.

## Acceptance (OCR)

Manual smoke in OCR_INVARIANTS / OCR_SMOKE must pass before claiming OCR fixed.

## Full audit & references

- 2026-07-26 multi-agent coverage → [FULL_PROJECT_AUDIT.md](./FULL_PROJECT_AUDIT.md).
- OSS steal map (STranslate / pot / read-frog / **AiNiee**) → [REFERENCE_STUDY.md](./REFERENCE_STUDY.md).
