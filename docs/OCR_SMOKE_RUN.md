# OCR smoke run log (2026-07-27)

Goal: `docs/OCR_SMOKE.md` — order **3 → 4 → 1 → 2 → 5–11**.  
Failures: **table step # + phenomenon** only.

## Automated / code gate (this session)

| Gate | Result |
|------|--------|
| Vitest OCR suite (23) | **PASS** (incl. min width 460) |
| Cargo build (debug) | **PASS** |
| Window perms (min/max/close) | **Fixed** — `capabilities/default.json` |
| Main white-over-OCR | **Fixed** — hide main before frame; close selector never auto-shows main |
| Progressive OCR paint | **Added** — OCR text first, then translation |
| Frame engine switch | **Added** — toolbar select + 机 toggle |
| #10 skip log | **Added** — `console.info('[OCR] continuous skip…')` |
| AiSettings crash | **Fixed** — `availableModels[id]?.length` |
| OCR black screen root cause | **Fixed** — see below |

### Black screen root cause (code audit 2026-07-27)

| Cause | Evidence | Fix |
|-------|----------|-----|
| Selector HWND forced visible before freeze image | `force_hwnd_cover_physical` always used `SWP_SHOWWINDOW` while builder set `visible(false)` + near-black bg | `show=false` for selector pre/post pin; region frame still `show=true` |
| Selection-effect cleanup crash | `unlisteners` undefined in that effect → listener leak / double handlers | `unlisten?.()` |
| Snapshot fail flashed black chrome | `win.show()` on load error after main already hidden | close only; cancel event restores main |
| Hide storm on handoff | FE `main.hide()` ×4 + Rust hide | one FE hide + close_selector + one re-hide |

Rebuild debug binary / `pnpm run tauri dev` required before manual retest.

## Manual (operator)

Run: debug `moontranslator.exe` + Vite `:5173`, or `pnpm run tauri dev`.

| Order | # | Result | Notes |
|-------|---|--------|-------|
| 1 | **3** 窄框 | ☐ | min frame 460 — engine控件应可点 |
| 2 | **4** 空白区 | ☐ | |
| 3 | **1** 一轮框选 | ☐ | 不应再被白色主窗挡住 |
| 4 | **2** 叠字 | ☐ | |
| 5 | **5** 拖框 | ☐ | |
| 6 | **6** 缩放 | ☐ | |
| 7 | **7** 刷新 | ☐ | |
| 8 | **8** Follow 移窗 | ☐ | |
| 9 | **9** Follow 绑点 | ☐ | |
| 10 | **10** ▶ 静 | ☐ | 看 console skip 日志 |
| 11 | **11** 滚动变字 | ☐ | |

**Do not** mark OCR fixed until all ☐ → ✅.

### Operator focus (this round)

1. 截图后主窗是否仍盖住翻译框（白页）→ 应已修  
2. 工具栏引擎下拉 +「机」启用/关闭 → 应可切换并重译  
3. 首帧是否更快出现原文叠字（再等译文）

### Code audit risks (watch while testing)

- **#10**: fingerprint skip logs via `console.info`  
- **#2**: if image size probe fails, brief DPR fallback misalign possible  
- **#7**: GDI fail falls back to slower full capture  
- TS/Rust **32 / 460** constants must stay in sync (`ocrRegionGeometry.ts` ↔ `window.rs`)
