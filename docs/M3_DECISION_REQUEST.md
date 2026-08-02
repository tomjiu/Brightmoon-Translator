# M3 Multi-Session Design — 决策请求（R1）

**Status:** 待用户评审拍板
**Date:** 2026-08-01
**依据:** `docs/M3_MULTI_SESSION_DESIGN.md`（352 行 Draft）§7 开放问题
**拍板后:** 更新 M3 设计 Status → Approved，进 M3.1 编码

---

## Q1. Region 数量上限 `MAX_REGIONS`

| 选项 | 值 | 依据 |
|------|-----|------|
| **建议** | **8** | snow-shot HotLoadPageService 对齐；内存预算 240-480MB（每 webview 30-60MB）；够截图软件手感 |
| 备选 | 12 | 对齐本仓 `PinWindowManager::MAX_POOL_SIZE`（`pin_manager.rs:27`）；但 M2 钉图是静态卡，M3 是 live webview，内存压力更大 |
| 待定 | settings 暴露可调 | 增加配置面；建议**暂不暴露**，硬编码 8，需要时再提 |

**倾向建议：** 8，不暴露配置。

---

## Q2. 新建 region 默认显示模式

| 选项 | 依据 |
|------|------|
| **建议** | **`translated`（沿用）** | 与 M0/M1 单框体验一致（译文叠字 + 底图），用户预期零变化 |
| 备选 | `source` / `image` | 新 region 默认显示原文/原图，与现单框不同，体验断裂 |
| 待定 | settings 可配默认模式 | 建议**暂不**，保持简单 |

**倾向建议：** 沿用 `translated`。

---

## Q3. continuous 开关粒度

| 选项 | 依据 |
|------|------|
| **建议** | **per-region 独立开关 + 全局 pause-all** | 目标已写"每 region 独立 continuous"；全局暂停便于框选新 region 时一键静默，不关框只停 tick |
| 备选 | 仅 per-region | 更简单，但框选新 region 时需逐个关旧 region 的 continuous |

**倾向建议：** per-region + 可选全局"暂停所有监视"主开关。

---

## Q4. 翻译缓存跨 region 共享

| 选项 | 依据 |
|------|------|
| **建议** | **deferred 到 M4** | M3 非目标；多框共享缓存涉及缓存 key 语义与 `last_text` 门闩联动，属 M4 范畴 |
| 备选 | M3 就做 | 增加 M3 面，拖慢多框主链路 |

**倾向建议：** deferred 到 M4；M3 内每 region 独立缓存状态。

---

## 决策后的执行链

1. 你拍板 Q1-Q4（或调整倾向）→ 我更新 M3 文档 Status + 固化结论
2. 进 **M3.1**：`RegionSessionManager` 骨架 + 5 命令（default 委托 shim）+ 单测
3. M3.2 → M3.3 → M3.4 → M3.5（见设计 §5，每阶段过单框 smoke）
