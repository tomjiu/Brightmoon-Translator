# M3 Multi-Session Design — 多 live OCR region 架构

**Status:** Draft — 待用户评审（开放问题未决，批准后再进 M3.1 编码）
**Date:** 2026-08-01
**Depends on:** M0/M1/M2 已冻结的 OCR session lifecycle — 本设计**不重写**现有 baton，仅在其外加一层 region 编排
**Unblocks:** M4（per-region continuous/follow/engine）、M5（替换译）

> 来源约束（`OCR_STRATEGY.md` § Screenshot-app multi-frame）：
> 「多框 = 新一层 Present/Capture 编排（regionId 路由），不是在现 baton 上硬叠第二个 HWND。」
> 「动 M3 前必须先过 multi-session 设计，避免拆坏冻结的 session lifecycle。」

---

## 0. 目标与非目标

### 目标
- 支持**多个 live region frame** 同时存在（截图软件手感：框选 → 浮框 → 再框选 → 第二个浮框，互不抢 session）。
- 每个 region 独立持有状态：显示模式（image/source/translated）、continuous watch、follow-HWND、最近图像指纹、最近文本。
- **不破坏** M0/M1/M2 单框路径：现有热键流程行为不变。
- 捕获卫生：任意 region 采样时，**所有** region frame 的 chrome 都被排除出截图（I1 的多框版本）。
- 可逐 region 关闭、可一键关闭全部；关闭最后一个 region 才恢复主窗。

### 非目标（不在 M3 做）
- ❌ M4：per-region 引擎选择 / 翻译缓存共享策略（M3 只保证 regionId 路由到位，引擎仍全局）。
- ❌ M5：替换译（画在原位）—— 独立轨道。
- ❌ 自由形状 / 多矩形选区（仍是单矩形）。
- ❌ 跨屏拖拽 region（先稳同屏多框）。
- ❌ 重写 selector：仍单例，一次框选生成一个新 region。
- ❌ region 间内容引用 / 翻译记忆共享（每个 region 独立 state）。

---

## 1. 现状架构盘点（single-session baton）

当前 OCR 是**全局单会话**：一条 baton 在 selector → region-frame 之间传递，主窗在会话期间隐藏。关键落点：

### 1.1 baton 流程（单框）

```
热键 (trigger-ocr-screenshot)
  → App.tsx: ocrLaunchNonce++  → OcrScreenshotTranslator 启动
  → window.rs: create_ocr_screenshot_selector        (窗 label "ocr-screenshot")
  → window.rs: ocr_begin_session_hide_main           (main 设 WDA_EXCLUDEFROMCAPTURE + hide)
  → capture.rs: prepare_screenshot_snapshot          (GDI 抓虚拟屏 → 缓存 PNG)
  → 用户在 selector 拖框
  → capture.rs: crop_screenshot_snapshot             (从缓存裁剪 → base64)
  → window.rs: create_ocr_region_frame               (窗 label 写死 "ocr-region-frame")
  → window.rs: close_ocr_screenshot_selector         (销毁 selector，region frame 接力)
  → capture.rs: system_ocr_detailed / offline_ocr / youdao_ocr  → 翻译
  → ocrRegionProtocol.ts: emitToRegion("ocr-region-update-data", ...)   (emitTo("ocr-region-frame", ...))
  → OcrRegionFrame.tsx (region 窗) 显示
  → 用户关闭 → "ocr-region-close" 事件
  → window.rs: close_ocr_region_frame + ocr_end_session_show_main      (恢复 main)
```

### 1.2 关键符号（现状，单框）

| 角色 | 文件 / 符号 | 现状 |
|------|-------------|------|
| 单一 region 窗 label | `window.rs` `create_ocr_region_frame` / `preload_ocr_region_frame` / `close_ocr_region_frame` / `move_ocr_region_frame` / `set_ocr_region_frame_visible` / `set_ocr_region_frame_sampling` / `set_ocr_region_frame_click_through` | 写死 `"ocr-region-frame"` |
| 会话开关 | `window.rs` `ocr_begin_session_hide_main` / `ocr_end_session_show_main` | 全局 main 显隐，无 regionId |
| 采样排除 | `window.rs` `set_ocr_region_frame_sampling(app, bool)` + `set_window_exclude_from_capture_inner` | 仅排除**一个** HWND |
| OCR / 抓图 | `capture.rs` `prepare_screenshot_snapshot` / `crop_screenshot_snapshot` / `system_ocr_detailed` / `offline_ocr` / `youdao_ocr` | 无 region 概念，结果直推单 frame |
| 事件协议 | `src/services/ocrRegionProtocol.ts` `OCR_REGION_LABEL='ocr-region-frame'`，`emitToRegion`→`emitTo('ocr-region-frame', ...)` | 单 label 广播，无 regionId 过滤 |
| 主窗会话状态 | `src/components/OcrScreenshotTranslator.tsx`（main 窗内） | 单份 state：一个 continuous、一个 follow hwnd、一个 `lastImageFpRef`、一个 `pendingRegion` |
| region 窗 UI | `src/components/OcrRegionFrame.tsx`（经 `App.tsx` `?window=ocr-region-frame` 路由挂载） | 监听无 id 的事件 |
| M2 钉图池（参照） | `src-tauri/src/overlay/pin_manager.rs` `PinWindowManager` | retain pool，`pin-{id}` 标签，`MAX_POOL_SIZE=12`，`OnceLock<Mutex<..>>` 单例 |

### 1.3 >1 region 时哪里会坏

| # | 现状 | 多框障碍 |
|---|------|---------|
| B1 | `create_ocr_region_frame` 用 `app.get_webview_window("ocr-region-frame")` 复用唯一窗 | 第二次框选只会**复用**第一个窗，无法并存第二框 |
| B2 | `emitToRegion` → `emitTo('ocr-region-frame', ev)` | 广播到单一 label，无法路由到指定 region；多框会串扰或只有一框收到 |
| B3 | `set_ocr_region_frame_sampling` 只排除一个 HWND | region A 采样时，region B/C 的 frame 会**入镜** A 的截图（自吃 overlay，I1 多框版） |
| B4 | `OcrScreenshotTranslator` 单份会话 state | 第二框覆盖第一框的 continuous / follow / fingerprint / pendingRegion |
| B5 | `ocr_end_session_show_main` 是全局 main 显隐 | 关闭任意一个 region 就恢复 main，**误杀**其余 live region 的会话 |
| B6 | `close_ocr_region_frame` 关单一 label | 无法只关一个 region |
| B7 | `hwnd_from_point`（`capture.rs`）标题过滤 `"OCR Region"` 等 | 多框窗都叫 `"OCR Region"`，过滤本身仍生效（见 I6 分析），但 follow 绑定是全局单份（B4） |

---

## 2. 总体设计

### 2.1 架构总览

```
┌─────────────────────────────────────────────────────────────┐
│  Selector（单例，不改）                                       │
│  一次框选 → 生成新 regionId → 交给 RegionSessionManager       │
│  不持 state；只采集几何 + 快照                                │
└──────────────────┬──────────────────────────────────────────┘
                   │ 框选完成 → ocr_begin_session(regionId, rect, snapshot)
                   ▼
┌─────────────────────────────────────────────────────────────┐
│  RegionSessionManager（新增，镜像 M2 PinWindowManager）       │
│  - Map<RegionId, RegionSession>                              │
│  - 创建/销毁/列举 region frame window（label ocr-region-frame-{id}）│
│  - 维护全局 capture exclusion set（所有 region HWND）         │
│  - main 显隐与"是否还有 live region"挂钩（仅最后一个关才恢复）│
└──────────────────┬──────────────────────────────────────────┘
                   │ per-region 事件：ocr-region-{evt}-{id}
                   ▼
┌─────────────────────────────────────────────────────────────┐
│  RegionFrame window（per-regionId）                           │
│  - label: ocr-region-frame-{id}                              │
│  - OcrRegionFrame 读 URL regionId，事件按 id 过滤            │
│  - 独立 continuous tick / follow bind / OCR cancel token     │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 RegionSessionManager（Rust，新增）

镜像 `src-tauri/src/overlay/pin_manager.rs` 的 `PinWindowManager` 模式：全局 `OnceLock<Mutex<RegionSessionManager>>` 单例、label-per-id、`active_count` / `list_active` 列举。区别在于管的是 **live region**（可 re-OCR / continuous / follow / 可拖可调几何），而非静态钉图卡：

| 维度 | PinWindowManager (M2) | RegionSessionManager (M3) |
|------|----------------------|---------------------------|
| 内容 | 静态截图 + 译文，不再 OCR | live region，可 re-OCR / continuous / follow |
| 窗 label | `pin-{id}` | `ocr-region-frame-{id}` |
| 几何 | min/max 限死，不可拖调 | 可拖可调（复用 `move_ocr_region_frame` 几何） |
| 生命周期 | dismiss → 标记 slot 空闲复用 | 关闭 → 销毁窗 + 移除 session（不复用，live 状态不复用） |
| Capture 排除 | 不参与 | 必须（live 采样，I1） |
| 池策略 | retain pool（隐藏复用） | 直建直销（每窗带独立 webview state） |

**RegionSession 字段（接口契约，非实现）：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `region_id` | `String` | 见 §2.3 |
| `label` | `String` | `"ocr-region-frame-{id}"`（default 退化为 `"ocr-region-frame"`） |
| `rect` | 屏幕物理坐标 | 复用现有 `OcrRegionRect` 语义 |
| `mode` | `"image" \| "source" \| "translated"` | 显示模式 |
| `continuous` | `bool` | per-region 监视开关，默认 false（I1） |
| `follow_hwnd` | `Option<isize>` | per-region follow 绑定（I6） |
| `last_image_fp` | `Option<String>` | 图像指纹（I7 跳过门闩） |
| `last_text` | `Option<String>` | 最近 OCR 文本（I7 相似门闩） |
| `sampling` | `bool` | 该 region 是否正处采样排除态 |
| `created_at` | `Instant` | 上限/调试用 |

### 2.3 RegionId

- **类型：`String`**。新 region 用单调递增字符串（`"1"`、`"2"`、…）—— 不用 UUID，便于 URL hash 透传、日志可读、与前端 `Map<regionId, RegionState>` 键一致。
- **保留 id `"default"`**：legacy 单框路径的 shim。其窗口 label 退化为裸 `"ocr-region-frame"`（不加后缀），使 M0/M1/M2 所有现有命令**零改动**继续工作（见 §3）。
- **上限 `MAX_REGIONS`**：值见开放问题 Q1（建议 8）。超限拒绝创建并 toast 提示先关旧 region。

### 2.4 窗口 label 方案

| 路径 | label | URL |
|------|-------|-----|
| 新 region（id=`"3"`） | `ocr-region-frame-3` | `index.html?window=ocr-region-frame&v=ocr2&regionId=3` |
| legacy 单框（id=`"default"`） | `ocr-region-frame`（裸，不变） | `index.html?window=ocr-region-frame&v=ocr2`（不变） |

label 函数：`id == "default"` → `"ocr-region-frame"`；否则 `"ocr-region-frame-{id}"`。`preload_ocr_region_frame` 预加载的仍是 default 那个裸 label 窗（冷启动加速不变）。

### 2.5 事件协议（per-regionId）

新 region 使用带 `-{id}` 后缀的事件，payload 额外携带 `regionId` 字段（双保险，便于单 listener 分发）：

| 新事件（multi-region） | 方向 | 对应现状事件（`ocrRegionProtocol.ts`） | Payload 要点 |
|------------------------|------|----------------------------------------|--------------|
| `ocr-region-ready-{id}` | BE→FE | `ocr-region-frame-ready` | region 窗就绪 |
| `ocr-region-text-{id}` | BE→FE | `ocr-region-update-data` | screenshot/sourceText/translatedText/ocrLines/imageWidth/imageHeight（I5） |
| `ocr-region-error-{id}` | BE→FE | `ocr-region-error` | message（I4：错误隔离到单 region，不关框） |
| `ocr-region-mode-{id}` | BE→FE | （新增） | mode: image/source/translated |

legacy default 路径**继续用现有无后缀事件名**（`ocr-region-frame-ready` / `ocr-region-update-data` / `ocr-region-error` / …），`OcrRegionFrame.tsx` 现有 listener 零改动。新 region 的 listener 按 URL `regionId` 订阅对应后缀事件，忽略其他 id。

> 连续/跟随等现有 frame→main 事件（`ocr-region-close` / `ocr-region-refresh` / `ocr-region-continuous` / `ocr-region-follow` / `ocr-region-position-changed` / `ocr-region-size-changed`）在 multi-region 路径同样加 `-{id}` 后缀 + payload.regionId；default 路径不变。

### 2.6 Capture 排除（全局排除集，I1 多框版）

**问题（B3）：** 单框 `set_ocr_region_frame_sampling` 只排除一个 HWND；多框时 region A 采样会吃进 B/C 的 frame。

**方案：** 采样前枚举**所有** active region 的 HWND，逐个 `WDA_EXCLUDEFROMCAPTURE`，采样后恢复。

```
continuous tick for region X:
  1. set_all_regions_exclude_from_capture(true)   // 排除所有 region frame HWND
  2. capture region X 的 rect                       // GDI/DXGI 跳过所有 region chrome
  3. set_all_regions_exclude_from_capture(false)   // 恢复
  4. OCR + 翻译 + emit ocr-region-text-{X}
```

复用 `window.rs` 已有的 `set_window_exclude_from_capture_inner(window, bool)`（已封装 `SetWindowDisplayAffinity`），只需新增"枚举所有 region label → 逐个调用"的编排函数。

**优化（开放）：** `WDA_EXCLUDEFROMCAPTURE` 是 Win32 µs 级调用，每 tick 设 N 个 HWND 仍廉价；亦可让"任一 region 处于 continuous 时，所有 region frame 常驻排除态"，只在 continuous 全关时清——零闪烁且省重复设置。老系统（Win10 2004 前）无此 affinity 时回退 hide 路径 + 长 settle（40–50ms），与现 `set_ocr_region_frame_sampling` 回退逻辑一致。

### 2.7 命令（per-region + default shim）

新增 5 个 per-region 命令。`id == "default"` 时**委托**给现有单框命令（不重写 baton）：

| 新命令 | id=`"default"` 时 | 其他 id 时 |
|--------|-------------------|------------|
| `ocr_begin_session(id, rect?, snapshot?)` | 调用现有 `ocr_begin_session_hide_main` 逻辑 + 注册 default session | 注册新 session；main 显隐由"是否首个 live region"决定（首个 region 开 → hide main） |
| `ocr_end_session(id)` | 若无其他 live region → 调用 `ocr_end_session_show_main`；移除 default session | 移除该 session；**仅当最后一个 region 关闭**才 `ocr_end_session_show_main` 恢复 main（修 B5） |
| `set_ocr_region_frame_sampling(id, sampling)` | 委托现有 `set_ocr_region_frame_sampling(app, sampling)` | 对 `ocr-region-frame-{id}` HWND 设 affinity；multi 下走全局排除集（§2.6） |
| `ocr_region_set_mode(id, mode)` | 对 default frame emit `ocr-region-mode`（或现有等价） | emit `ocr-region-mode-{id}` |
| `ocr_region_list()` | — | 返回所有 active `RegionSessionInfo`（含 default） |

> **命名冲突处理：** 现有 `set_ocr_region_frame_sampling(app, bool)` 与新 `set_ocr_region_frame_sampling(id, bool)` 同名异参，Tauri 按名派发不可共存。决策：**扩展现有命令签名**加 `id: Option<String>`（缺省=`"default"`），旧调用点零改动；其余 4 个是新名，无冲突。

### 2.8 前端

- `OcrScreenshotTranslator.tsx`（main 窗）：单份 session state → `Map<regionId, RegionState>`。每条 `RegionState` 独立持有 continuous / follow / lastImageFp / lastText / OCR cancel token。
- `OcrRegionFrame.tsx`（region 窗）：mount 时读 URL `regionId`；事件 listener 按 `regionId` 过滤（订阅 `ocr-region-text-{regionId}` 等），忽略其他 id。
- `App.tsx` 路由：`?window=ocr-region-frame&regionId={id}` → `<OcrRegionFrame regionId={id} />`；default 无 `regionId` 参数时退化为现有行为。
- `ocrRegionProtocol.ts`：`OCR_REGION_LABEL` 由常量改为按 id 解析的 label 函数；新增 `emitToRegionId(id, event, payload)` 助手；旧 `emitToRegion` 保留（指向 default）。

---

## 3. 向后兼容（M0/M1/M2 单框路径不破坏）

**原则：default shim 让现有热键流程完全无感。**

1. 现有热键 `trigger-ocr-screenshot` → `OcrScreenshotTranslator` → `create_ocr_screenshot_selector` 流程**不改**。
2. 框选完成时，shim 以 `id="default"` 调 `ocr_begin_session("default", rect, snapshot)` → 委托现有 `ocr_begin_session_hide_main` + 注册 default session。
3. default 的窗 label 仍是裸 `"ocr-region-frame"`，`create_ocr_region_frame` / `move_ocr_region_frame` / `close_ocr_region_frame` / `preload_ocr_region_frame` / `set_ocr_region_frame_click_through` 全部不动。
4. default 继续用 `ocrRegionProtocol.ts` 现有无后缀事件，`OcrRegionFrame.tsx` 现有 listener 不动。
5. 关闭 default → `ocr_end_session("default")` → 无其他 live region → `ocr_end_session_show_main`。

**结果：** M0/M1/M2 行为字节级一致；multi-region 是**纯增量**路径。冻结的 `ocr_begin/end_session_*` baton 逻辑不被重写，仅被 default 路径**调用**。

---

## 4. 不变量保持表（I1–I7）

`OCR_INVARIANTS.md` 的 I1–I7 不可回退。M3 对每条的处理：

| 不变量 | 现状要求 | M3 保持方式 |
|--------|---------|-------------|
| **I1** continuous 默认 OFF；采样前 frame 不入镜（`WDA_EXCLUDEFROMCAPTURE` 优先，hide 回退） | 每 region `continuous` 默认 false；采样前对**所有** region HWND 设全局排除集（§2.6），而非仅采样那一个——多框版强化 | 强化：多框互不污染 |
| **I2** 工具栏高常量 32（`OCR_TOOLBAR_CSS_PX`） | per-region 窗复用 `create_ocr_region_frame` / `move_ocr_region_frame` 几何计算，工具栏高不变 | 不变 |
| **I3** 最小窗宽 ≥380（`OCR_MIN_FRAME_CSS_W`） | per-region 窗同样 `set_min_size` 强制 | 不变 |
| **I4** 空 OCR / 翻译失败不关框 | per-region：错误 emit `ocr-region-error-{id}`，仅该 frame 显示错误 + retry，不关框、不影响其他 region | per-region 隔离 |
| **I5** 行布局用图像自然尺寸（payload 带 `imageWidth/Height`） | `ocr-region-text-{id}` payload 仍带 `imageWidth/imageHeight` | 不变 |
| **I6** follow 绑内容窗而非 OCR chrome；`hwnd_from_point` 标题过滤 | per-region `follow_hwnd` 独立绑定；`hwnd_from_point`（`capture.rs`）已按标题 `"OCR Region"` 过滤，**多框窗都叫 `"OCR Region"` 故过滤天然覆盖**（前提：新 region 窗 `.title("OCR Region")` 不改）；click-through per-region | per-region 绑定，标题过滤无需改 |
| **I7** 相似度 ≥0.92 跳过翻译；图像指纹匹配跳过 OCR+翻译 | per-region `last_text` + `last_image_fp`，门闩在 emit 前按 region 独立判定 | per-region 独立 |

**I6 注意点（实现时必查）：** `capture.rs` `hwnd_from_point` 的标题黑名单含 `"OCR Region"`；只要新 region 窗保持该标题，多框下 follow 不会误绑到任意 region frame。若日后改 region 窗标题，须同步更新该黑名单。

---

## 5. 分阶段实施计划（设计批准后的编码阶段）

> 顺序设计为**先把后端管子做稳、再动前端、最后攻多框捕获卫生**，每阶段可独立 smoke、可回滚。

| 阶段 | 内容 | 产出 | 依赖 |
|------|------|------|------|
| **M3.1** | `RegionSessionManager` 骨架 + 5 个命令（default 委托 shim）+ 单测；**不改 UI、不改现有命令** | `region_session.rs`（或并入 `window.rs`）+ `Map<RegionId, RegionSession>` + 命令注册（`lib.rs` invoke_handler）+ 单测覆盖 create/close/list/default 委托/上限拒绝 | 本设计批准 |
| **M3.2** | 窗 label 方案 `ocr-region-frame-{id}` + 事件 regionId 路由 | label 函数（default 裸 label）+ `ocr-region-ready/text/error/mode-{id}` 事件 + `ocrRegionProtocol.ts` `emitToRegionId` | M3.1 |
| **M3.3** | 前端 `Map<regionId, RegionState>` + 多框 `OcrRegionFrame` 挂载 | `OcrScreenshotTranslator` 拆 state 为 Map；`OcrRegionFrame` 读 URL `regionId` 并按 id 过滤事件；`App.tsx` 路由带 `regionId` | M3.2 |
| **M3.4** | 全局 capture exclusion set（枚举所有 region HWND） | `set_all_regions_exclude_from_capture` 编排 + 接入 continuous tick；`set_ocr_region_frame_sampling` 扩展 `id` 参数 | M3.3 |
| **M3.5** | 多框 smoke + 边界 | 关一个 region / 关全部 / 上限拒绝 / selector 与 active region 互斥（框选时暂停所有 continuous） | M3.4 |

每阶段完成须过 `OCR_SMOKE.md` 单框路径（保证 default 未坏）+ 该阶段新增多框 smoke。

---

## 6. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| **多 webview 内存膨胀**（每 frame ~30–60MB） | 8 框可达 240–480MB | `MAX_REGIONS` 硬上限（Q1，建议 8）；超限 toast 拒绝；`ocr_region_list` 暴露占用供 UI 提示 |
| **事件风暴**（N region × continuous tick） | CPU/IO 飙升、WinRT OCR 引擎争用 | per-region `imageFingerprint` 跳过相同帧（I7）；OCR 并发信号量（建议 cap=2，同 region 新 OCR cancel 旧）；连续 tick 自适应间隔（已有） |
| **capture exclusion 性能**（每 tick 设 N 个 HWND affinity） | 通常可忽略（µs 级），但极端 N+高频 tick 下累积 | affinity 常驻方案（任一 region continuous 时所有 region frame 保持排除态，全关才清）；或每 tick 设一次 |
| **冻结 lifecycle 回归**（动 `ocr_begin/end_session_*`） | 单框路径坏掉（最高风险） | M3.1 不重写现有命令，default shim 仅**调用**它们；multi-region 纯增量；feature flag `multi-region`（默认 off）可一键回退到 M0；每阶段过单框 smoke |
| **regionId 路由串扰** | 事件发错 frame、状态错乱 | 事件名带 `-{id}` 后缀 + payload 双带 `regionId`；listener 按 URL `regionId` 严格过滤；M3.1 单测覆盖路由 |
| **selector 与 active region 互斥** | 框选新 region 时旧 region continuous 干扰选区 | selector 启动时 pause 所有 region continuous；selector 关闭后 resume |

### 回滚
- 全部新代码在独立模块 + feature flag `multi-region`（默认 off）。
- 出问题关 flag 即回 M0 单框；现有 `ocr_begin/end_session_*` / `create_ocr_region_frame` 等未被删除/重写。

---

## 7. 开放问题（待用户决策）

> 以下未决，列出我的建议倾向，等用户拍板后再进 M3.1。

**Q1. Region 数量上限 `MAX_REGIONS` 取多少？**
- 建议：8（与 snow-shot HotLoadPageService 上限对齐；内存预算 240–480MB）。
- 备选：12（对齐本仓 `PinWindowManager::MAX_POOL_SIZE`）。
- 待定：是否在 settings 暴露可调？

**Q2. 新建 region 的默认显示模式？**
- 现状单框默认是「译文叠字 + 底图」（即 translated 模式偏译）。
- 选项：`translated`（沿用）/ `source`（原文）/ `image`（原图）。
- 建议：沿用 `translated`，与 M0/M1 单框体验一致。
- 待定：是否让"新建 region 默认模式"可在 settings 配？

**Q3. continuous 是 per-region 独立开关，还是全局总开关？**
- 目标已写"每个 region 独立 continuous watch"——倾向 **per-region**。
- 但可加一个**全局"暂停所有监视"主开关**（不关框，只停 tick），便于框选新 region 时一键静默。
- 待定：仅 per-region？还是 per-region + 全局 pause-all？

**Q4（顺带）.** multi-region 路径下，翻译缓存是否跨 region 共享（同文本不重复翻译）？M3 不做也行（非目标），但影响 `last_text` 门闩语义，需确认是否 deferred 到 M4。

---

## 8. 接口契约汇总（仅供编码参考，非实现）

> 下述为签名/字段契约，**不含实现体**。最终命名以编码阶段 review 为准。

### Tauri 命令（新增 5 个，注册于 `src-tauri/src/lib.rs` invoke_handler）

```
ocr_begin_session(id: String, rect?: OcrRegionRect, snapshot?: String) -> Result<(), String>
ocr_end_session(id: String) -> Result<(), String>
set_ocr_region_frame_sampling(id: String, sampling: bool) -> Result<bool, String>   // 扩展现有签名
ocr_region_set_mode(id: String, mode: String) -> Result<(), String>                 // mode ∈ image|source|translated
ocr_region_list() -> Result<Vec<RegionSessionInfo>, String>
```

### 类型（`RegionSessionInfo`，序列化 camelCase）

| 字段 | 类型 |
|------|------|
| regionId | string |
| label | string |
| rect | OcrRegionRect |
| mode | string |
| continuous | bool |
| followHwnd | number \| null |
| sampling | bool |
| createdAtMs | number |

### 前端（`src/services/ocrRegionProtocol.ts` 扩展）

```
regionLabel(id: String): string          // "default" → "ocr-region-frame"；否则 "ocr-region-frame-{id}"
emitToRegionId(id, event, payload): Promise<void>
REGION_EVENTS_BY_ID = { ready:(id)=>`ocr-region-ready-${id}`, text:(id)=>`ocr-region-text-${id}`, error:(id)=>`ocr-region-error-${id}`, mode:(id)=>`ocr-region-mode-${id}` }
```

### 不变量契约（编码时必守）
- I1：采样前对所有 active region HWND 设 `WDA_EXCLUDEFROMCAPTURE`。
- I5：`ocr-region-text-{id}` payload 必带 `imageWidth/imageHeight`。
- I6：新 region 窗标题保持 `"OCR Region"`（`hwnd_from_point` 标题黑名单依赖）。
- I7：emit 前按 region 独立过 `last_text`(0.92) / `last_image_fp` 门闩。

---

## 9. 相关文档

- [OCR_STRATEGY.md](./OCR_STRATEGY.md) — M0–M5 阶段表、预留槽位、multi-frame 边界
- [OCR_INVARIANTS.md](./OCR_INVARIANTS.md) — I1–I7 不变量（本设计 §4 映射）
- [REFERENCE_OCR_CAPTURE.md](./REFERENCE_OCR_CAPTURE.md) — snow-shot / capcap / kivio 参考
- 代码落点：`src-tauri/src/commands/window.rs`（单框命令）、`src-tauri/src/commands/capture.rs`（OCR/抓图）、`src-tauri/src/overlay/pin_manager.rs`（M2 PinWindowManager 参照）、`src/services/ocrRegionProtocol.ts`（事件协议）、`src/components/OcrScreenshotTranslator.tsx` + `src/components/OcrRegionFrame.tsx`（前端 session state）

---

**Next step:** 用户评审 §7 开放问题 → 拍板后进 M3.1 编码（`RegionSessionManager` 骨架 + 5 命令 + 单测，default shim，不改 UI）。
