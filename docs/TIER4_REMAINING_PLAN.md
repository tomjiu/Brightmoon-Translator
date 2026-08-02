# Tier 4 Remaining Plan — 剩余工作清单（锚点文档）

**Status:** Active — 作为后续所有 Tier 4 工作的单一事实来源
**Date:** 2026-08-01
**Depends on:** PR #8（Tier 4 batch 1+2）已合并到 `feat/tier4-m3-fixes`，CI 全绿；R4/R5/R6 已复核完成（见各节）
**Origin:** 仓库内无正式批次 3 清单（FIX_PLAN.md 已删）；本清单由 M3 设计文档、OCR_STRATEGY.md、.pi-subagents fixplan 产物、git 分支状态综合重建

---

## 0. 背景

- **Tier 4 batch 1+2 已完成**（提交 `f13a59e`，PR #8），含：ResizeWindowService、specialRules、Main container 启发式、Shadow DOM、OCR 几何后处理、一次性子进程 OCR worker、Hook DLL IAT 改进、M3 multi-session 设计文档（Draft）、Auto text region detect、Pin window manager、PDF 增量布局/font subset/capture geometry。
- PR #8 质量修复已合入 `feat/tier4-m3-fixes`：`5f540cb`（补实现 + manifest 修复）、`f01da64`（修 5 个 pre-existing 测试失败）。CI 三项全绿，537 lib 测试 0 failed / 2 ignored。
- **git 分支现状：**
  - `master` tip = `33d5241`（merge PR #7）
  - `feat/tier4-m3-fixes` = 当前工作分支（含 PR #8 全部 + 2 个修复提交）
  - `feature/next-stage-optimization` = S0-S4 工程化轨道，**13 个提交未合并**（见 R7）

---

## 1. 剩余工作总表

| 编号 | 项 | 严重度 | 阻塞 | 依据 | 状态 |
|------|-----|--------|------|------|------|
| R1 | M3 设计评审（§7 Q1-Q4 拍板）→ M3.1-M3.5 编码 | 高 | 需用户评审 | `docs/M3_MULTI_SESSION_DESIGN.md`（Draft） | **M3.1-M3.5 全部完成**（后端骨架+事件路由+前端 Map 化+exclusion set+边界）；Q1-Q4 拍板后只调常量 |
| R2 | M4：每框 continuous / follow / 引擎（per-regionId） | 中 | 依赖 R1（M3.1+） | `OCR_STRATEGY.md` L121 | 待 R1 后 |
| R3 | M5：替换译（画在原位） | 中 | 独立轨道，可与 M4 并行 | `OCR_STRATEGY.md` L122 | 待排期 |
| R4 | OCR 残留修复 M1-17~20 | 低-中 | 无 | `.pi-subagents/artifacts/outputs/fixplan/ocr.md` | **已 DONE**（全部复核已修） |
| R5 | 引擎残留 M2-03/04 + M5-04 | 低-中 | 无 | `.pi-subagents/artifacts/outputs/fixplan/engine.md` | **已 DONE**（M2-03/04 已修，M5-04 原已修） |
| R6 | M4-03 剪贴板竞态 / M4-02 DPI 定位 / M3-05 UIA 间隔 | 低-中 | 无 | fixplan/overlay.md + hook.md | **已 DONE**（M4-03 已修；M4-02/M3-05/hook low 复核已缓解/已修） |
| R7 | 处理 `feature/next-stage-optimization` 分支 S0-S4（10 提交） | 高 | 需评估 | git 分支 | OPEN |
| R8 | PR #8 合并决策（合并到 master 或继续在分支推进） | 高 | 需用户决策 | git/PR | 待决策 |

---

## 2. R1 — M3 设计评审（最高优先，卡住 M4）

**依据：** `docs/M3_MULTI_SESSION_DESIGN.md`（352 行，Draft，2026-08-01）

§7 开放问题（需用户拍板）：
- **Q1.** `MAX_REGIONS` 上限：建议 8（snow-shot 对齐，内存 240-480MB）；备选 12（PinWindowManager::MAX_POOL_SIZE）。是否 settings 可调？
- **Q2.** 新建 region 默认显示模式：建议沿用 `translated`（与单框一致）。是否 settings 可配？
- **Q3.** continuous per-region 独立开关 vs 加全局 pause-all：倾向 per-region + 可选全局暂停。
- **Q4.** 翻译缓存是否跨 region 共享：建议 deferred 到 M4（M3 非目标）。

批准后的编码阶段（M3.1-M3.5，见设计 §5）：
- **M3.1 ✅ 已完成（2026-08-01）**：`commands/region_session.rs` — `RegionSessionManager`（`OnceLock<Mutex<..>>` 单例）+ `OcrRegionRect`/`RegionMode`/`RegionSessionInfo` + `region_label()` + `MAX_REGIONS=8` + 5 命令（`ocr_begin_session`/`ocr_end_session`/`ocr_region_set_mode`/`ocr_region_list` 新增；`set_ocr_region_frame_sampling` 扩展 `id: Option<String>` 缺省 default）。default 委托现有 `ocr_begin_session_hide_main`/`ocr_end_session_show_main`/`close_ocr_region_frame`，不重写 baton。9 单测。**验证：cargo check --all ✅，cargo test --lib 544 passed / 0 failed / 2 ignored ✅**。
- **M3.2 ✅ 已完成（2026-08-01）**：`ocrRegionProtocol.ts` — `regionLabel(id)` + `regionEventName(base,id)` + `REGION_EVENTS_BY_ID`（ready/text/error/mode → `-{id}` 后缀；default 保 legacy 名）+ `emitToRegionId`（`emitToRegion` 退化为 default 委托）+ `OcrRegionEvents.mode`。Rust：`emit_to_region(app,id,event,payload)` + `ocr_region_set_mode` 现在 emit 模式事件。**验证：cargo ✅、vitest 173 ✅（含 protocol 7 条路由测试）、tsc ✅**。
- **M3.3 ✅ 已完成（2026-08-01）**：前端会话状态 Map 化。
  - `OcrRegionFrame`：读 URL `regionId`，按 id 订阅 BE→FE 事件（`REGION_EVENTS_BY_ID` / `ev(base)`），frame→main 事件保持基础名 + payload 带 `regionId`（main 按 payload.regionId 路由，default 路径字节级不变）。
  - `App.tsx`：`?window=ocr-region-frame&regionId={id}` → `<OcrRegionFrame regionId={id} />`。
  - `OcrScreenshotTranslator`：**完整重构** 762 行 — 140 处单会话 ref → `Map<regionId, RegionState>`（每 region 独立 continuous/follow/fingerprint/lastText/lang/pending/busy/sessionId）；`captureAndTranslate(regionId, region, preCapturedImage)`；全部 frame→main 监听按 payload.regionId 路由；continuous 循环与 `continuous` state 绑定 default region（per-region loop 属 M3.4）；`windowBinding` 全局单例 + `followRegionIdRef` 路由。
  - Rust：`set_ocr_region_frame_visible`/`set_ocr_region_frame_click_through` 扩展 `id: Option<String>`（与 sampling 一致）。
  - **验证：tsc ✅、vitest 173 ✅、cargo test --lib 544 ✅、eslint 0 errors ✅**。
- **M3.4 ✅ 已完成（2026-08-01）**：`set_all_regions_exclude_from_capture(app, exclude)`（枚举所有 active region HWND，逐个 `WDA_EXCLUDEFROMCAPTURE`，I1 多框版）；`set_ocr_region_frame_sampling` 非 default id 路由到全局排除集，default 保持单窗字节级行为；前端 continuous 循环泛化为 per-region（每 tick 扫描所有 region，`regionsVersion` state 驱动存活门控）。**验证：cargo ✅、548 lib tests（全量独立进程确认）✅、vitest 173 ✅**。
- **M3.5 ✅ 已完成（2026-08-01）**：多框边界 — 后端补 4 单测：关一个不影响其他（B5 修复验证）/关全部清空 / default+regular 共存 / 上限占满后释放槽位可重建；前端 selector 启动时暂停所有 region continuous（设计 §6 selector/active-region 互斥）。**验证：region_session 13 单测 ✅、全量 548 passed / 0 failed ✅、vitest 173 ✅、tsc ✅、eslint 0 errors ✅**。
- **M3 全部阶段完成（M3.1-M3.5）**。剩余：Q1-Q4 决策可只调常量；多框 window 创建路径（`ocr_begin_session` 非 default 目前只注册 session，未建窗）待 M4 per-region 引擎/continuous/follow 时接线。

关键约束（编码时必守，见设计 §4）：
- I1：采样前对所有 active region HWND 设 `WDA_EXCLUDEFROMCAPTURE`
- I6：新 region 窗标题保持 `"OCR Region"`（`hwnd_from_point` 标题黑名单依赖）
- I7：emit 前按 region 独立过 `last_text`(0.92) / `last_image_fp` 门闩
- feature flag `multi-region`（默认 off）保证回滚

---

## 3. R4 — OCR 残留修复（M1-17~20）

**依据：** `.pi-subagents/artifacts/outputs/fixplan/ocr.md`（READ ONLY 扫描）

**现状复核（2026-08-01，PR #8 分支）：全部已 DONE，fixplan 已过时。**

| ID | 严重度 | 现状 | 证据 |
|----|--------|------|------|
| M1-17 | medium | **已修** | `OcrScreenshotTranslator.tsx` L243-277：GDI 优先 + snapshot crop 兜底（含 `screenX/Y` 坐标换算与 cache），非仅 GDI |
| M1-18 | low | **已修** | 前端 L1027-1036 处理 `info.screenX/screenY`（负坐标）到 screen 坐标；Rust `virtual_screen_info` 已用 `SM_XVIRTUALSCREEN` |
| M1-19 | low | **已修** | `OcrMonitor.tsx` / `useOcrMonitor.ts` 已删除（Test-Path False） |
| M1-20 | low | **已修** | `OcrRegionFrame.tsx` 已用共享常量 `OCR_TOOLBAR_HEIGHT_CSS`/`OCR_MIN_FRAME_WIDTH_CSS`（L29-30, 79-80） |
| emitTo | OK | 无残留 | — |

---

## 4. R5 — 引擎残留修复（M2-03/04 + M5-04）

**依据：** `.pi-subagents/artifacts/outputs/fixplan/engine.md`

| ID | 严重度 | 位置 | 问题 | 最小修 |
|----|--------|------|------|--------|
| M5-04 | medium | `src/pages/settings/OcrSettings.tsx:497` | `|| 2000` 已修复为 `?? OCR_WATCH_INTERVAL_DEFAULT_MS` | **已 DONE** |
| M2-03 | medium/high | `src/types/index.ts`、`src-tauri/src/models/translation.rs`、`engine/mod.rs`、`services/translation.rs`、`translateStore.ts` | 引擎失败细节到不了 UI：`TranslateResponse` 无 `errors[]`，Router 丢 `Err` 只 log | 扩 `TranslateResponse` 加 `errors?: string[]`；Router 收集失败消息；`translateStore` 拼接进 error |
| M2-04 | low/WARN | `services/translation.rs:1203/1223` `translate_batch_core` | `Err(e)` → `String::new()`，失败行与空译文不可分 | batch 返回错误字段，或保留 `""` + 文档 WARN + 上层不写回空译文 |

相关（已 DONE 但注意）：M2-01 空 results 显式 error、M5-02 `INITIAL_CONFIG.ocrInterval: 2000`、M5-03 双旋钮已文档化。

---

## 5. R6 — 剪贴板竞态 / DPI / UIA 间隔

**依据：** `.pi-subagents/artifacts/outputs/fixplan/overlay.md` + `hook.md`

| ID | 严重度 | 位置 | 问题 | 状态 |
|----|--------|------|------|------|
| M4-03 | P1 | `platform/windows.rs` `replace_text_via_clipboard` + `hook_monitor.rs` `read_clipboard_text` + `selection/clipboard.rs` `get_clipboard_selection_win` | replace 与剪贴板监听/用户复制并发 → 无 mutex，OpenClipboard 失败/误触发翻译 | **已修**：`clipboard_dedupe::clipboard_lock()`（`OnceLock<Mutex<()>>`）串行化所有 OpenClipboard 区间；3 个调用点持锁 |
| M4-02 | P1/WARN | `App.tsx` L287-291 回退路径 | cursor+20 overlay 回退定位 | **已缓解**：`get_cursor_position` 返回物理像素，`window_manager.rs:38` 确认 overlay 期望物理像素，positioner 内部做 DPI 换算 → 坐标类型一致，cursor+20 是合理视觉偏移。文档化为已知限制 |
| M3-05 | info | UIA 默认 500ms | UIA 间隔 | **已 DONE**：`config.rs` `uia_interval_ms`（默认 500，L783-784）+ `hook_cmd.rs:43` 接线 + 测试（L1242） |
| M3-04 | info | 注入 E2E | WONTFIX 本轮 | 文档化 |
| hook low | low | `HookMonitor.tsx` L683/688/706 | `dllAvailable === null` 时按钮短暂可点 | **已修**：全部用 `dllAvailable !== true` 禁用，null 也禁用 |
| hook low | low | `hook_inject_cmd::hook_inject` | 无 `dll_available` 前置短路 | 后端依赖 `find_hook_dll` 失败返回错误；UI 已禁用（可接受，不做服务端预检） |

**验证（2026-08-01）：** `cargo check --all` 通过；`cargo test --lib` 535 passed / 0 failed / 2 ignored（535+2=537 总数，与基线一致）；`npm run check`（tsc --noEmit）通过。

---

## 6. R7 — `feature/next-stage-optimization` 分支（13 提交，未合并）

**git 状态：** `master..feature/next-stage-optimization` = 10 提交，`merge-base --is-ancestor` 返回 not merged。

| 提交 | 主题 | 风险/注意 |
|------|------|-----------|
| `e4bcc62` | S0+S1 正确性修复 + 死代码清理（28 文件，capture.rs 大改 579 行） | **与 master 大量冲突风险**（capture.rs / window.rs 大改） |
| `cc4651f` | S2 资源管理与可靠性（12 文件） | overlay/window_manager.rs 大改 |
| `1f0b725` | S3-backend Rust 一致性 + SAFETY 注释 | hover_pick.rs 改动 |
| `f30c9ed` | S3-frontend TS 一致性 + safeInvoke 迁移（30 文件） | 与 master 前端冲突风险 |
| `a9468cc` | S3-followup 补齐 | migrations + DictionarySearch |
| `4548d4e` | i18n enginesMeta 死数据移除 + OCR 下拉 i18n | 与 master i18n 可能有冲突 |
| `7836c4f` | docs v2 健康度审计计划书 | 纯文档 |
| `3964c2c` | S4 工程化加固（5 项：eslint/CI/release） | CI 改动 |
| `7c9be02` | i18n 补全 en/ja/ko 四语 | i18n 大改 |
| `00ece6c` + `2a1cb04` | build 修复：comctl32 v6 manifest + doctest | **已被 `5f540cb` 以相同思路并入 PR #8 分支**，需核对重复 |

**注意：** `b4e79f6`（修 v2 health audit 引入的 5 个测试回归）改动文件与我们的 `f01da64` 完全相同（alignment.rs / patch_validator.rs / pdf.rs / hover_pick.rs）。**需确认两者修复内容是否一致或冲突**——若 S 分支此提交已被 f01da64 覆盖，则合并 S 分支时此提交可跳过或自动合并。

**评估结论（2026-08-01 已执行）：**

S 分支 = **与 PR #8 平行的独立轨道**（merge-base = `33d5241` = master tip）。两条轨道都从 master 分叉、各自改动 capture.rs/window.rs 等核心文件（S 删死代码，PR #8 加功能），**直接 merge 会大量语义冲突**。13 个提交分三类：

| 类别 | 提交 | 价值 | 处理建议 |
|------|------|------|----------|
| **A. 与 PR #8 重复** | `2a1cb04`（comctl32 manifest）、`00ece6c`（doctest/build）、`b4e79f6`（5 测试修复） | 已被 `5f540cb`/`f01da64` 覆盖（同根因、思路一致） | **跳过**，不重复合并 |
| **B. 高价值独立内容** | `e4bcc62` S0+S1（死代码清理：删 `capture_full_screen`/`system_ocr`/`detect_text_regions` + 统一 WinRT OCR 到 `ocr_engine::run_winrt_ocr_raw` + spawn_blocking 卸载）、`cc4651f` S2（资源管理 12 项）、`1f0b725` S3-backend、`f30c9ed` S3-frontend、`a9468cc` S3-followup | 死代码清理（3 个未注册命令）+ 一致性 + SAFETY | **按需 cherry-pick**，但 capture.rs 与 PR #8 语义冲突，需逐个审 |
| **C. 低冲突独立内容** | `7836c4f`（docs）、`3964c2c` S4（eslint/CI/release，6 独立文件）、`7c9be02`（i18n 四语）、`4548d4e`（OCR 下拉 i18n）、`0e2c2b4`（extension Tier 1 B1-B8） | i18n/CI/extension 是独立文件面 | **低风险合入**，可单独 cherry-pick |

**关键事实：**
- `detect_text_regions` / `capture_full_screen` / `system_ocr`（非 detailed）在 lib.rs **均未注册为命令**、无调用方 → S 分支删除是纯死代码清理，无风险
- `system_ocr_detailed` 仍注册保留（S 分支未删）
- S 分支与 PR #8 在 capture.rs 的 diff：S 删 579 行，PR #8 加 365 行 → 需逐一比对避免删掉 PR #8 新功能

**R7.1 执行记录（2026-08-01）：**
- ✅ cherry-pick `7836c4f`（docs v2 审计计划书）、`3964c2c`（S4 工程化：eslint/CI/release）、`7c9be02`（i18n en/ja/ko 四语）、`4548d4e`（enginesMeta nameZh 移除 + OCR 下拉 i18n，OcrRegionFrame 自动合并保留 M3.3 regionId 逻辑）
- ✅ 补 1 个 lint 修复提交（translateStore.ts optional chain，S4 eslint 严格规则暴露）
- ⛔ **`0e2c2b4`（extension Tier1 B1-B8）跳过**：与 HEAD extension 架构语义冲突——S 分支是旧代码上完全重写 page-translator.js（586 行），HEAD 已演进到 Tier4（1074 行含 specialRules/Main container）。强行合并会覆盖 HEAD 功能。需单独架构评审。
- **验证：tsc ✅、vitest 311 passed（src 173 + extension 138）✅、eslint 0 errors ✅**

**决策建议（R7.1 → R7.3 顺序）：**
1. **R7.1 C 类** ✅ 已完成（除 `0e2c2b4` extension 因架构冲突跳过）
2. **R7.2** cherry-pick B 类中与 capture.rs 无关的项（S2 资源管理、S3-backend 非 capture 部分）
3. **R7.3** capture.rs 相关（S0+S1 死代码清理 + WinRT OCR 重构）需人工比对 PR #8 新功能后再定；建议在 PR #8 合并后做

---

## 7. 执行顺序建议

1. ~~**R4/R5/R6 快速修**~~ ✅ **已全部完成**（2026-08-01，见各节，测试 535 passed）
2. **R1 M3.1-M3.3** ✅ **已完成**（544 lib tests + 173 FE tests）；**M3.4 可继续**（后端 exclusion set，风险中低）或等 Q1-Q4 拍板
3. **R7.1 cherry-pick C 类**（docs/S4/i18n/extension）——低风险、独立文件面
4. **R7.2 cherry-pick B 类非 capture 项**（S2 资源管理、S3-backend 非 capture 部分）
5. **R8 PR #8 合并** + **R7.3**（capture.rs 相关，需 PR #8 合并后比对）
6. **R2/R3**（M4/M5 开发，M4 依赖 R1）

---

## 8. 相关文档

- `docs/M3_MULTI_SESSION_DESIGN.md` — M3 设计（R1 依据）
- `docs/OCR_STRATEGY.md` — M0-M5 阶段表、multi-frame 边界（R2/R3 依据）
- `docs/OCR_INVARIANTS.md` — I1-I7 不变量
- `.pi-subagents/artifacts/outputs/fixplan/ocr.md` — R4 依据
- `.pi-subagents/artifacts/outputs/fixplan/engine.md` — R5 依据
- `.pi-subagents/artifacts/outputs/fixplan/overlay.md` / `hook.md` — R6 依据
- `docs/CURRENT_FOCUS.md` — 现有工作清单（划词/hook 分区）
