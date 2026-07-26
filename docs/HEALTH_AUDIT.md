# Full health audit (2026-07-25)

Five parallel scans: build/CI · dead code · security · FE↔BE contract · extension/hygiene.  
**Does not replace** [MODULE_MAP.md](./MODULE_MAP.md) (OCR/engine/hook product map).

## Patch status (2026-07-25 session)

| ID | Status |
|----|--------|
| B1 pnpm + lock + CI/release/tauri scripts | **Fixed** (uses existing `pnpm-lock.yaml`) |
| B2 CI `on:` re-enabled | **Fixed** (still custom runner) |
| B3 icons list → only `icon.ico` | **Fixed** |
| C1 `loadConfig` on MainApp mount | **Fixed** |
| C2 hotkeys fields + ghost flags + save-if-!loaded | **Partial** (hotkeys+ghost+guard; full schema gen still open) |
| E1 bridge docs | **Fixed** (README; default still off by design) |
| E2 extension hosts + DeepLX default | **Fixed** |
| S2 caiyun encrypt/mask + API sanitize | **Fixed** |
| OCR / engine façade / Hook IAT | OCR code ready (manual smoke); façade **E4 inventory done** → ENGINE_FACADE_INVENTORY.md; Hook IAT still open |

## Executive summary

| Area | Worst finding |
|------|----------------|
| Build/CI | ~~No lockfile / CI dead~~ → pnpm; runner label still custom |
| Contract | ~~loadConfig never~~ → fixed; residual incomplete FE schema |
| Extension | hosts fixed; API still default off (documented) |
| Security | caiyun fixed; API still no auth when enabled |
| Dead code | still open (orphans / half cmds) |
| Docs | FEATURES rewritten; old status → archive |

**Already known product bugs (OCR flicker/offset, engine chaos, Hook IAT)** stay first for UX; items below are **repo-wide** debt that will sabotage fixes if ignored.

---

## P0 — Fix before trusting ship / long refactor

| ID | Finding | Evidence | Effort |
|----|---------|----------|--------|
| **B1** | No `package-lock.json`; `.gitignore` ignores pnpm-lock; CI/release/`tauri` use `npm ci` | workflows, gitignore | S–M pick **one** PM |
| **B2** | `ci.yml` has `on:` commented — CI effectively dead | `.github/workflows/ci.yml` | S |
| **B3** | Bundle icons listed but only `icon.ico` present | `tauri.conf.json` vs `icons/` | S |
| **C1** | App only `loadDefaults()`; **`loadConfig` never used in production** | `App.tsx`, `configStore.ts` | S |
| **C2** | Incomplete FE `AppConfig` + ghost fields → **save clobbers** disk config | `types/index.ts` vs `models/config.rs` | M |
| **E1** | Extension assumes `:60828`; `api_server_enabled: false` | SW + `config.rs` | S product decision |
| **E2** | MV3 missing hosts for DeepLX free / MS edge translator | `manifest.json` vs fetch URLs | S |
| **D1** | Status docs false: hover/tray/TM/OCR claimed missing | FEATURES, IMPLEMENTATION_STATUS, triage | S rewrite/archive |

## P1 — High risk / correctness

| ID | Finding |
|----|---------|
| **B4** | Custom runner `windows-2025-vs2026` only; release npm+extension zip fragile |
| **B5** | `dictionaries/` gitignored; runtime expects ECDICT — silent empty dict |
| **B6** | Hook DLL not in bundle; thin OCR/hook tests; e2e without `build.js` first |
| **S1** | ~~no auth~~ Bearer/X-Api-Token (2026-07-26); still loopback-only, restart to apply enable |
| **S2** | Caiyun `api_token` not encrypted/masked like other secrets |
| **S3** | Plugin install-from-URL + weak “sandbox” = user-level arbitrary code |
| **C3** | ~18 files raw `invoke` bypass `safeInvoke`; silent catches (dict/OCR) |
| **C4** | Defaults drift (ocrEngine auto vs winrt; hotkeys empty vs Rust) |
| **C5** | CacheStats FE shape ≠ Rust; dual metrics naming |
| **K1** | ~~Office/image + quality/speech~~ all registered in `generate_handler` (2026-07-26) |
| **K2** | ~~Orphans FE~~ Dictionary/clipboard/project/stream stores archived; SyncSettings **wired**; `project_cmd` BE still orphan |
| **K3** | Dual selection cmds; post-process BE without settings UI |
| **E3** | `build.js` ships `tests/` into dist; DeepLX endpoint leftover localhost |
| **H1** | Dual ROADMAP (root Luna vs docs multi-cloud) vs CURRENT_FOCUS |
| **H2** | CONTRIBUTING wrong `npm` scripts; README snake_case config sample |

## P2 — Hygiene

- `tmp/reference` ~259MB, `github-data/` not ignored, `archive/` untracked, OLD exe / db.bak  
- ja/ko i18n −5 keys; settings chrome Chinese-only  
- ~67 `unsafe`; `security.rs` / `process_list` missing SAFETY comments  
- `pnpm-workspace` `allowBuilds` false for esbuild/tesseract  
- Husky rustfmt `--check` blocks commits  

---

## Severity counts (approx)

| Sev | Themes |
|-----|--------|
| P0 ×8 | lockfile, CI off, icons, config load/save, bridge, hosts, false docs |
| P1 ×15 | runner, dict asset, tests, API auth, secrets, invoke, dead cmds, extension build, dual roadmap |
| P2 | clutter, i18n, unsafe docs, husky, packaging ACL |

---

## Recommended work streams (parallel-safe)

```
Stream A — Trust base     B1 lockfile → B2 CI on → B3 icons → cargo/npm check green
Stream B — Config safety  C1 loadConfig → C2 schema align → C3 safeInvoke migration (core paths first)
Stream C — Product UX     OCR (MODULE_MAP) → engine façade → Hook gate
Stream D — Extension      E1 bridge policy → E2 hosts → E3 build exclude tests
Stream E — Debt trim      K1–K3 delete-or-wire; D1/H1 doc truth; ignore github-data
Stream F — Security later S1–S3 when enabling API/plugins for real users
```

**Do not** big-bang modularize until A+B+C OCR slice green.

## Verify commands

```powershell
# lockfile truth
Test-Path package-lock.json; Test-Path pnpm-lock.yaml

npm run check; npm run lint; npm run test:unit
cd src-tauri; cargo check
# after lockfile fix:
# re-enable ci.yml on:; npm ci in clean clone
```

## Related docs

- [CURRENT_FOCUS.md](./CURRENT_FOCUS.md) · [MODULE_MAP.md](./MODULE_MAP.md) · [OCR_INVARIANTS.md](./OCR_INVARIANTS.md)
