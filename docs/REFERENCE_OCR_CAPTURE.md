# 截图 / OCR / 钉图 — 开源参考深读（2026-07-29）

**目的：** 在既有划词参考（Easydict / MTT / YomiNinja）之外，补充 **截图壳、OCR 框、钉图、排除自捕获、就地替换译文** 类项目。对照 Moon **单例** `ocr-region-frame`（可拖可 continuous，**不能多框并存**）。

**政策：** 只吸 pipeline / 状态机 / 捕获卫生；不抄 UI 皮肤；不把 Moon 做成纯 snip 工具；**不**用这些仓库当「多 continuous OCR 框」现成答案（三者都没有）。

**本地克隆（gitignored）：**

| 项目 | 路径 | Upstream |
|------|------|----------|
| Snow Shot | `tmp/reference/oss/snow-shot` | https://github.com/xiaofeiTM233/snow-shot （上游多为 mg-chao/snow-shot） |
| capcap | `tmp/reference/oss/capcap` | https://github.com/realskyrin/capcap |
| Kivio Desktop | `tmp/reference/oss/kivio` | https://github.com/ZMGID/kivio |

已有 OCR 对照：`tmp/reference/oss/{YomiNinja,LunaTranslator,STranslate,pot-desktop}` · 策略见 [OCR_STRATEGY.md](./OCR_STRATEGY.md)。

---

## 0. 能力矩阵（相对 Moon 单 OCR 框）

| 能力 | Moon | snow-shot | capcap | kivio |
|------|------|-----------|--------|-------|
| 栈 | Tauri+React | **Tauri2+React** | **纯 AppKit macOS** | **Tauri2+React** |
| 区域框 + 拖拽 | ✅ 单例 live | 一次框选→冻帧 OCR | 一次选区 | 一次框选/热键 |
| **多 continuous OCR 框** | ❌ 单例 | ❌ | ❌ | ❌ |
| 钉图/浮窗多开 | ❌ | ✅ 多贴图窗 | ✅ 多 Pin 图 | 会话浮卡 |
| Continuous 监视 | ✅ fp 门控 | ❌ | ❌ | ❌ |
| 绑 HWND follow | ✅ | ❌（贴图自由漂） | ❌ | ❌ |
| 排除自捕获 | ✅ WDA sampling | ✅ WDA Windows | ✅ sharingType + SCK exclude | ✅ SCK exclude self PID |
| 行级 OCR 叠字 | ✅ 单框 | ✅ 冻帧 blocks | ✅ Live Text 选行 | 替换模式画行 |
| 就地替换译文 | ❌/弱 | 弱 | 面板流式 | **✅ 强（inpaint+槽位）** |
| 本地 OCR 打包 | WinRT 等 | RapidOCR 插件 | Vision | RapidOCR/系统/云 |

**结论：**  
- Moon 在 **continuous + follow + 单 live 框** 上已经领先这三家。  
- 三家强在 **一次截图体验 / 钉图 / 捕获不吃自己 / 替换画字**。  
- **多框并存 continuous** 仍是 Moon 自研产品面，抄不来现成实现。

---

## 1. Snow Shot（Tauri 截图全家桶）

### 1.1 定位

轻量安装包 + **插件**拉 OCR/翻译/录屏；强调标注、滚动截图、窗口吸附、**贴图固定到屏幕**。

### 1.2 架构要点

| 区域 | 路径 |
|------|------|
| OCR 命令 crate | `src-tauri/src-crates/tauri-commands/ocr/`（`ocr_init` / `ocr_detect_core`） |
| 排除捕获 | `src-tauri/src-crates/app-utils` → `set_exclude_from_capture`（WDA） |
| 贴图多窗 | `src/functions/fixedContent.ts` · `pages/fixedContent/` |
| OCR 块 + 译 | `src/pages/draw/components/ocrBlocks/` · `ocrResult/` |
| 分段提示 | `src/constants/components/translation.ts`（`%%` 分隔） |

### 1.3 可抄（pipeline）

1. **多行 OCR → 一次 LLM**：用稳定分隔符（如 `%%`），返回行数不齐时按源布局比例对齐（`alignTranslatedBySourceProportion` 一类）。  
2. **小图上采样再 OCR**（`scale_factor` 过低时）。  
3. **捕获前后 WDA 开关 + 复位**检查清单（与 Moon sampling 对照）。  
4. **多贴图窗**仅作「静态对照」产品灵感，不是 continuous OCR。

### 1.4 不要抄

Excalidraw 标注、滚动长图、录屏、插件商店皮肤；别把 Moon 做成 snip 主产品。

### 1.5 对 Moon ROI

**中。** 栈最接近；优先吸 **分段 OCR→译** 与 **捕获亲和性复位**。

---

## 2. capcap（macOS 菜单栏截图）

### 2.1 定位

纯 **AppKit**、无 Electron/Tauri；双击 ⌘ 截图、标注、长截、美化、**钉图**、可选图床。OCR/翻译是截图后的面板能力。

### 2.2 架构要点

| 区域 | 路径 |
|------|------|
| 捕获 | `capcap/Capture/ScreenCapturer.swift` · `ScreenSnapshotProvider.swift` |
| 排除策略 | overlay `sharingType = .none`；SCK `excludingWindows`；**刻意不排除整个进程**以便钉图仍可被截 |
| 钉图 | `capcap/Capture/PinLauncher.swift` |
| OCR/译 | `capcap/Translation/OCRService.swift` · `OCRTranslatePanel.swift` · `TranslationService.swift` |
| 多屏遮罩池 | `OverlayPanelPool` / `OverlayWindowController` |

### 2.3 可抄（思路，非代码）

1. **排除 chrome 而不必整窗 hide 闪一下**：自有 UI 标为不可共享；捕获过滤器只剔 chrome 窗口 ID。  
2. **策略**：排除自己的壳，不排除整个进程 → 钉图/设置仍可作参考源。  
3. 行级可选 OCR token（后期 copy 单行）。  
4. 多显示器 overlay 生命周期。

### 2.4 不要抄

AppKit 编辑器、美化、图床、双击 ⌘；**不可移植到 Windows 的实现细节**。

### 2.5 对 Moon ROI

**低–中。** 捕获卫生文档最好；可移植性最差。只吸 **策略**。

---

## 3. Kivio Desktop（屏幕级 AI + 翻译）

### 3.1 定位

托盘 **Agent 客户端** + 屏幕工具：快译 / 选中译 / **截图译** / **替换译** + Lens 视觉问答。自带 Key，无中转。

### 3.2 架构要点

| 区域 | 路径 |
|------|------|
| Lens/截图命令 | `src-tauri/src/lens_commands.rs`（大体量） |
| 替换几何 | `src-tauri/src/replace_translation/layout.rs` · `mask` |
| 捕获 | macOS `sck.rs`（`exclude_self_pid`）；Win 几何 `capture_geometry.rs` |
| OCR | `rapidocr.rs`；设置 `ocrMode` cloud / system / rapid |
| 就地画字 FE | `src/lens/ReplaceTranslateOverlay.tsx` |
| 流式协议 | 组合流 + 分隔原文/译文（如 `<<<ORIGINAL>>>`） |

### 3.3 可抄（pipeline）

1. **替换翻译管线（高价值可选模式）**  
   OCR 行 → `build_replace_geometry` 分组/槽位 → mask/inpaint → **id 稳定批量译** → 流式事件 → 画到原位。  
2. **单次多模态流式契约**：译文 + 原文分段。  
3. **SCK/自 PID 排除**（与 Moon WDA 目标一致：少 hide 闪）。  
4. **引擎矩阵**：云 vision / 系统 OCR / 本地 RapidOCR + 模型档位下载。  
5. **多显示器 crop 几何** helper。

### 3.4 不要抄

Agent/MCP/RAG/Skills 整壳（另一产品）；GPL 面；默认依赖大型 inpaint 包除非 Moon 明确做「替换模式」。

### 3.5 对 Moon ROI

**高（叠字质量 / 捕获清洁）。** 仍 **不是** 多 continuous 框。Moon 已有 continuous+follow；Kivio 赢在 **就地替换布局** 与 **OCR 打包**。

---

## 4. 对 Moon 的映射（执行顺序）

**产品排期（owner，2026-07-29）——现在不做，以后必做：**

> 截图软件式使用：框选拖拽、多框可拖到旁边不关、框内 **原图 / 原文 / 译文** 切换。  
> 正式里程碑与 session 边界见 **[OCR_STRATEGY.md § Screenshot-app multi-frame](./OCR_STRATEGY.md)**（M0→M5）。

| 优先级 | 动作 | 参考 | 落点 | 注意 |
|--------|------|------|------|------|
| P0 | 用户 smoke 单框几何/闪烁 | — | 现有 region frame | **M0 门闩** |
| P1 | 捕获排除清单对齐（采样开/关复位） | snow-shot / kivio / capcap 策略 | `set_ocr_region_frame_sampling` | **不**改 session baton |
| P1 | OCR 多行 → 译 分隔符/对齐 | snow-shot `%%` | 已有 `run_batch` 可加强契约 | 非 OCR lifecycle |
| **Later M1** | 单框 **原图/原文/译文** 显示切换 | kivio 卡、snow-shot 块 | `OcrRegionFrame` viewMode | 仍单 session |
| **Later M2** | 多 **静态钉图**（可拖，不 continuous） | snow-shot fixedContent、capcap Pin | 新 pin label | 可与 session 解耦 |
| **Later M3+** | 多 **live** OCR 框 + per-id session | **无现成** | 多 label + 事件 regionId | **须先 multi-session 设计** |
| Later M5 | 可选「替换译」 | kivio replace | 布局+inpaint 可选 | 大块；独立模式 |
| P2 | 小图上采样再 OCR | snow-shot | 识别前 preprocess | 任意阶段可插 |

**明确非目标（与 OCR_STRATEGY 一致）：** 整站 snip 标注、美化、图床、Agent 壳、插件市场。

---

## 5. ROI 总表

| 排名 | 项目 | 原因 | 优先偷 |
|------|------|------|--------|
| 1 | **kivio** | 同 Tauri 档；OCR→布局→译/替换最强 | replace 几何+batch id；流式分隔；引擎档；mon 几何 |
| 2 | **snow-shot** | 同栈；`%%` 块对齐；WDA+多贴图 | 分段 prompt；上采样；affinity 复位 |
| 3 | **capcap** | 排除策略思维最好；仅 macOS | 只吸策略 |

---

## 6. 关键符号索引

```
snow-shot:
  src-tauri/src-crates/tauri-commands/ocr/src/lib.rs
  src-tauri/src-crates/app-utils/…          # exclude from capture
  src/pages/draw/components/ocrBlocks/
  src/pages/fixedContent/…/ocrResult/
  src/constants/components/translation.ts

capcap:
  capcap/Capture/ScreenCapturer.swift
  capcap/Capture/ScreenSnapshotProvider.swift
  capcap/Capture/PinLauncher.swift
  capcap/Translation/OCRService.swift
  capcap/Translation/OCRTranslatePanel.swift

kivio:
  src-tauri/src/lens_commands.rs
  src-tauri/src/sck.rs
  src-tauri/src/capture_geometry.rs
  src-tauri/src/rapidocr.rs
  src-tauri/src/replace_translation/layout.rs
  src/lens/ReplaceTranslateOverlay.tsx
```

---

## 7. 与划词参考的关系

| 文档 | 主题 |
|------|------|
| [REFERENCE_SELECTION_UX.md](./REFERENCE_SELECTION_UX.md) | 划词 / 悬停 / 取词 |
| **本页** | 截图壳 / OCR 框 / 钉图 / 替换译 |
| [OCR_STRATEGY.md](./OCR_STRATEGY.md) | Moon 产品规则与冻结项 |

克隆深度：`--depth 1`（2026-07-29）。更新上游：`git -C tmp/reference/oss/<name> pull`。
