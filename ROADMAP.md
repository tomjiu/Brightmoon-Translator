# Moon Translator 开发计划

## 与 LunaTranslator 对比：功能差距

### A. 核心功能差距

| # | 功能 | 说明 | 优先级 | Phase |
|---|---|---|---|---|
| 1 | H-Code / T-Code 文本钩取 | 注入进程内存直接读取游戏文本，VN 翻译核心 | P0 | 2.3 |
| 2 | 翻译记忆库 (TM) | 存储已翻译句对，下次直接复用 | P0 | 2.1 |
| 3 | 多引擎并排对比 UI | 同一原文多引擎结果分栏展示 | P1 | 2.2 |
| 4 | Hook 配置文件保存/加载 | 针对不同游戏保存钩取配置 | P1 | 3.1 |
| 5 | 日语振假名 / Ruby 注音 | 汉字上方显示假名读音 | P1 | 3.2 |
| 6 | 自定义翻译 Prompt 模板 | 用户自定义 LLM system prompt | P1 | 3.3 |
| 7 | 翻译结果自动纠错/学习 | 用户修正后系统学习 | P2 | - |
| 8 | 语音包 / TTS 自动播放 | 翻译结果自动朗读 | P2 | - |
| 9 | 正则预处理管道 | 翻译前正则替换原文 | P1 | 2.4 |
| 10 | OCR 区域自动检测 | 自动检测游戏文本区域 | P2 | - |

### B. 架构差距

| # | 问题 | 说明 | Phase |
|---|---|---|---|
| A1 | AppState God Struct | 15 字段全堆一起，违反 ISP | 1.1 |
| A2 | hook_cmd 包含业务逻辑 | is_translatable/去重在 command 层 | 1.3 |
| A3 | 配置默认值三重复制 | Rust Default / TS DEFAULT / configStore | 1.4 |
| A4 | lib.rs 职责混乱 | 热键解析 80 行嵌在 setup 闭包 | 1.2 |
| A5 | overlay 定位逻辑重复 | HookMonitor 和 OcrMonitor 各自计算 | 1.5 |
| A6 | 无集中错误处理 | 每个 invoke 各自 .catch(() => {}) | 3.4 |
| A7 | TranslationService 方法冗余 | 三个 translate* 方法重复预处理 | - |
| A8 | Rust/TS 类型不同步 | 手动维护两边类型 | 1.4 |

---

## 开发阶段

### Phase 1: 架构治理

#### 1.1 AppState 子结构拆分
```
AppState
├── TranslationContext { service, engine_router, cache, glossary, metrics }
├── DocumentContext { history, wordbook, post_processor }
├── OverlayContext { follow_controller }
├── HookContext { hook_monitor }
└── SystemContext { config, selection_manager, app_detector, selection_translation, input_replacement }
```

#### 1.2 提取 hotkey.rs
- 将 parse_hotkey() 和注册逻辑从 lib.rs 移到 src/hotkey.rs
- lib.rs setup 从 120 行缩减到 10 行

#### 1.3 hook_cmd.rs 业务逻辑下沉
- is_translatable, 去重, 翻译调度 → capabilities/hook_monitor.rs
- hook_cmd.rs 只做参数解析 + 调用

#### 1.4 配置默认值统一
- Rust Default 为唯一权威来源
- 通过 ts-rs 自动生成 TypeScript 类型

#### 1.5 前端 overlay 定位统一
- 提取 src/services/overlayPosition.ts
- HookMonitor 和 OcrMonitor 共用

### Phase 2: 核心功能

#### 2.1 翻译记忆库
- 扩展 memory.rs: fuzzy_match(text, threshold)
- 翻译前先查 TM，命中直接返回
- 前端: TM 开关 + 相似度阈值

#### 2.2 多引擎并排对比
- 新增 parallel_translate() 命令
- 前端 CompareView 组件

#### 2.3 H-Code / T-Code 钩取
- 独立注入 DLL (C/C++)
- CreateRemoteThread + LoadLibrary 注入
- 拦截 TextOut/ExtTextOut/DrawText

#### 2.4 正则预处理管道
- 新增 pre_process.rs
- 翻译前正则管道

### Phase 3: 增强体验

#### 3.1 Hook Profile 管理
- HookProfile 结构体
- 前端 Profile 下拉菜单

#### 3.2 日语振假名注音
- MeCab / kuromoji-rs 形态分析
- <ruby> 渲染

#### 3.3 自定义 Prompt 模板
- 设置页 Prompt 编辑器
- 变量插值: {source_lang}, {target_lang}, {text}, {glossary}

#### 3.4 统一错误处理
- safeInvoke() 包装所有 Tauri invoke
- 统一 toast notification

---

## 依赖关系

Phase 1 (架构治理)
  ├── 1.1 AppState 拆分
  ├── 1.2 hotkey.rs 提取
  ├── 1.3 hook_cmd 下沉
  ├── 1.4 配置默认值统一
  └── 1.5 overlay 定位统一
          │
Phase 2 (核心功能)
  ├── 2.1 翻译记忆库 ← 依赖 1.1
  ├── 2.2 多引擎对比 ← 依赖 1.1
  ├── 2.3 H-Code 钩取 ← 依赖 1.3
  └── 2.4 正则预处理 ← 依赖 1.4
          │
Phase 3 (增强体验)
  ├── 3.1 Hook Profile ← 依赖 2.3
  ├── 3.2 振假名 ← 独立
  ├── 3.3 Prompt 模板 ← 依赖 2.1
  └── 3.4 统一错误处理 ← 依赖 1.1
