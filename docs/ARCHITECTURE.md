# Moon Translator 架构文档

## 项目概述

Moon Translator 是一款多功能桌面翻译工具，集成了 OCR 截图翻译、划词翻译、浏览器扩展、多引擎对比等特性。项目基于 Tauri 2.0 + React + Rust 构建，采用前后端分离架构。

### 核心特性

- **多翻译引擎**: LLM (DeepSeek/OpenAI)、Google、百度、有道、DeepL、DeepLX、Microsoft、Yandex
- **OCR 翻译**: Windows 原生 OCR + Tesseract.js 跨平台兜底
- **悬浮窗系统**: 穿透点击、文字可选中、位置跟随、置顶显示
- **浏览器扩展**: Chrome MV3 / Firefox 双语对照、划词翻译
- **文档翻译**: PDF、DOCX、Excel、PPTX、EPUB、字幕文件
- **Hook 监控**: UIA 文本捕获、剪贴板监听

---

## 技术栈

| 组件 | 选型 | 版本 |
|------|------|------|
| 桌面端框架 | Tauri | 2.0 |
| 前端 UI | React + TypeScript + Vite | 18.3 / 5.5 / 5.4 |
| 状态管理 | Zustand | 4.5 |
| 样式方案 | Tailwind CSS | 3.4 |
| 后端语言 | Rust | 1.70+ |
| HTTP 客户端 | reqwest | 0.12 |
| 异步运行时 | tokio | 1.x |
| HTTP 服务器 | axum | 0.7 |
| 数据库 | rusqlite | 0.31 |
| 浏览器扩展 | Chrome MV3 / Firefox | - |

---

## 目录结构

```
moontranslator/
├── src/                          # React 前端
│   ├── main.tsx                  # 入口文件
│   ├── App.tsx                   # 路由 + 布局
│   ├── components/               # UI 组件
│   │   ├── translator/           # 翻译器组件
│   │   ├── HookMonitor.tsx       # Hook 监控
│   │   ├── OcrScreenshotTranslator.tsx  # OCR 截图翻译
│   │   └── ...
│   ├── pages/                    # 页面组件
│   │   ├── MainTranslator.tsx    # 主翻译页
│   │   ├── Settings.tsx          # 设置页
│   │   ├── DocumentsViewer.tsx   # 文档查看器
│   │   ├── Vocabulary.tsx        # 词汇本
│   │   └── Plugins.tsx           # 插件管理
│   ├── stores/                   # Zustand 状态
│   │   ├── translateStore.ts     # 翻译状态
│   │   ├── configStore.ts        # 配置状态
│   │   ├── clipboardStore.ts     # 剪贴板状态
│   │   └── ...
│   ├── services/                 # 服务层
│   │   ├── invoke.ts             # Tauri invoke 封装
│   │   ├── ocr.ts                # OCR 服务
│   │   └── tts.ts                # TTS 服务
│   ├── hooks/                    # 自定义 Hooks
│   ├── i18n/                     # 国际化
│   │   ├── zh.json               # 中文
│   │   └── en.json               # 英文
│   └── types/                    # TypeScript 类型
│
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs               # 程序入口
│   │   ├── lib.rs                # 应用状态 + 命令注册
│   │   ├── commands/             # Tauri 命令
│   │   │   ├── translate.rs      # 翻译命令
│   │   │   ├── window.rs         # 窗口管理
│   │   │   ├── capture.rs        # 截图 + OCR
│   │   │   ├── config_cmd.rs     # 配置命令
│   │   │   └── ...
│   │   ├── engine/               # 翻译引擎
│   │   │   ├── mod.rs            # 引擎 trait + Router
│   │   │   ├── llm.rs            # LLM 引擎
│   │   │   ├── google.rs         # Google 翻译
│   │   │   ├── baidu.rs          # 百度翻译
│   │   │   ├── youdao.rs         # 有道翻译
│   │   │   └── ...
│   │   ├── models/               # 数据模型
│   │   │   ├── config.rs         # 配置模型
│   │   │   ├── translation.rs    # 翻译模型
│   │   │   └── error.rs          # 错误类型
│   │   ├── services/             # 服务层
│   │   │   └── translation.rs    # 翻译服务
│   │   ├── capabilities/         # 能力模块
│   │   │   ├── selection_translation.rs  # 划词翻译
│   │   │   ├── input_replacement.rs      # 输入替换
│   │   │   ├── hook_monitor.rs           # Hook 监控
│   │   │   └── browser_translation.rs    # 浏览器翻译
│   │   ├── overlay/              # 悬浮窗系统
│   │   ├── selection/            # 文本选择提供者
│   │   ├── api_server.rs         # HTTP API 服务器
│   │   ├── cache.rs              # 翻译缓存
│   │   ├── memory.rs             # 历史记录
│   │   └── ...
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── extension/                    # 浏览器扩展 (Chrome MV3)
│   ├── manifest.json
│   ├── background/
│   │   └── service-worker.js
│   ├── content/
│   │   ├── selector.js           # 划词翻译
│   │   ├── page-translator.js    # 整页翻译
│   │   └── hover-translator.js   # 悬停翻译
│   └── popup/
│       └── popup.js
│
├── firefox-extension/            # Firefox 扩展
├── scripts/                      # 工具脚本
└── docs/                         # 文档
```

---

## 核心架构模式

### 1. 前后端分离

```
┌─────────────────────────────────────────────────────────┐
│                    React 前端 (WebView)                   │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐    │
│  │ 组件    │  │ 状态    │  │ 服务    │  │ Hooks   │    │
│  └────┬────┘  └────┬────┘  └────┬────┘  └─────────┘    │
│       │            │            │                        │
│       └────────────┼────────────┘                        │
│                    │ invoke() / listen()                 │
├────────────────────┼────────────────────────────────────┤
│                    │ Tauri Bridge                        │
├────────────────────┼────────────────────────────────────┤
│                    ▼                                     │
│  ┌─────────────────────────────────────────────────┐    │
│  │              Rust 后端 (Tauri)                    │    │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐         │    │
│  │  │ Commands│  │ Services│  │ Engine  │         │    │
│  │  └────┬────┘  └────┬────┘  └────┬────┘         │    │
│  │       │            │            │               │    │
│  │       └────────────┼────────────┘               │    │
│  │                    │                            │    │
│  │              ┌─────▼─────┐                      │    │
│  │              │ AppState  │                      │    │
│  │              └───────────┘                      │    │
│  └─────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
```

### 2. 状态管理

**前端 (Zustand)**:
- `translateStore` - 翻译源文本、目标文本、加载状态
- `configStore` - 应用配置
- `clipboardStore` - 剪贴板状态
- `themeStore` - 主题切换
- `toastStore` - 通知提示

**后端 (AppState)**:
```rust
pub struct AppState {
    pub translation: TranslationContext,   // 翻译服务、引擎、缓存
    pub document: DocumentContext,         // 历史、词汇本、后处理
    pub overlay: OverlayContext,           // 悬浮窗控制
    pub hook: HookContext,                 // Hook 监控
    pub system: SystemContext,             // 配置、选择管理器
    pub batch: Arc<BatchManager>,          // 批量翻译
}
```

### 3. 引擎路由

翻译引擎采用策略模式，支持多种路由策略：

```rust
pub enum RoutingStrategy {
    PrimaryOnly,      // 仅主引擎
    FallbackOnError,  // 失败时降级
    ParallelCompare,  // 并行对比
    CostAware,        // 成本优先
    LatencyFirst,     // 延迟优先
}
```

引擎优先级：LLM > Youdao > DeepL > DeepLX > Baidu > Microsoft > Yandex > Google > Plugins

### 4. 事件系统

Tauri 事件用于前后端通信：

| 事件名 | 方向 | 用途 |
|--------|------|------|
| `stream-chunk` | 后端→前端 | 流式翻译块 |
| `trigger-ocr-screenshot` | 后端→前端 | 触发 OCR 截图 |
| `selection-translated` | 后端→前端 | 划词翻译结果 |
| `auto-copy` | 后端→前端 | 自动复制 |
| `navigate` | 后端→前端 | 页面导航 |
| `read-clipboard` | 后端→前端 | 读取剪贴板 |

---

## 数据流

### 翻译流程

```
用户输入 → 防抖 500ms → invoke("translate")
                              │
                              ▼
                     TranslationService
                              │
                              ▼
                      ┌───────┴───────┐
                      │   Router      │
                      │ (策略选择)    │
                      └───────┬───────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
         Engine 1        Engine 2        Engine N
              │               │               │
              └───────────────┼───────────────┘
                              ▼
                     TranslateResponse
                              │
                              ▼
                     前端渲染结果
```

### OCR 翻译流程

```
快捷键 Ctrl+Shift+T
        │
        ▼
创建截图选择器窗口
        │
        ▼
用户框选区域
        │
        ▼
截图保存为 base64
        │
        ▼
Windows OCR / Tesseract.js
        │
        ▼
识别文字 → 翻译
        │
        ▼
创建悬浮窗显示结果
```

### 浏览器扩展流程

```
Content Script 捕获文本
        │
        ▼
发送消息到 Background
        │
        ▼
Background 调用 API
(127.0.0.1:60828/browser/translate)
        │
        ▼
返回结果到 Content Script
        │
        ▼
注入翻译结果到页面 DOM
```

---

## 关键设计决策

### 1. 为什么选择 Tauri 而不是 Electron?

- **体积小**: 打包后约 5-10MB，Electron 约 100MB+
- **性能好**: Rust 后端，内存占用低
- **原生能力**: 直接调用 Windows API (UIA、OCR)
- **安全性**: 最小权限原则

### 2. 为什么使用 Zustand 而不是 Redux?

- **轻量**: 无 boilerplate
- **TypeScript 友好**: 类型推断好
- **简单 API**: 学习成本低
- **性能**: 细粒度订阅

### 3. 为什么使用 axum 作为 API 服务器?

- **Tokio 生态**: 与 Tauri 共享异步运行时
- **类型安全**: 编译时检查
- **性能**: 零成本抽象
- **中间件**: tower-http 支持 CORS

### 4. 为什么支持多个翻译引擎?

- **冗余**: 单引擎故障时自动降级
- **质量**: 不同引擎在不同语言对上表现不同
- **成本**: 免费引擎优先，付费引擎兜底
- **速度**: 并行调用获取最快结果

### 5. 为什么使用 UIA 而不是剪贴板获取选中文本?

- **非侵入**: 不修改用户剪贴板
- **实时性**: 直接读取 UI 元素
- **准确性**: 可获取精确的选区范围
- **兼容性**: 支持大多数 Windows 应用

---

## 模块依赖关系

```
                    ┌─────────────┐
                    │   commands  │
                    └──────┬──────┘
                           │
           ┌───────────────┼───────────────┐
           ▼               ▼               ▼
    ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
    │  services   │ │ capabilities│ │   overlay   │
    └──────┬──────┘ └──────┬──────┘ └──────┬──────┘
           │               │               │
           └───────────────┼───────────────┘
                           ▼
                    ┌─────────────┐
                    │   engine    │
                    └──────┬──────┘
                           │
           ┌───────────────┼───────────────┐
           ▼               ▼               ▼
    ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
    │   models    │ │   config    │ │   cache     │
    └─────────────┘ └─────────────┘ └─────────────┘
```

---

## 性能优化

### 1. 翻译缓存

- LRU 缓存避免重复请求
- 缓存键: hash(text + from + to + engine)
- 可配置 TTL

### 2. 批量翻译

- 合并多段文本为单次请求
- 并发控制 (默认 3)
- 进度回调

### 3. 流式输出

- LLM 引擎支持 SSE 流式返回
- 前端逐字显示
- 减少用户等待感

### 4. 防抖处理

- 输入翻译防抖 500ms
- 剪贴板监听防抖 500ms
- 窗口位置保存防抖 500ms

---

## 安全考虑

### 1. API 密钥保护

- 配置文件存储在 `%APPDATA%/moontranslator/`
- API 服务器响应中密钥脱敏
- 浏览器扩展仅允许 localhost 访问

### 2. CSP 配置

- 开发模式: 允许 eval (HMR)
- 生产模式: 严格 CSP

### 3. 网络安全

- HTTPS 优先
- 代理支持
- 请求超时控制

---

## 扩展性

### 1. 插件系统

- 基于 HTTP 端点的插件架构
- manifest.json 描述插件能力
- 支持翻译、OCR、TTS 插件类型

### 2. 引擎扩展

- 实现 `TranslationEngine` trait
- 注册到 Router
- 自动参与路由策略

### 3. 前端扩展

- 组件化架构
- Zustand 状态隔离
- i18n 国际化支持
