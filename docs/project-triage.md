# Project Triage

Date: 2026-05-05

This table is the working handoff for future AI work. Treat an item as done only when its acceptance checks pass.

| Priority | Area | Current State | Owner Task | Acceptance Checks | Status |
|---|---|---|---|---|---|
| P0 | Build scripts | Desktop build works; extension build had ESM/CJS drift | Keep `node extension/build.js` working and package Chrome/Firefox output directories | `node extension/build.js` exits 0; `extension/dist/chrome/manifest.json` exists; `extension/dist/firefox/manifest.json` exists | Fixed baseline |
| P0 | Tests | Frontend test command previously failed because no tests matched | Add behavior tests for every new bugfix before touching production code | `npm test` exits 0 and includes at least one real test for the changed behavior | Fixed baseline |
| P0 | API protocol | Desktop response uses camelCase JSON for browser/frontend consumers | Keep Rust/TS/extension protocol fields aligned | `cargo test translate_response_serializes_detected_language_as_camel_case`; frontend reads `detectedLanguage` | Fixed baseline |
| P1 | Browser extension | Chrome and Firefox code paths diverged; old `firefox-extension/` still exists | Decide one-source build vs two maintained trees, then remove or archive the unused path | One documented install path per browser; no stale manifest/icon references | Open |
| P1 | Desktop bridge | Extension probes `127.0.0.1:60828`, but desktop API server is opt-in | Make bridge status explicit in popup and settings, or enable API server during extension onboarding | Popup clearly shows desktop bridge on/off; selection translation behavior is predictable with desktop app off | Open |
| P1 | OCR | Actual OCR uses Tesseract.js; Rust `ocr_screen` is a placeholder and not registered | Either remove placeholder command/docs or implement/register native OCR intentionally | No docs claim Windows.Media.Ocr unless implemented; OCR path has an automated smoke test or manual checklist evidence | Open |
| P1 | Docs | README and architecture mixed planned and implemented features | Split docs into `implemented`, `experimental`, and `planned` sections | A new developer can run desktop app and extension from README without guessing | Partially fixed |
| P2 | Lint warnings | ESLint has warnings for hook deps and `any` usage | Reduce warnings without behavior changes | `npm run lint` reports 0 errors and intentionally accepted warning count, preferably 0 | Open |
| P2 | Lockfiles | Both `package-lock.json` and `pnpm-lock.yaml` exist | Chose npm, deleted pnpm-lock.yaml (commit f70e2c3) | README, CI/check commands, and lockfile all use the same package manager | Done |
| P2 | Git hygiene | Local branch is 21 commits ahead of origin | Create an integration branch or push intentionally after review | `git status --short --branch` has no unexplained changes; branch strategy documented | Open |

## Supervision Rules

- No new feature work until P0 remains green after a fresh verification run.
- Every bugfix needs a failing test or a written reason why it can only be manually verified.
- Any AI handoff must include changed files, commands run, command results, and remaining risk.
- Do not accept “works” claims without the exact command output or a reproducible manual checklist.
