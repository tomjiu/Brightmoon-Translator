# AGENTS.md

## Build / Release workflow

- **Do NOT run `pnpm tauri build` locally.** Local Rust builds are slow and
  block other work. All Windows installers and browser extensions are built in
  the cloud via GitHub Actions.
- Release builds are triggered by pushing a `v*` tag (see
  `.github/workflows/release.yml` on GitHub):
  1. `git push origin master`
  2. `git tag vX.Y.Z` (bump minor for features, patch for fixes)
  3. `git push origin vX.Y.Z`
  4. `gh run list --workflow=release.yml` to monitor the build.
- The release is created as a **draft**; publish it on GitHub once the build
  finishes.
- Local verification before releasing: `pnpm exec tsc --noEmit`,
  `pnpm exec vitest run`, and `npx eslint` (0 errors).
