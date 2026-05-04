# Moon Translator - 架构设计文档

## 项目定位

结合三个参考项目的优点：
- **沉浸式翻译**: 双语对照、多种翻译模式、输入框翻译
- **LunaTranslator**: OCR图像翻译、悬浮窗穿透点击
- **Read Frog**: 上下文感知翻译、批量请求优化、自定义Prompt

---

## 一、技术栈

| 组件 | 选型 | 理由 |
|------|------|------|
| 桌面端框架 | **Tauri 2.0** | 轻量、Rust高性能、原生窗口控制 |
| 前端UI | **React + TypeScript + Vite** | 生态成熟、开发效率高 |
| 翻译引擎 | **Rust async** | 并发请求、流式输出 |
| OCR | **Windows.Media.Ocr + Tesseract.js** | 原生OCR精度高，Tesseract跨平台兜底 |
| Firefox插件 | **WXT框架 + React** | 参考Read Frog，现代化扩展开发 |
| 状态管理 | **Zustand** | 轻量、TypeScript友好 |
| 样式 | **Tailwind CSS** | 原子化CSS、暗色主题内置 |

---

## 二、功能模块清单

### 2.1 PC桌面端

#### A. 翻译模式
1. **文本输入翻译** - 主界面输入框，防抖自动翻译
2. **OCR图像翻译** - 截图→OCR识别→翻译→悬浮窗显示
3. **剪贴板监听** - 监听剪贴板变化自动翻译
4. **划词翻译** - 全局选中文字快捷键翻译

#### B. 悬浮窗系统（核心难点）
1. **穿透点击** - 鼠标事件穿透到下层窗口
2. **文字可选中** - 翻译结果区域user-select:text
3. **位置跟随** - 显示在截图选区附近
4. **置顶显示** - always-on-top
5. **ESC关闭** - 快捷键关闭
6. **可拖拽** - 用户可拖动调整位置
7. **可复制** - 一键复制翻译结果

#### C. 翻译引擎
1. **LLM引擎** - OpenAI兼容API (DeepSeek/OpenAI/自定义)
2. **Google翻译** - 免费API
3. **百度翻译** - 需要密钥
4. **有道翻译** - 需要密钥
5. **引擎路由器** - 并行调用、自动降级
6. **流式输出** - LLM翻译逐字显示
7. **翻译缓存** - 避免重复请求

#### D. 上下文感知（参考Read Frog）
1. **术语表** - 用户自定义专业术语
2. **自定义Prompt** - 用户可配置翻译提示词
3. **上下文传递** - 传入前后文提升准确性

#### E. 系统功能
1. **全局快捷键** - Ctrl+Shift+T截图翻译，Ctrl+T唤起主窗口
2. **系统托盘** - 最小化到托盘
3. **配置持久化** - JSON文件存储
4. **历史记录** - 本地存储，支持搜索

### 2.2 Firefox插件

#### A. 翻译模式（参考沉浸式翻译）
1. **双语对照翻译** - 原文段落下方插入译文
2. **划词翻译** - 选中文字弹出翻译工具栏
3. **悬停翻译** - 鼠标悬停段落按快捷键翻译
4. **输入框翻译** - 输入框内连按三次空格触发翻译
5. **整页翻译** - 翻译整页内容

#### B. 工具栏功能（参考Read Frog）
1. **翻译按钮** - 流式翻译
2. **复制按钮** - 复制翻译结果
3. **TTS按钮** - 朗读原文/译文（可选）

#### C. 设置功能
1. **引擎配置** - LLM API Key、服务商选择
2. **语言配置** - 默认源语言/目标语言
3. **Prompt配置** - 自定义翻译提示词
4. **快捷键配置** - 自定义快捷键

---

## 三、项目结构（最终版）

```
moontranslator/
├── src-tauri/                          # Tauri后端
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   └── src/
│       ├── main.rs                     # 入口
│       ├── lib.rs                      # 应用状态+命令注册
│       ├── config.rs                   # 配置管理
│       ├── history.rs                  # 历史记录
│       ├── engine/                     # 翻译引擎
│       │   ├── mod.rs                  # 引擎trait+路由器
│       │   ├── llm.rs                  # LLM引擎(OpenAI兼容)
│       │   ├── google.rs              # Google翻译
│       │   ├── baidu.rs               # 百度翻译
│       │   └── youdao.rs              # 有道翻译
│       ├── ocr/                        # OCR模块
│       │   ├── mod.rs                  # OCR接口
│       │   ├── windows_ocr.rs         # Windows原生OCR
│       │   └── capture.rs             # 屏幕截图
│       └── commands/                   # Tauri命令
│           ├── mod.rs
│           ├── translate.rs           # 翻译命令
│           ├── ocr.rs                 # OCR命令
│           ├── window.rs              # 窗口管理命令
│           ├── config_cmd.rs          # 配置命令
│           └── history_cmd.rs         # 历史命令
│
├── src/                                # React前端
│   ├── main.tsx                        # 入口
│   ├── App.tsx                         # 路由+布局
│   ├── styles.css                      # 全局样式
│   ├── stores/                         # Zustand状态
│   │   ├── translateStore.ts          # 翻译状态
│   │   └── configStore.ts            # 配置状态
│   ├── pages/
│   │   ├── MainTranslator.tsx         # 主翻译页
│   │   ├── Settings.tsx               # 设置页
│   │   └── History.tsx                # 历史页
│   ├── components/
│   │   ├── TranslationInput.tsx       # 输入框
│   │   ├── TranslationResult.tsx      # 结果展示(多引擎对比)
│   │   ├── LanguageSelector.tsx       # 语言选择器
│   │   ├── EngineSelector.tsx         # 引擎选择器
│   │   ├── FloatingOverlay.tsx        # 悬浮翻译窗
│   │   └── OcrSelector.tsx           # OCR区域选择
│   ├── hooks/
│   │   ├── useTranslate.ts            # 翻译hook
│   │   └── useOcr.ts                 # OCR hook
│   └── types/
│       └── index.ts                   # 类型定义
│
├── firefox-extension/                  # Firefox插件
│   ├── manifest.json                  # MV2清单
│   ├── background/
│   │   └── service-worker.js          # 后台脚本
│   ├── content/
│   │   ├── selector.js               # 划词翻译
│   │   ├── hover.js                  # 悬停翻译
│   │   ├── fullpage.js               # 整页翻译
│   │   ├── input-box.js              # 输入框翻译
│   │   └── content.css               # 内容脚本样式
│   ├── popup/
│   │   ├── popup.html
│   │   ├── popup.js
│   │   └── popup.css
│   └── icons/
│       ├── icon-48.png
│       └── icon-96.png
│
├── package.json
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.js
├── postcss.config.js
└── index.html
```

---

## 四、数据流设计

### 4.1 PC端翻译流程
```
用户输入 → 防抖500ms → 调用Tauri命令 → 引擎路由器
                                              ↓
                                    ┌─────────┼─────────┐
                                    ↓         ↓         ↓
                                  LLM      Google    Baidu
                                    ↓         ↓         ↓
                                    └─────────┼─────────┘
                                              ↓
                                    合并结果返回前端 → 渲染
```

### 4.2 OCR翻译流程
```
快捷键Ctrl+Shift+T → 进入截图模式 → 框选区域
                                        ↓
                              截图保存为base64 → OCR识别
                                                    ↓
                                              识别文字 → 翻译
                                                          ↓
                                              创建悬浮窗显示结果
```

### 4.3 Firefox插件翻译流程
```
划词/悬停/输入框 → Content Script捕获文本
                          ↓
               发送消息到Background Script
                          ↓
               Background调用翻译API
                          ↓
               返回结果到Content Script
                          ↓
               注入翻译结果到页面DOM
```

---

## 五、关键实现细节

### 5.1 悬浮窗穿透点击（核心难点）
```rust
// Tauri创建透明窗口
WebviewWindowBuilder::new(app, "overlay", url)
    .transparent(true)           // 透明背景
    .always_on_top(true)         // 置顶
    .decorations(false)          // 无边框
    .skip_taskbar(true)          // 不显示在任务栏
    .build()?;

// Windows平台设置鼠标穿透
#[cfg(target_os = "windows")]
window.set_ignore_cursor_events(true);  // 鼠标穿透

// 翻译结果区域需要响应鼠标（选中文字）
// 通过JavaScript动态切换穿透状态
```

### 5.2 流式输出
```rust
// LLM引擎支持SSE流式返回
async fn translate_stream(&self, text: &str, ...) -> impl Stream<Item = String> {
    // 使用reqwest的stream feature
    // 解析SSE data行
    // 逐chunk发送到前端
}

// 前端通过Tauri事件监听流式结果
import { listen } from '@tauri-apps/api/event';
listen('translate-chunk', (event) => {
    // 追加显示
});
```

### 5.3 双语对照翻译（参考沉浸式翻译）
```javascript
// Firefox插件：提取页面段落
const paragraphs = document.querySelectorAll('p, h1, h2, h3, h4, h5, h6, li, td, th, blockquote');

// 批量翻译（参考Read Frog批量优化）
const batchTexts = paragraphs.map(p => p.textContent);
const translations = await batchTranslate(batchTexts);

// 插入译文到原文下方
paragraphs.forEach((p, i) => {
    const translated = document.createElement('div');
    translated.className = 'moon-translation';
    translated.textContent = translations[i];
    p.insertAdjacentElement('afterend', translated);
});
```

### 5.4 批量请求优化（参考Read Frog）
```rust
// 将多个翻译请求合并为单次API调用
async fn batch_translate(&self, texts: &[String], from: &str, to: &str) -> Vec<String> {
    let combined = texts.join("\n\n---SPLIT---\n\n");
    let prompt = format!(
        "请翻译以下多段文本，用---SPLIT---分隔，返回时也用同样分隔符分隔每段翻译：\n\n{}",
        combined
    );
    let result = self.translate_single(&prompt, from, to).await;
    result.split("---SPLIT---").map(|s| s.trim().to_string()).collect()
}
```

---

## 六、开发阶段

### Phase 1: 项目骨架 + 可编译运行
- 清理现有代码，重新搭建项目结构
- 确保 `pnpm tauri dev` 能启动
- 前端能看到基本界面

### Phase 2: 核心翻译功能
- LLM引擎完整实现
- Google翻译引擎
- 引擎路由器
- 主翻译界面交互

### Phase 3: 设置+历史
- 配置管理完整实现
- 设置页面UI
- 历史记录存储和展示

### Phase 4: OCR功能
- Windows原生OCR集成
- 屏幕截图功能
- 悬浮翻译窗（穿透点击）

### Phase 5: 系统功能
- 全局快捷键
- 系统托盘
- 流式输出

### Phase 6: Firefox插件
- 插件框架搭建
- 划词翻译
- 双语对照整页翻译
- 悬停翻译
- 输入框翻译

### Phase 7: 高级功能
- 术语表
- 自定义Prompt
- 批量请求优化
- 翻译缓存
