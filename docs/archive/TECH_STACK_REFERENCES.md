# 技术选型和参考实现 - 2026-06-12

## 需求澄清总结

### 1. AI模型架构

**三层分离设计**：

```
┌─────────────────────────────────────────┐
│  Layer 1: 内置轻量翻译模型（本地）       │
│  目的：离线翻译，无网络依赖              │
│  选择：小型量化模型                      │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│  Layer 2: 词典学习AI（API）              │
│  目的：词义解释、例句生成、学习辅助      │
│  支持：OpenAI、Claude、Gemini、DeepSeek │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│  Layer 3: 翻译引擎（API）                │
│  目的：高质量翻译                        │
│  现有：Google、Youdao、DeepL等          │
└─────────────────────────────────────────┘
```

---

## 一、内置轻量翻译模型

### 参考项目

#### 1. 🌟 LibreTranslate (推荐)
**GitHub**: https://github.com/LibreTranslate/LibreTranslate
- ✅ 完全开源
- ✅ 支持多语言（英/俄/日/中/德等）
- ✅ 可离线使用
- ✅ 基于Argos Translate
- ✅ 模型小（每个语言对约30-50MB）

**集成方案**:
```bash
# 使用Python API包装
pip install libretranslate
# 或使用Docker镜像
docker pull libretranslate/libretranslate
```

**Tauri集成**:
- 可以打包Python运行时
- 或使用sidecar（独立进程）
- 模型文件放在app bundle中

#### 2. Bergamot Translator
**GitHub**: https://github.com/mozilla/bergamot-translator
- Mozilla开发
- 基于Marian NMT
- 在Firefox中使用
- C++实现，可编译为WASM

#### 3. NLLB (Meta)
**Model**: facebook/nllb-200-distilled-600M
- 支持200+语言
- 量化后约150MB
- 可用llama.cpp运行

**推荐方案**: **LibreTranslate** + 按需下载模型

---

## 二、词典学习API适配

### API统一接口层

参考OpenAI SDK的适配器模式：

```typescript
// src/services/ai/provider.ts

interface AIProvider {
  chat(messages: Message[], options?: ChatOptions): Promise<string>;
  stream(messages: Message[]): AsyncIterator<string>;
}

// OpenAI格式（标准）
class OpenAIProvider implements AIProvider {
  async chat(messages, options) {
    const response = await fetch('https://api.openai.com/v1/chat/completions', {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${this.apiKey}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        model: 'gpt-4',
        messages,
        ...options,
      }),
    });
    return response.json();
  }
}

// Claude格式
class ClaudeProvider implements AIProvider {
  async chat(messages, options) {
    // Anthropic Messages API
    const response = await fetch('https://api.anthropic.com/v1/messages', {
      method: 'POST',
      headers: {
        'x-api-key': this.apiKey,
        'anthropic-version': '2023-06-01',
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        model: 'claude-3-5-sonnet-20241022',
        messages,
        max_tokens: options.maxTokens || 1024,
      }),
    });
    return response.json();
  }
}

// Gemini格式
class GeminiProvider implements AIProvider {
  async chat(messages, options) {
    // Google Gemini API
    const response = await fetch(
      `https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent?key=${this.apiKey}`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          contents: this.convertMessages(messages),
          generationConfig: options,
        }),
      }
    );
    return response.json();
  }
}

// DeepSeek/通用OpenAI兼容格式
class DeepSeekProvider extends OpenAIProvider {
  constructor(apiKey: string) {
    super(apiKey, 'https://api.deepseek.com/v1');
  }
}
```

### 参考项目

#### 1. 🌟 Vercel AI SDK (推荐)
**GitHub**: https://github.com/vercel/ai
- ✅ 统一多个AI提供商
- ✅ 支持OpenAI、Anthropic、Google等
- ✅ TypeScript原生支持
- ✅ 流式响应

**使用示例**:
```typescript
import { openai } from '@ai-sdk/openai';
import { anthropic } from '@ai-sdk/anthropic';
import { google } from '@ai-sdk/google';
import { generateText } from 'ai';

// 统一接口，随时切换提供商
const result = await generateText({
  model: openai('gpt-4'), // 或 anthropic('claude-3-5-sonnet')
  messages: [...],
});
```

#### 2. LangChain.js
**GitHub**: https://github.com/langchain-ai/langchainjs
- 更重量级，功能更多
- 支持链式调用

---

## 三、词典格式和转换

### 开源词典格式

#### 1. 🌟 MDX/MDD (推荐)
**工具**: https://github.com/zhansliu/writemdict
- ✅ 最流行的开源词典格式
- ✅ 大量现成词典资源
- ✅ 支持HTML/CSS/JS
- ✅ 音频/图片支持

**已有词典资源**:
- 牛津高阶词典 (OALD)
- 柯林斯词典 (Collins)
- 朗文词典 (LDOCE)
- Cambridge Dictionary
- 俄语、日语、德语词典

**解析库**:
```bash
# JavaScript解析
npm install mdict-parser
```

#### 2. StarDict格式
**GitHub**: https://github.com/huzheng001/stardict-3
- 老牌开源词典
- 格式简单（.ifo .dict.dz .idx）
- 资源丰富

#### 3. DSL (ABBYY Lingvo)
- 纯文本格式
- 易于编辑
- 可转换为其他格式

### 词典转换工具

#### 🌟 PyGlossary (推荐)
**GitHub**: https://github.com/ilius/pyglossary
- ✅ 支持50+词典格式互转
- ✅ MDX ↔ StarDict ↔ DSL ↔ JSON
- ✅ Python实现，易集成

**使用示例**:
```python
from pyglossary import Glossary

glos = Glossary()
glos.read('oxford.mdx')
glos.write('oxford.json')  # 转为JSON供应用使用
```

### PDF/Word → 词典格式

#### AI辅助转换流程

```
PDF/Word文档
    ↓
提取文本（pypdf2/python-docx）
    ↓
AI解析结构（识别词条、释义、例句）
    ↓
生成MDX或JSON
    ↓
导入词典库
```

**参考项目**:
- **pdf2dict**: https://github.com/yourname/pdf2dict (需要自己开发)
- 可基于Claude API做智能解析

---

## 四、学习功能参考项目

### 遗忘曲线和间隔重复

#### 1. 🌟 Anki算法 (推荐)
**参考**: https://github.com/ankitects/anki
**算法**: https://faqs.ankiweb.net/what-spaced-repetition-algorithm.html

**核心算法（SM-2增强版）**:
```typescript
interface Card {
  word: string;
  easeFactor: number;    // 难度因子 (1.3-2.5)
  interval: number;       // 复习间隔（天）
  repetitions: number;    // 连续正确次数
  lastReview: Date;
  nextReview: Date;
}

function scheduleNextReview(card: Card, quality: 0|1|2|3|4|5): Card {
  // quality: 0=完全忘记, 5=完美记住
  
  if (quality < 3) {
    // 忘记了，重置
    return {
      ...card,
      repetitions: 0,
      interval: 1,
      nextReview: addDays(new Date(), 1),
    };
  }
  
  // 记住了，增加间隔
  let interval: number;
  if (card.repetitions === 0) {
    interval = 1;
  } else if (card.repetitions === 1) {
    interval = 6;
  } else {
    interval = Math.round(card.interval * card.easeFactor);
  }
  
  // 调整难度因子
  const newEaseFactor = Math.max(
    1.3,
    card.easeFactor + (0.1 - (5 - quality) * (0.08 + (5 - quality) * 0.02))
  );
  
  return {
    ...card,
    repetitions: card.repetitions + 1,
    interval,
    easeFactor: newEaseFactor,
    lastReview: new Date(),
    nextReview: addDays(new Date(), interval),
  };
}
```

#### 2. SuperMemo算法
**论文**: https://supermemo.com/en/archives1990-2015/english/ol/sm2

### 打卡和习惯追踪

#### 参考项目

**1. Habitica**
**GitHub**: https://github.com/HabitRPG/habitica
- 游戏化打卡系统
- 连续打卡奖励

**2. Loop Habit Tracker**
**GitHub**: https://github.com/iSoron/uhabits
- Android开源习惯追踪
- 简洁实用

**实现要点**:
```typescript
interface StudyStreak {
  currentStreak: number;      // 当前连续天数
  longestStreak: number;      // 最长连续
  totalDays: number;          // 总学习天数
  lastStudyDate: Date;
  
  calendar: {
    [date: string]: {
      wordsReviewed: number;
      timeSpent: number;      // 分钟
      completed: boolean;
    };
  };
}
```

---

## 五、云同步方案

### 方案对比

#### 方案1: 🌟 Cloudflare R2 + Workers (推荐)
**优势**:
- ✅ 免费额度大（10GB存储，1000万次请求/月）
- ✅ 速度快（全球CDN）
- ✅ S3兼容API
- ✅ Workers可做服务端逻辑

**架构**:
```
桌面/手机客户端
    ↓ (REST API)
Cloudflare Workers (认证、冲突解决)
    ↓
R2对象存储 (用户数据)
```

**参考项目**:
- **Obsidian Sync** 实现原理类似

#### 方案2: GitHub作为数据存储
**GitHub**: https://github.com/cyanzhong/synckit

**优势**:
- ✅ 完全免费
- ✅ 版本控制
- ✅ 易于分享

**劣势**:
- ❌ 不适合频繁写入
- ❌ 有API限制

#### 方案3: WebDAV (自托管)
**参考**: https://github.com/hacdias/webdav

**优势**:
- ✅ 标准协议
- ✅ 支持各种云盘（坚果云、NextCloud）

**实现**:
```typescript
// 已有WebDAV实现，在src-tauri/src/sync.rs
// 只需补全UI和冲突解决
```

### 冲突解决策略

参考**CRDTs**（Conflict-free Replicated Data Types）:
- **Automerge**: https://github.com/automerge/automerge
- **Yjs**: https://github.com/yjs/yjs

或使用简单的**Last-Write-Wins**策略：
```typescript
interface SyncData {
  version: number;
  timestamp: number;
  data: any;
  deviceId: string;
}

// 冲突时选择最新的
if (remote.timestamp > local.timestamp) {
  applyRemoteChanges();
}
```

---

## 六、跨平台开发

### 架构设计

```
核心代码（TypeScript + Rust）
    ↓
┌────────────┬─────────────┬──────────────┐
│  Desktop   │   Mobile    │  Mini-Program│
│  (Tauri)   │ (React Nat.)│   (微信)      │
└────────────┴─────────────┴──────────────┘
```

### 手机端方案

#### 方案1: 🌟 Tauri Mobile (推荐)
**官方**: https://beta.tauri.app/guides/develop/mobile/

**优势**:
- ✅ 代码复用（80%+共享）
- ✅ Rust性能
- ✅ 原生体验

**状态**: Beta（可用）

#### 方案2: React Native
**如果Tauri不满足**:
- 使用React Native
- 共享UI组件逻辑
- Rust核心编译为Native Module

### 微信小程序

**限制**:
- 不能使用原生模块
- 需要独立实现

**方案**:
```
小程序（纯Web）
    ↓ (API)
云函数 (Cloudflare Workers)
    ↓
共享数据存储
```

**参考**:
- 欧路词典小程序
- 扇贝单词小程序

---

## 七、完整技术栈推荐

### 前端
```
- Framework: React + TypeScript
- UI: Tailwind CSS
- 状态管理: Zustand（现有）
- 路由: React Router
- 词典渲染: DOMPurify + marked（支持MDX的HTML）
```

### 后端（Tauri/Rust）
```
- 词典解析: mdict-rs
- 数据库: SQLite (词典索引、学习数据)
- 同步: WebDAV client
- 轻量翻译: LibreTranslate sidecar
```

### AI服务
```
- API适配: Vercel AI SDK
- 支持: OpenAI、Claude、Gemini、DeepSeek
```

### 手机端
```
- Tauri Mobile (iOS/Android)
- 或 React Native（fallback）
```

### 云服务
```
- Cloudflare R2 + Workers
- 或 自托管WebDAV
```

---

## 八、参考完整项目

### 类似开源项目

#### 1. 🌟 Anki (推荐学习)
**GitHub**: https://github.com/ankitects/anki
- ✅ 完整的间隔重复系统
- ✅ 跨平台（桌面+手机）
- ✅ 云同步
- ✅ 插件系统

**可参考**:
- 学习算法实现
- 数据库结构
- 同步协议

#### 2. GoldenDict
**GitHub**: https://github.com/goldendict/goldendict
- ✅ 多格式词典支持
- ✅ MDX/StarDict解析

**可参考**:
- 词典文件处理
- 索引构建

#### 3. Eudic (欧路词典，闭源但可研究)
- 离线词典
- AI助手
- 学习功能

#### 4. Obsidian (同步参考)
**官网**: https://obsidian.md
- 文件同步策略
- 冲突解决

#### 5. Duolingo (学习体验参考)
- 游戏化设计
- 每日目标
- 连续打卡

---

## 九、开发路线图（基于参考项目）

### 阶段1: 基础词典系统（2-3周）
```
1. 集成PyGlossary（词典转换）
2. 实现MDX解析（参考GoldenDict）
3. 构建索引数据库
4. 词条查询界面
```

### 阶段2: AI学习辅助（2-3周）
```
1. 集成Vercel AI SDK
2. 实现多提供商适配
3. 词义解释生成
4. 例句和记忆技巧
```

### 阶段3: 间隔重复系统（2-3周）
```
1. 实现Anki算法（参考Anki源码）
2. 复习计划生成
3. 学习统计
4. 打卡日历
```

### 阶段4: 云同步（1-2周）
```
1. Cloudflare R2配置
2. Workers API开发
3. 冲突解决（Last-Write-Wins）
4. 增量同步
```

### 阶段5: 移动端（3-4周）
```
1. Tauri Mobile配置
2. UI适配
3. 离线支持
4. 推送通知
```

---

## 十、立即可做的工作

### Quick Start任务

1. **集成LibreTranslate**（1-2天）
   ```bash
   # 下载模型
   # 集成sidecar
   # 添加离线翻译选项
   ```

2. **MDX词典支持**（2-3天）
   ```bash
   # 安装mdict-parser
   # 导入词典功能
   # 词条显示
   ```

3. **AI API适配层**（1-2天）
   ```typescript
   // 基于Vercel AI SDK
   // 支持OpenAI、Claude、Gemini、DeepSeek
   ```

4. **基础学习记录**（2天）
   ```typescript
   // 生词本增强
   // 学习历史
   // 简单统计
   ```

---

## 总结

**核心技术选型**:
- 内置翻译: **LibreTranslate**
- 词典格式: **MDX** (PyGlossary转换)
- AI适配: **Vercel AI SDK**
- 学习算法: **Anki算法**
- 云同步: **Cloudflare R2 + Workers**
- 移动端: **Tauri Mobile**

**参考项目**:
1. Anki - 学习系统
2. GoldenDict - 词典解析
3. Vercel AI SDK - AI适配
4. PyGlossary - 词典转换
5. Habitica - 打卡系统

**下一步**:
1. 先实现MDX词典支持（最有价值）
2. 然后AI学习辅助
3. 最后云同步和移动端

需要我开始实施哪个功能？我可以先从MDX词典集成开始！

---

**文档日期**: 2026-06-12  
**整理者**: Claude Opus 4.8 (1M context)  
**状态**: 技术方案确定，待实施
