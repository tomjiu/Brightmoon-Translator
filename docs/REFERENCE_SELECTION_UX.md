# 划词 / 悬停 / OCR 取词 — 开源参考深读（2026-07-28）

**目的：** pot / STranslate / Luna 对「有道式选中 + 悬停词典 + OCR 强力取词」覆盖不足；本页记录 **新克隆** 的四个开源项目代码级可抄点，以及映射到 Moon 的实现顺序。

**政策：** 只偷 pipeline / 状态机 / 分词与取词策略；不抄 UI 皮肤；有道闭源二进制（`TextExtractor`/`Monitor.exe`）仅作 UX 对照，不 RE。

**本地克隆（gitignored）：**

| 项目 | 路径 | Upstream |
|------|------|----------|
| Easydict Win32 | `tmp/reference/oss/easydict_win32` | https://github.com/xiaocang/easydict_win32 |
| QTranslate (开源重写) | `tmp/reference/oss/QTranslate` | https://github.com/ahatem/QTranslate |
| Mouse Tooltip Translator | `tmp/reference/oss/MouseTooltipTranslator` | https://github.com/ttop32/MouseTooltipTranslator |
| YomiNinja | `tmp/reference/oss/YomiNinja` | https://github.com/matt-m-o/YomiNinja |

已有对照（本机早已有）：`tmp/reference/oss/{pot-desktop,STranslate,LunaTranslator}`、`tmp/reference/youdao-dict`（闭源 UX 镜像）。

---

## 0. 能力矩阵（相对有道 UX）

| 能力 | 有道 | pot | STranslate | Easydict Win | QTranslate | MTT (扩展) | YomiNinja | **Moon 现状** |
|------|------|-----|------------|--------------|------------|------------|-----------|---------------|
| 快捷键划词 | ✅ | ✅ | ✅ | ✅ | ✅ Ctrl+Q | — | — | ✅ |
| 选中后浮钮再译 | ✅ | ❌ | 鼠标钩+直接译 | ✅ **PopButton** | ❌（热键） | 选中 tooltip | — | ❌（可选自动即译） |
| 拖拽结束再取词 | ✅ | 热键 | ✅ DragFinished | ✅ WH_MOUSE_LL | 热键 | mousemove | — | ⚠️ LMB 轮询 |
| UIA 优先 / Ctrl+C 回退 | 闭源 | selection crate | Ctrl+C | ✅ **按进程分流** | 未深读 | DOM | — | ⚠️ 有链，无分流 |
| 终端禁 Ctrl+C | — | — | — | ✅ 名单 | — | — | — | ❌ |
| 系统悬停词典 | ✅ | ❌ | 文档有「悬停」偏划词 | ❌ | Ctrl+D 词典热键 | ✅ **词/句/块** | 叠字+扩展词典 | ⚠️ UIA Name 粗取 |
| 悬停图 OCR | ✅ 强力取词 | 截图 | 截图 | 截图 | 框选 OCR | ✅ **Shift+悬停图** | 全屏/区域 OCR | ⚠️ 光标小图 |
| 词典卡（音标/词性） | ✅ | 弱 | 弱 | Google Dict 等 | Quick Dictionary | Wiktionary 等 | 靠 10ten/Yomitan | ⚠️ 通用 overlay |

**结论：**  
- **选中 UX** 第一参考 = **Easydict**（钩子 + 浮钮 + 按应用取词策略）。  
- **悬停分词** 第一参考 = **MTT**（仅浏览器，但算法可移植概念）。  
- **OCR 强力** 参考 = **MTT 修饰键悬停 OCR** + **YomiNinja 叠字可点**。  
- pot/STranslate 继续当「热键 + 剪贴板 + 截图」基线，**不要**再当悬停词典主参考。

---

## 1. Easydict Win32（.NET / WinUI3）— 选中取词主力

### 1.1 架构（三件套）

| 文件 | 职责 |
|------|------|
| `dotnet/src/Easydict.WinUI/Services/MouseHookService.cs` | `WH_MOUSE_LL` + 可选 `WH_KEYBOARD_LL` |
| `.../TextSelectionService.cs` | UIA（FlaUI）/ 剪贴板 Ctrl+C，**按进程分类** |
| `.../PopButtonService.cs` | 拖拽结束 → 延迟取词 → **浮钮** → 用户点再开 Mini 窗 |

链路：

```text
LMB down → move ≥ MinDragDistance(10px) → LMB up
  → OnDragSelectionEnd(screenPoint)
  → PopButtonService: Delay 150ms
  → TextSelectionService.GetSelectedTextAsync()
  → 有字 → 显示 PopButton（5s 自动消失）
  → 用户点击 → MiniWindow 翻译
```

另：双击/三击走 `MultiClickDetector`，同样触发选区结束事件（不要求大拖拽）。

### 1.2 必须抄的细节

**A. 真拖拽，不误触单击**

```csharp
// MouseHookService — 概念
public const int MinDragDistance = 10;
// 平方距离 ≥ 100 才算 drag
```

Moon 现状用 `GetAsyncKeyState` 边沿，**无法区分 click / drag**。应对齐：  
- 优先 `WH_MOUSE_LL`（或等价）记录 down 点，up 时算距离；  
- 或至少要求移动阈值后才 auto-on-select。

**B. 合成 Ctrl+C 不关掉浮钮**

```csharp
// 合成按键 dwExtraInfo 打标，键盘钩跳过
internal const nint EASYDICT_SYNTHETIC_KEY = 0x4541_5344; // "EASD"
```

Moon 若用全局键钩 dismiss overlay，必须同样忽略自己的 SendInput。

**C. 按应用分流取词（核心）**

`TextSelectionService.GetSelectedTextAsync` 策略：

| 应用类型 | 策略 | 原因 |
|----------|------|------|
| **Electron**（code/slack/discord…） | **先剪贴板** Ctrl+C | UIA TextPattern 不可靠 |
| **终端**（wt/cmd/pwsh/mintty…） | **只 UIA，永不 Ctrl+C** | Ctrl+C = SIGINT |
| 其它 | UIA 先（信号量+超时）→ ClipWait 剪贴板 | 通用 |

附加：

- UIA 信号量 `Wait 200ms` + 执行超时 `800ms`，避免卡死。  
- 连续非文本剪贴板（如播放器复制帧）→ **进程级 5 分钟抑制** 剪贴板回退。  
- 前台是自己进程 → 跳过。  
- 设置里 **按进程排除** 鼠标划词（`IsMouseSelectionExcluded`）。

**D. 浮钮而不是立刻翻译**

`PopButtonService`：`SelectionDelayMs=150`，`AutoDismissMs=5000`。  
用户意图确认 → 少误触、少 API 费。有道/Easydict 一致。

### 1.3 Moon 映射（选中）

| # | 动作 | Moon 落点 |
|---|------|-----------|
| E1 | `WH_MOUSE_LL` 拖拽/双击检测 | 新 `selection/mouse_hook.rs` 或增强 `auto_watch` |
| E2 | Electron/终端进程名单 + 取词顺序 | `selection/clipboard.rs` + `uiautomation.rs` 前加 classifier |
| E3 | 合成 Ctrl+C 标记 / ClipWait 序列号 | 已有 sequence；补 marker + 终端禁 C |
| E4 | 可选「浮钮再译」模式 | 设置 `selectionUx` 第三档：`hotkey` / `auto` / `pop_button` |
| E5 | 排除进程列表 | `selectionUx.excludeProcesses: string[]` |

**不要抄：** WinUI 皮肤、FlaUI 整包（可用现有 `windows` crate UIA）。

---

## 2. QTranslate 开源重写（Kotlin / Swing）— 热键 + 词典窗

### 2.1 产品模型

- **Ctrl+Q**：选中 → Quick Translate **popup**（不依赖主窗）。  
- **Ctrl+D**：Quick Dictionary 浮层（词典插件）。  
- **Ctrl+I**：框选 OCR。  
- 一切热键 `GLOBAL` / `LOCAL` 数据驱动（`Configuration.hotkeys`）。

### 2.2 代码入口

| 区域 | 路径 |
|------|------|
| 架构说明 | `wiki/Architecture.md`（Hotkey system 段） |
| 快译窗 | `ui-swing/.../quciktranslate/QuickTranslateDialog.kt` |
| 词典窗 | `ui-swing/.../dictionary/QuickDictionaryDialog.kt` |
| OCR 用例 | `core/.../OcrAndTranslateUseCase.kt` |
| 词典 API | `api/.../dictionary/Dictionary.kt` |

### 2.3 可抄点

- **词典与翻译分窗/分热键**（Ctrl+D vs Ctrl+Q）→ Moon 应对齐：悬停/单字 = 词典卡；多字 = 翻译 overlay。  
- Popup：**多显示器按鼠标所在屏**摆位置；**鼠标离开 120ms debounce** 再关。  
- OCR 框选后 action bar：复制字/图、再裁剪（体验，非必须）。

### 2.4 局限

全局选区实现偏 **热键触发**，不是 Easydict 级「拖完出钮」。选中链路仍以 Easydict 为主。

---

## 3. Mouse Tooltip Translator（浏览器扩展）— 悬停 / OCR-on-hover

### 3.1 悬停取词（核心算法）

文件：`src/event/mouseover.js`

```text
mousemove → debounce(mouseoverEventInterval, 默认~300ms)
  → clientX/Y
  → caretRangeFromPoint / 元素上 Range
  → 按类型 expand：word | sentence | container
  → 派发 mouseoverText 事件 → 翻译 tooltip
```

要点：

| 点 | 说明 |
|----|------|
| **类型** | `mouseoverTextType`: word / sentence / container；按住键可互换 |
| **特殊块** | 目标带 `ocr_text_div` 等 class → 强制 container |
| **双击无移动** | 仍要出 tooltip（把 selection 完成当 active） |
| **输入框焦点** | 默认可隐藏 tooltip 免挡打字 |
| **词典源** | 可选 Wiktionary 等（tooltip 词条，不只是机翻） |

桌面移植概念（**无 DOM**）：

1. 光标稳定 N ms（已有 dwell）。  
2. **优先**：UIA TextPattern 在插入点/选区拿 **词边界**（若有）。  
3. 否则：控件 Name/Value 上做 **英文空格分词 / CJK 字窗**（已有 `extract_word_candidate`，需按指针相对控件 bounds 估偏移，目前仍偏粗）。  
4. 单字/短语 → **词典**；长句 → **翻译**（QTranslate 的 Q/D 分离）。

### 3.2 悬停 OCR（强力取词范式）

文件：`src/ocr/ocrView.js`

```text
按住 keyDownOCR（默认 left-shift）+ 鼠标在 <img> 上
  → 取图 base64
  → tesseract 多模式并行 (auto / bbox / …)
  → 页面上铺 ocr_text_div
  → 之后可像普通字一样 mouseover
```

Moon 映射：

| # | 动作 | 落点 |
|---|------|------|
| M1 | 修饰键 + 悬停才 OCR（默认关，避免费电） | `selectionUx.ocrForcePickup` 可改为「修饰键模式」 |
| M2 | OCR 结果拆成可点词块（不只整段） | 未来 OCR 框 / 悬停结果层 |
| M3 | 结果缓存 per image src | 避免重复 OCR |

### 3.3 不要直接做的

整站 contentScript、Tippy 主题、YouTube 双字幕 — 与桌面无关。

---

## 4. YomiNinja（Electron）— OCR 叠字 + 外置词典

### 4.1 模型

```text
选窗口/区域 → 截图 → Paddle/Manga OCR 服务
  → overlay 窗口把字盖在原图上
  → 用户用 10ten / Yomitan 悬停查词（浏览器扩展装进 Electron）
```

目录：`yomininja-e/electron-src/{ocr_recognition,overlay,screen_capturer,ocr_templates,dictionaries}`。

### 4.2 可抄点

- **OCR 与词典解耦**：OCR 只负责「变成可选/可悬停文字」。  
- **模板区域**（固定游戏对话框框）→ 连续监视省算力。  
- Auto OCR + 绑定窗口跟随（与 Moon OCR region follow 同族）。

### 4.3 Moon 映射

短期不嵌 Yomitan；中期「OCR 结果按词 hit-test」可借鉴 overlay 分层。游戏向再开 T2。

---

## 5. 旧参考简评（为何不够）

| 项目 | 划词 | 缺口 |
|------|------|------|
| **pot** | 热键 → `selection::get_text` → 窗 | 无浮钮、无系统悬停、无按应用分流 |
| **STranslate** | `MouseKeyHelper` 拖拽结束 + `ClipboardHelper` Ctrl+C | 有鼠标划词，但无终端保护、无浮钮、悬停词典弱 |
| **Luna** | HOOK/OCR/剪贴板模式 | 游戏注入向；非通用划词词典 |
| **有道镜像** | `TextExtractor*.dll` / `Monitor.exe` | 闭源；只看 resultui 布局 |

STranslate 仍值得保留：**DragFinished + GetSelectedTextAsync(timeout) + 序列号**（Moon clipboard 路径已部分对齐）。

---

## 6. Moon 落地优先级（建议）

### P0 — 选中可靠（Easydict）

1. **进程分类**取词：Electron 先剪贴板；终端禁 Ctrl+C。 → **Done 2026-07-28** `selection/process_class.rs` + `manager.get_selection_routed`  
2. **拖拽阈值** 减少单击误触。 → **Done** `auto_watch` 跟踪 `min_drag_px`（默认 10；非完整 WH_MOUSE_LL）  
3. 设置 **排除进程**。 → **Done** `selectionUx.excludeProcesses`  
4. 可选 **浮钮模式**。 → **Done 2026-07-28** `selection/pop_button.rs` + `triggerMode: pop_button`

### P1 — 悬停词典（MTT 概念 + 现有 UIA）

1. 单字/短语 → `lookup_dictionary` 词典卡（音标+词性），勿用整段机翻 overlay。  
2. dwell + 同词去重（已有雏形）。  
3. 分词：word vs sentence 设置项。

### P2 — OCR 强力（MTT + 现有小图 OCR）

1. 默认 **修饰键** 才光标 OCR。  
2. 选中为空时 OCR 回退保留，但降频、小 ROI。  
3. 长期：OCR 词块可点（YomiNinja 方向）。

### 设置页（已有「划词取词」）

继续集中：`triggerMode` / `hoverDictionary` / `ocrForcePickup` / 浮层级别；后续加 `pop_button`、`excludeProcesses`、`ocrModifierKey`。

---

## 7. 关键文件速查

### Easydict

- `MouseHookService.cs` — LL 钩、拖拽/多击  
- `TextSelectionService.cs` — Electron/终端/UIA/ClipWait  
- `PopButtonService.cs` — 延迟、浮钮、自动消失  
- `PopButtonWindow.xaml(+.cs)` — 浮钮 UI  

### QTranslate

- `wiki/Architecture.md` — 热键 GLOBAL/LOCAL  
- `QuickTranslateDialog.kt` / `QuickDictionaryDialog.kt`  
- `OcrAndTranslateUseCase.kt`  

### MTT

- `src/event/mouseover.js` — 悬停分词  
- `src/ocr/ocrView.js` — Shift+悬停 OCR  
- `src/contentScript.js` — 总装  
- `CLAUDE.md` — 仓库约定  

### YomiNinja

- `yomininja-e/electron-src/ocr_recognition/`  
- `yomininja-e/electron-src/overlay/`  
- `yomininja-e/electron-src/screen_capturer/`  

### Moon 对应实现（本阶段）

- `src-tauri/src/selection/{auto_watch,hover_pick,clipboard,uiautomation}.rs`  
- `src-tauri/src/capabilities/selection_translation_impl.rs`  
- `src/pages/settings/SelectionSettings.tsx`  

---

## 8. 诚实边界

- **没有**开源项目完整复刻有道桌面「任意像素悬停取词」；最接近的是 MTT（仅网页 DOM）+ 有道闭源。  
- Easydict 是目前 **Windows 桌面选中链路** 质量最高的开源实现，应作为 P0 主教材。  
- 克隆体积大、在 `tmp/reference`（gitignore）；更新用 `git -C <path> pull`。

---

*写于 2026-07-28；克隆均为 `--depth 1`。*
