# AI Agent 框架选型与集成方案

生成时间: 2026-06-13
目标: 选择成熟的 AI Agent 框架，快速集成智能学习功能

---

## 🎯 需求分析

### 我们需要什么？
1. **单词卡片生成 Agent**: 输入单词 → 输出结构化内容（例句/词根/助记法）
2. **学习计划生成 Agent**: 分析用户数据 → 生成今日学习任务
3. **智能复习调度**: 根据熟练度 → 计算下次复习时间
4. **Rust 后端集成**: Tauri 命令调用 Agent

### 不需要什么？
❌ 复杂的多 Agent 协作
❌ 向量数据库（我们单词量小，不需要）
❌ 持续学习/微调（直接用 LLM）
❌ 图形化工作流编辑器

---

## 📊 成熟框架对比

### 1. **LangChain** (Python)
- ⭐⭐⭐⭐⭐ 最流行
- ✅ 丰富的工具生态（LLM/Prompt/Memory）
- ✅ Structured Output（Pydantic）
- ❌ Python，需要额外服务
- ❌ 依赖重（几百 MB）

### 2. **LangGraph** (Python)
- ⭐⭐⭐⭐ LangChain 团队出品
- ✅ 有状态的 Agent 工作流
- ✅ 图形化调试
- ❌ Python，复杂度高
- ❌ 我们不需要复杂工作流

### 3. **AutoGen** (Microsoft, Python)
- ⭐⭐⭐⭐ 多 Agent 对话
- ✅ 代码生成能力强
- ❌ 重量级（多 Agent 协作）
- ❌ 我们场景太简单，用不上

### 4. **LlamaIndex** (Python)
- ⭐⭐⭐⭐ RAG 框架
- ✅ 文档检索 + LLM
- ❌ 我们不需要 RAG
- ❌ 向量数据库依赖

### 5. **Instructor** (Python)
- ⭐⭐⭐⭐⭐ **最适合我们**
- ✅ 轻量级（只做 Structured Output）
- ✅ Pydantic 模型 → LLM → 验证输出
- ✅ 支持 OpenAI/Anthropic/Ollama
- ✅ 简单直接，无复杂依赖

### 6. **Vercel AI SDK** (TypeScript)
- ⭐⭐⭐⭐ 前端友好
- ✅ TypeScript 原生
- ✅ Streaming 支持
- ❌ 主要面向 Web，不适合 Rust 后端

### 7. **Rust AI 生态**
- **llm-chain-rs**: ⭐⭐ 半成品
- **candle**: ⭐⭐⭐ Meta 的推理框架，太底层
- **rig-rs**: ⭐⭐⭐ 新项目，生态不成熟
- ❌ Rust AI 生态还不够成熟

---

## 🎯 推荐方案

### 方案 A: **Instructor (Python) + FastAPI 微服务** ⭐推荐

#### 架构
```
┌─────────────────┐
│ Tauri Desktop   │
│ (Rust)          │
└────┬────────────┘
     │ HTTP (127.0.0.1:8001)
     ▼
┌─────────────────┐
│ FastAPI 服务     │
│ (Python)        │
│ - Instructor    │
│ - Pydantic      │
└────┬────────────┘
     │
     ▼
┌─────────────────┐
│ OpenAI/DeepSeek │
│ API             │
└─────────────────┘
```

#### 代码示例

**Python 服务 (ai_service.py)**
```python
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field
import instructor
from openai import OpenAI

app = FastAPI()

# 使用 Instructor 包装 OpenAI client
client = instructor.from_openai(OpenAI(api_key="..."))

# 定义输出结构
class WordCard(BaseModel):
    phonetic: str = Field(description="IPA 音标")
    part_of_speech: str = Field(description="词性")
    definitions: list[str] = Field(description="释义列表")
    examples: list[str] = Field(min_length=3, max_length=5)
    etymology: str | None = Field(description="词根词源")
    mnemonic: str = Field(description="助记法")
    synonyms: list[str]
    antonyms: list[str]
    related_words: list[str]
    difficulty_level: str = Field(pattern="^(A1|A2|B1|B2|C1|C2)$")

@app.post("/generate_word_card")
async def generate_word_card(word: str, source_lang: str, target_lang: str):
    try:
        # Instructor 自动验证输出格式
        card = client.chat.completions.create(
            model="gpt-4",
            response_model=WordCard,  # Pydantic 模型
            messages=[
                {"role": "system", "content": "你是专业的语言学习助手"},
                {"role": "user", "content": f"为单词 '{word}' 生成完整学习卡片"}
            ]
        )
        return card.model_dump()
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

# 学习计划生成
class LearningPlan(BaseModel):
    today_words: list[str] = Field(max_length=5)
    review_words: list[str] = Field(max_length=10)
    estimated_minutes: int = Field(ge=5, le=60)
    reasoning: str

@app.post("/generate_learning_plan")
async def generate_learning_plan(wordbook_stats: dict):
    plan = client.chat.completions.create(
        model="gpt-4",
        response_model=LearningPlan,
        messages=[
            {"role": "system", "content": "你是智能学习计划助手"},
            {"role": "user", "content": f"根据数据生成计划: {wordbook_stats}"}
        ]
    )
    return plan.model_dump()

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="127.0.0.1", port=8001)
```

**Rust 调用 (src-tauri/src/commands/ai_cmd.rs)**
```rust
use reqwest;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct WordCard {
    pub phonetic: String,
    pub part_of_speech: String,
    pub definitions: Vec<String>,
    pub examples: Vec<String>,
    pub etymology: Option<String>,
    pub mnemonic: String,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
    pub related_words: Vec<String>,
    pub difficulty_level: String,
}

#[tauri::command]
pub async fn generate_word_card(
    word: String,
    source_lang: String,
    target_lang: String,
) -> Result<WordCard, String> {
    let client = reqwest::Client::new();
    
    let response = client
        .post("http://127.0.0.1:8001/generate_word_card")
        .query(&[
            ("word", &word),
            ("source_lang", &source_lang),
            ("target_lang", &target_lang),
        ])
        .send()
        .await
        .map_err(|e| format!("HTTP 请求失败: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("AI 服务错误: {}", response.status()));
    }
    
    let card: WordCard = response
        .json()
        .await
        .map_err(|e| format!("JSON 解析失败: {}", e))?;
    
    Ok(card)
}
```

**部署方式**
```bash
# 打包成单文件可执行程序
pip install pyinstaller
pyinstaller --onefile ai_service.py

# Tauri 启动时自动启动 AI 服务
# src-tauri/src/lib.rs
fn setup_ai_service() {
    std::process::Command::new("./ai_service.exe")
        .spawn()
        .expect("Failed to start AI service");
}
```

#### 优势
✅ **成熟稳定**: Instructor 是业界标准，维护活跃
✅ **类型安全**: Pydantic 自动验证 LLM 输出
✅ **轻量级**: 只依赖 FastAPI + Instructor，打包后 ~20MB
✅ **易扩展**: 新增 Agent 只需定义 Pydantic 模型
✅ **热更新**: Python 服务可独立更新，无需重新编译 Rust

#### 劣势
⚠️ 额外进程（AI 服务 + Tauri）
⚠️ 用户需要安装 Python 环境（或打包成可执行文件）

---

### 方案 B: **纯 Rust + JSON Schema** ⭐轻量

如果不想引入 Python 依赖，可以用现有的 Rust LLM 调用 + 手动 JSON 验证。

```rust
// src-tauri/src/engine/llm.rs

use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize)]
pub struct WordCard {
    pub phonetic: String,
    pub part_of_speech: String,
    pub definitions: Vec<String>,
    pub examples: Vec<String>,
    // ... 其他字段
}

pub async fn generate_word_card_structured(
    llm_config: &LlmConfig,
    word: &str,
    source_lang: &str,
) -> Result<WordCard, String> {
    // 使用 OpenAI response_format: json_schema
    let schema = json!({
        "type": "object",
        "properties": {
            "phonetic": { "type": "string" },
            "part_of_speech": { "type": "string" },
            "definitions": { "type": "array", "items": { "type": "string" } },
            "examples": { "type": "array", "items": { "type": "string" }, "minItems": 3 },
            // ...
        },
        "required": ["phonetic", "part_of_speech", "definitions", "examples"]
    });
    
    let messages = vec![
        json!({"role": "system", "content": "你是语言学习助手"}),
        json!({"role": "user", "content": format!("为 '{}' 生成学习卡片", word)}),
    ];
    
    let response = call_openai_with_schema(llm_config, messages, schema).await?;
    
    // 解析并验证
    let card: WordCard = serde_json::from_str(&response)
        .map_err(|e| format!("JSON 解析失败: {}", e))?;
    
    // 手动验证
    if card.examples.len() < 3 {
        return Err("例句数量不足".to_string());
    }
    
    Ok(card)
}

async fn call_openai_with_schema(
    config: &LlmConfig,
    messages: Vec<serde_json::Value>,
    schema: serde_json::Value,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    
    let body = json!({
        "model": config.model,
        "messages": messages,
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "word_card",
                "schema": schema,
                "strict": true  // 强制遵守 schema
            }
        },
        "temperature": 0.7
    });
    
    let response = client
        .post(&format!("{}/chat/completions", config.base_url))
        .header("Authorization", format!("Bearer {}", config.api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("No content")?;
    
    Ok(content.to_string())
}
```

#### 优势
✅ 无额外依赖，纯 Rust
✅ 单一进程
✅ 打包体积小

#### 劣势
⚠️ 手动验证 JSON，不如 Pydantic 优雅
⚠️ OpenAI `json_schema` 模式需要 gpt-4-turbo+（贵）
⚠️ 不支持 DeepSeek 等国产模型的 structured output

---

### 方案 C: **LangChain.js (TypeScript)** 备选

如果前端也需要 AI 功能（浏览器扩展），可以考虑。

```typescript
// extension/background/ai-agent.ts
import { ChatOpenAI } from "@langchain/openai";
import { z } from "zod";

const model = new ChatOpenAI({
  apiKey: config.llmApiKey,
  model: "gpt-4",
});

// Zod schema 定义
const WordCardSchema = z.object({
  phonetic: z.string(),
  partOfSpeech: z.string(),
  definitions: z.array(z.string()),
  examples: z.array(z.string()).min(3).max(5),
  etymology: z.string().optional(),
  mnemonic: z.string(),
  synonyms: z.array(z.string()),
  antonyms: z.array(z.string()),
  relatedWords: z.array(z.string()),
  difficultyLevel: z.enum(["A1", "A2", "B1", "B2", "C1", "C2"]),
});

const structuredModel = model.withStructuredOutput(WordCardSchema);

async function generateWordCard(word: string) {
  const result = await structuredModel.invoke([
    { role: "system", content: "你是语言学习助手" },
    { role: "user", content: `为 '${word}' 生成学习卡片` },
  ]);
  
  return result; // 自动验证类型
}
```

#### 优势
✅ TypeScript 原生，类型安全
✅ 可以在扩展中直接调用（离线时降级）

#### 劣势
⚠️ 前端打包体积大（几 MB）
⚠️ 不适合复杂 Agent 逻辑

---

## 🎯 最终推荐

### 短期（1-2 周）: 方案 B（纯 Rust）
**理由**: 
- ✅ 快速启动，无额外依赖
- ✅ 用 OpenAI `response_format: json_object` 即可
- ✅ 单一可执行文件

**实现步骤**:
1. 扩展现有 `call_llm` 函数，支持 JSON 模式
2. 定义 `WordCard` / `LearningPlan` 结构体
3. 手动验证关键字段（例句数量、难度等级）

### 中期（1 个月后）: 方案 A（Instructor + FastAPI）
**理由**:
- ✅ 功能复杂后，Pydantic 自动验证更可靠
- ✅ Python 生态丰富，可接入更多工具
- ✅ 可以做更复杂的 RAG / Multi-Agent

**迁移路径**:
1. 保留 Rust 调用接口不变
2. 后端从 Rust 切换到 Python 微服务
3. 用 PyInstaller 打包成单文件，随 Tauri 分发

---

## 📦 现成工具推荐

### 1. **单词卡片生成**
- ✅ 直接用 ChatGPT API + 详细 Prompt
- ✅ 不需要额外框架

### 2. **间隔重复算法**
- ✅ 用现成库: **anki-algorithm-rs** (Rust)
- GitHub: https://github.com/open-spaced-repetition/fsrs-rs
- MIT 协议，直接集成

### 3. **学习统计可视化**
- ✅ 前端用 **Recharts** (React)
- 显示学习曲线、熟练度分布

---

## 📝 行动计划

### Week 1: 基础集成
- [ ] 扩展 `WordBookItem` 数据结构
- [ ] 实现 `generate_word_card` (纯 Rust + JSON)
- [ ] Dictionary 页面连接真实 wordbook

### Week 2: AI 卡片
- [ ] 优化 Prompt，保证输出质量
- [ ] 添加缓存机制（避免重复生成）
- [ ] 批量生成功能

### Week 3-4: 学习系统
- [ ] 集成 FSRS 间隔重复算法
- [ ] 学习计划生成
- [ ] 闪卡学习界面

### Month 2: 高级优化（可选）
- [ ] 迁移到 Instructor + FastAPI
- [ ] 多模态支持（图片例句）
- [ ] 发音朗读（TTS）

---

## 💡 总结

**不要造轮子，直接用成熟方案**：

1. **短期**: OpenAI API + Structured Output (纯 Rust)
2. **中期**: Instructor + FastAPI (Python 微服务)
3. **算法**: FSRS 间隔重复算法 (开源库)

**核心原则**: 
- 先用最简单的方案快速验证
- 功能复杂后再引入更强大的框架
- 优先选择轻量级、单一职责的工具

**下一步**: 
1. 用纯 Rust 实现第一版（1-2 周）
2. 收集用户反馈
3. 再决定是否迁移到更复杂的 Agent 框架
