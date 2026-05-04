# Moon Translator - 功能整合计划

## 一、参考项目分析总结

| 项目 | 技术栈 | 核心特色 | 可借鉴功能 |
|------|--------|----------|------------|
| **LunaTranslator** | Python + C++ | Hook模式注入游戏进程 | Hook文本提取、440+引擎自动检测、嵌入式翻译 |
| **pot-desktop** | Tauri + React | 多引擎并行翻译 | 插件系统、HTTP API服务器、变量名转换、鼠标跟随窗口 |
| **STranslate** | C# WPF | 插件架构+多种翻译方式 | 增量翻译、替换翻译、剪贴板监听、外部HTTP API |
| **immersive-translate** | Browser Extension | 网页双语翻译 | 30+翻译服务、视频字幕翻译、20+显示主题 |
| **read-frog** | WXT Extension | AI驱动的沉浸式翻译 | 批量请求优化、上下文感知翻译、25+AI Provider |

---

## 二、功能优先级矩阵

### P0 - 核心差异化功能（立即实现）

| 功能 | 来源 | 价值 | 实现难度 |
|------|------|------|----------|
| **Hook模式** | LunaTranslator | 游戏翻译杀手锏，市场稀缺 | 高 |
| **多引擎并行翻译** | pot-desktop | 提升翻译质量和用户体验 | 中 |
| **剪贴板监听翻译** | STranslate | 极大提升日常使用效率 | 低 |
| **窗口位置记忆** | pot-desktop | 基础体验优化 | 低 |

### P1 - 重要增强功能（近期实现）

| 功能 | 来源 | 价值 | 实现难度 |
|------|------|------|----------|
| **本地HTTP API服务器** | pot-desktop/STranslate | 生态集成能力 | 中 |
| **替换翻译（打字回填）** | STranslate | 独特的交互方式 | 中 |
| **自定义Prompt模板** | read-frog | LLM翻译质量提升 | 低 |
| **批量翻译请求** | read-frog | 降低API成本70% | 中 |
| **实时OCR模式** | STranslate | 连续屏幕翻译 | 中 |

### P2 - 体验增强功能（中期实现）

| 功能 | 来源 | 价值 | 实现难度 |
|------|------|------|----------|
| **插件系统** | pot-desktop/STranslate | 可扩展性 | 高 |
| **视频字幕翻译** | immersive-translate | 视频学习场景 | 高 |
| **翻译遮罩（学习模式）** | immersive-translate | 语言学习辅助 | 低 |
| **术语表增强** | LunaTranslator | 专业翻译质量 | 中 |
| **变量名转换** | pot-desktop | 开发者友好 | 低 |

---

## 三、Hook模式实现方案（重点）

### 3.1 架构设计

```
┌─────────────────────┐     Named Pipes      ┌─────────────────────┐
│   目标游戏进程        │  ◄──────────────────►  │   Moon Translator    │
│   moon_hook.dll      │     (异步通信)         │   Rust Host DLL      │
│   - MinHook引擎      │                       │   - 命令分发          │
│   - VEH异常处理      │                       │   - 文本处理管线      │
│   - 内存读取         │                       │   - 引擎检测器        │
└─────────────────────┘                       └─────────────────────┘
                                                        │
                                                        ▼
                                              ┌─────────────────────┐
                                              │   Tauri Frontend     │
                                              │   - 翻译结果显示      │
                                              │   - Hook管理UI       │
                                              │   - 引擎选择器       │
                                              └─────────────────────┘
```

### 3.2 技术实现路径

#### Phase 1: 基础Hook框架（Rust实现）

```rust
// src-tauri/src/hook/mod.rs
pub mod injector;
pub mod pipe_server;
pub mod text_hook;
pub mod engine_detector;

// 核心结构
pub struct HookManager {
    pipes: HashMap<u32, PipeConnection>,  // PID -> Pipe
    hooks: Vec<ActiveHook>,
    text_tx: mpsc::Sender<HookTextEvent>,
}
```

**DLL注入器** (`injector.rs`):
- 使用 `windows` crate 调用 `OpenProcess`, `VirtualAllocEx`, `WriteProcessMemory`, `CreateRemoteThread`
- 支持32/64位进程注入
- 管理员权限自动提权

**命名管道通信** (`pipe_server.rs`):
- 使用 `windows::Win32::System::Pipes` 创建命名管道
- 异步读写，tokio集成
- 消息协议：Header(PID, HookCode, TextLength) + TextData

#### Phase 2: Hook DLL（C/C++）

```cpp
// moon_hook/main.cc
// 使用 MinHook 库进行函数Hook
// 支持三种Hook方式：
// 1. Inline Hook (MinHook) - 主要方式
// 2. VEH Hook - 备用方式（Unity游戏）
// 3. 内存轮询 - 最后手段
```

**引擎检测**:
- 从LunaTranslator移植核心引擎定义（精选100+常用引擎）
- 按游戏类型分类：RPGMaker, Unity, Unreal, KiriKiri, NScripter等

#### Phase 3: 前端集成

```typescript
// src/hooks/useGameHook.ts
export function useGameHook() {
  const [hookStatus, setHookStatus] = useState<HookStatus>('disconnected');
  const [threads, setThreads] = useState<HookThread[]>([]);
  const [selectedThread, setSelectedThread] = useState<string>('');

  // 监听Hook事件
  useEffect(() => {
    const unlisten = listen<HookTextEvent>('hook-text', (event) => {
      // 处理拦截到的文本
      translateText(event.payload.text);
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);
}
```

### 3.3 与现有翻译系统集成

```
Hook文本 → 文本过滤/清理 → 术语表匹配 → 翻译引擎 → 结果显示/嵌入
                              ↓
                        缓存检查（命中则直接返回）
```

---

## 四、多引擎并行翻译方案

### 4.1 架构

```typescript
// src/services/parallelTranslate.ts
interface TranslationCard {
  id: string;
  engine: string;
  status: 'loading' | 'done' | 'error';
  result?: string;
}

async function parallelTranslate(
  text: string,
  engines: string[],
  onResult: (engine: string, result: string) => void
) {
  const promises = engines.map(engine =>
    translateWithEngine(engine, text)
      .then(result => onResult(engine, result))
  );
  await Promise.allSettled(promises);
}
```

### 4.2 UI设计

```
┌────────────────────────────────────────┐
│  原文：Hello World                      │
│  [自动检测: 英语]  ▼    [翻译] [清空]   │
├────────────────────────────────────────┤
│  ┌─ DeepSeek ──────────────┐  [📌] [📋]│
│  │ 你好世界                  │          │
│  │ ✓ 0.8s                   │          │
│  └─────────────────────────┘          │
│  ┌─ Google ────────────────┐  [📌] [📋]│
│  │ 你好，世界                │          │
│  │ ✓ 0.3s                   │          │
│  └─────────────────────────┘          │
│  ┌─ DeepL ─────────────────┐  [📌] [📋]│
│  │ 你好世界                  │          │
│  │ ✓ 0.5s                   │          │
│  └─────────────────────────┘          │
└────────────────────────────────────────┘
```

---

## 五、剪贴板监听翻译方案

```rust
// src-tauri/src/commands/clipboard_monitor.rs
use windows::Win32::UI::WindowsAndMessaging::*;

#[command]
pub async fn start_clipboard_monitor(app: AppHandle) -> Result<(), String> {
    // 创建隐藏窗口监听 WM_CLIPBOARDUPDATE
    // 使用 AddClipboardFormatListener
    // 去重逻辑：记录上次文本hash
    // 发送事件到前端
}

#[command]
pub async fn stop_clipboard_monitor() -> Result<(), String> {
    // RemoveClipboardFormatListener
}
```

```typescript
// src/components/ClipboardToggle.tsx
function ClipboardToggle() {
  const [enabled, setEnabled] = useState(false);

  useEffect(() => {
    if (enabled) {
      const unlisten = listen<string>('clipboard-changed', (e) => {
        translateText(e.payload);
      });
      return () => { unlisten.then(fn => fn()); };
    }
  }, [enabled]);

  return <Toggle checked={enabled} onChange={setEnabled} label="剪贴板监听" />;
}
```

---

## 六、本地HTTP API服务器

### 6.1 端点设计

```
POST /translate          - 翻译文本
POST /ocr               - OCR识别
POST /ocr-translate     - OCR + 翻译
POST /tts               - 语音合成
GET  /history           - 获取历史记录
POST /hook/attach       - 附加到进程
POST /hook/detach       - 从进程分离
GET  /hook/threads      - 获取Hook线程列表
POST /hook/select       - 选择Hook线程
```

### 6.2 实现

```rust
// src-tauri/src/api_server.rs
use axum::{Router, routing::post, Json};

pub async fn start_api_server(port: u16) {
    let app = Router::new()
        .route("/translate", post(translate_handler))
        .route("/ocr", post(ocr_handler))
        .route("/ocr-translate", post(ocr_translate_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

---

## 七、自定义Prompt模板

### 7.1 模板变量

| 变量 | 说明 | 示例 |
|------|------|------|
| `{{input}}` | 待翻译文本 | "Hello World" |
| `{{sourceLang}}` | 源语言 | "English" |
| `{{targetLang}}` | 目标语言 | "Chinese" |
| `{{context}}` | 上下文（游戏/网页场景） | "RPG游戏对话" |
| `{{glossary}}` | 术语表 | "NPC=非玩家角色" |

### 7.2 预设模板

```typescript
const PROMPT_TEMPLATES = {
  general: {
    name: '通用翻译',
    system: '你是专业翻译。准确传达原文含义，保持自然流畅。',
    user: '翻译以下{{sourceLang}}文本为{{targetLang}}：\n{{input}}'
  },
  game: {
    name: '游戏翻译',
    system: '你是游戏本地化专家。保持角色语气，处理游戏术语。',
    user: '【游戏场景】{{context}}\n术语表：{{glossary}}\n\n翻译：\n{{input}}'
  },
  technical: {
    name: '技术文档',
    system: '你是技术文档翻译专家。保留专业术语，保持代码格式。',
    user: '翻译技术文档：\n{{input}}'
  },
  literature: {
    name: '文学翻译',
    system: '你是文学翻译家。注重文采和意境传达。',
    user: '翻译文学作品：\n{{input}}'
  }
};
```

---

## 八、批量翻译请求优化

### 8.1 批量协议

```
请求：paragraph1 %% paragraph2 %% paragraph3
响应：translation1 %% translation2 %% translation3
```

### 8.2 实现

```typescript
// src/services/batchTranslate.ts
class BatchQueue {
  private queue: Map<string, TranslateRequest[]> = new Map();
  private flushTimer: NodeJS.Timeout | null = null;

  add(request: TranslateRequest) {
    const key = `${request.engine}-${request.sourceLang}-${request.targetLang}`;
    if (!this.queue.has(key)) this.queue.set(key, []);
    this.queue.get(key)!.push(request);

    // 达到阈值或超时后批量发送
    if (this.getTotalChars(key) > 2000 || this.queue.get(key)!.length > 10) {
      this.flush(key);
    } else if (!this.flushTimer) {
      this.flushTimer = setTimeout(() => this.flushAll(), 500);
    }
  }

  private async flush(key: string) {
    const requests = this.queue.get(key) || [];
    this.queue.delete(key);

    const batchText = requests.map(r => r.text).join(' %% ');
    try {
      const batchResult = await translateBatch(batchText);
      const results = batchResult.split(' %% ');
      requests.forEach((req, i) => req.resolve(results[i]));
    } catch {
      // 降级为逐个翻译
      for (const req of requests) {
        try {
          const result = await translate(req.text, req.engine);
          req.resolve(result);
        } catch (e) {
          req.reject(e);
        }
      }
    }
  }
}
```

---

## 九、实现路线图

### Phase 1: 基础能力补齐（2周）

- [ ] 剪贴板监听翻译
- [ ] 窗口位置/大小记忆
- [ ] 快捷键自定义配置
- [ ] 翻译结果自动复制选项

### Phase 2: 翻译能力增强（3周）

- [ ] 多引擎并行翻译UI
- [ ] 自定义Prompt模板
- [ ] 批量翻译请求优化
- [ ] 术语表增强（导入导出、优先级）

### Phase 3: Hook模式开发（4周）

- [ ] Rust DLL注入器
- [ ] C++ Hook DLL（MinHook + VEH）
- [ ] 命名管道通信
- [ ] 引擎检测器（移植核心引擎）
- [ ] Hook管理UI
- [ ] 嵌入式翻译（回写游戏）

### Phase 4: 生态能力（2周）

- [ ] 本地HTTP API服务器
- [ ] 实时OCR模式
- [ ] 翻译遮罩（学习模式）
- [ ] 变量名转换工具

### Phase 5: 插件系统（3周）

- [ ] 插件接口定义
- [ ] 插件加载/管理
- [ ] 插件市场（可选）

---

## 十、技术债务和风险

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| Hook DLL的反作弊检测 | 游戏可能检测到DLL注入 | 提供多种注入方式，用户自担风险 |
| 32/64位兼容性 | 需要维护两套DLL | 使用条件编译，统一代码库 |
| 翻译API限流 | 多引擎并行可能触发限流 | 令牌桶限流 + 指数退避 |
| 内存占用 | Hook DLL增加目标进程内存 | 精简DLL体积，按需加载引擎 |
| 管理员权限 | 注入需要提权 | 明确提示用户，提供手动注入选项 |

---

## 十一、与竞品差异化定位

| 能力 | Moon Translator | pot-desktop | STranslate | LunaTranslator |
|------|----------------|-------------|------------|----------------|
| Hook模式 | ✅ (计划) | ❌ | ❌ | ✅ (核心) |
| 多引擎并行 | ✅ (计划) | ✅ | ✅ | ❌ |
| OCR翻译 | ✅ | ✅ | ✅ | ✅ |
| 网页翻译 | ❌ (Firefox插件) | ❌ | ❌ | ❌ |
| 跨平台 | ❌ (Windows优先) | ✅ | ❌ | ❌ |
| Tauri架构 | ✅ | ✅ (v1) | ❌ | ❌ |
| 现代UI | ✅ | ✅ | ✅ | ❌ |
| HTTP API | ✅ (计划) | ✅ | ✅ | ❌ |
| 插件系统 | ✅ (计划) | ✅ | ✅ | ❌ |

**核心差异化**: Moon Translator = Tauri现代架构 + Hook游戏翻译 + 多引擎并行 + 网页翻译插件

---

## 十二、依赖新增

### Rust (Cargo.toml)

```toml
[dependencies]
# 新增
axum = "0.7"                    # HTTP API服务器
tower-http = { version = "0.5", features = ["cors"] }
windows = { version = "0.58", features = [
    "Win32_System_Memory",
    "Win32_System_Threading",
    "Win32_System_Pipes",
    "Win32_UI_WindowsAndMessaging",
    "Media_Ocr",
    "Graphics_Imaging",
]}
```

### Node (package.json)

```json
{
  "dependencies": {
    "zustand": "^4.5",
    "lucide-react": "^0.400"
  }
}
```

---

*文档生成时间: 2026-05-03*
*基于: LunaTranslator, pot-desktop, STranslate, immersive-translate, read-frog 五个参考项目分析*
