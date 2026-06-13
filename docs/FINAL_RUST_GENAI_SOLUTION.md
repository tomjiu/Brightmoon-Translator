# 最终方案：rust-genai - 纯 Rust AI 解决方案

生成时间: 2026-06-13
结论: **rust-genai 是最佳选择** - 成熟、活跃、功能完整

---

## ✅ 为什么选择 rust-genai？

### 核心优势
1. ✅ **纯 Rust**，无 Python 依赖
2. ✅ **Structured Output 原生支持**（JsonSpec）
3. ✅ **25+ 提供商**（OpenAI/Anthropic/DeepSeek/Ollama...）
4. ✅ **活跃维护**（799⭐，889 commits，v0.6.0 刚发布）
5. ✅ **轻量级**（无SDK依赖，原生协议）
6. ✅ **简洁 API**（统一接口，易用）

### 与其他方案对比

| 方案 | 优势 | 劣势 | 推荐度 |
|------|------|------|--------|
| **rust-genai** | ✅ Structured Output<br>✅ 25+提供商<br>✅ 活跃维护 | 无 | ⭐⭐⭐⭐⭐ |
| rig | ✅ 轻量<br>✅ extract 方法 | ⚠️ 相对新<br>⚠️ 文档少 | ⭐⭐⭐⭐ |
| open-agent-sdk-rust | ✅ 完整工具系统 | ❌ 无 Structured Output | ⭐⭐⭐ |
| llm-chain-rs | ✅ Chain 支持 | ⚠️ 维护不活跃 | ⭐⭐ |
| Instructor + FastAPI | ✅ 功能强大 | ❌ Python 依赖 | ⭐⭐ |
| 手动实现 | ✅ 完全可控 | ⚠️ 代码冗长 | ⭐⭐⭐ |

---

## 🚀 完整实现方案

### Step 1: 添加依赖

```toml
# src-tauri/Cargo.toml

[dependencies]
genai = "0.6"  # rust-genai
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
```

---

### Step 2: 定义数据结构

```rust
// src-tauri/src/ai/models.rs

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WordCard {
    pub phonetic: String,
    pub part_of_speech: String,
    pub definitions: Vec<String>,
    
    #[serde(default)]
    #[schemars(length(min = 3, max = 5))]
    pub examples: Vec<String>,
    
    pub etymology: Option<String>,
    pub mnemonic: String,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
    pub related_words: Vec<String>,
    
    #[schemars(regex = "^(A1|A2|B1|B2|C1|C2)$")]
    pub difficulty_level: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LearningPlan {
    #[schemars(length(max = 5))]
    pub today_words: Vec<String>,
    
    #[schemars(length(max = 10))]
    pub review_words: Vec<String>,
    
    #[schemars(range(min = 5, max = 60))]
    pub estimated_minutes: u32,
    
    pub reasoning: String,
}
```

**关键点**：
- 用 `schemars` 自动生成 JSON Schema
- 添加验证约束（长度/范围/正则）

---

### Step 3: 实现 AI 生成函数

```rust
// src-tauri/src/ai/word_card_generator.rs

use genai::chat::{ChatMessage, ChatRequest, ChatOptions};
use genai::chat::{ChatResponseFormat, JsonSpec};
use genai::Client;
use serde_json::json;
use schemars::schema_for;

use super::models::WordCard;

pub async fn generate_word_card(
    api_key: &str,
    model: &str,  // "gpt-4" or "deepseek-chat"
    word: &str,
    source_lang: &str,
    target_lang: &str,
) -> Result<WordCard, String> {
    // 1. 创建 genai Client
    let client = Client::builder()
        .with_auth_env_name("GENAI_API_KEY")  // 或直接 .with_auth_key(api_key)
        .build()
        .map_err(|e| e.to_string())?;
    
    // 2. 构建详细 Prompt
    let system_prompt = "你是专业的语言学习助手。严格按照 JSON Schema 返回结果。";
    
    let user_prompt = format!(
        r#"为单词 "{word}" 生成完整的学习卡片。

要求：
1. 提供准确的音标（IPA 格式）、词性、释义
2. 生成 3-5 个实用例句（从易到难）
3. 如果有词根/词源，简要说明（不超过 50 字）
4. 提供一个生动的助记法（联想/谐音/词根拆解）
5. 列出 2-3 个同义词和反义词
6. 列出同词根的相关词（如 architecture → architect, architectural）
7. 标注 CEFR 难度等级（A1/A2/B1/B2/C1/C2）

单词: {word}
源语言: {source_lang}
目标语言: {target_lang}"#
    );
    
    // 3. 生成 JSON Schema（自动）
    let schema = schema_for!(WordCard);
    let schema_json = serde_json::to_value(&schema)
        .map_err(|e| format!("Schema 序列化失败: {}", e))?;
    
    // 4. 创建 JsonSpec
    let json_spec = JsonSpec::new("word_card", schema_json)
        .with_description("Language learning word card with examples and mnemonics");
    
    // 5. 构建 ChatRequest
    let chat_req = ChatRequest::new(vec![
        ChatMessage::system(system_prompt),
        ChatMessage::user(user_prompt),
    ]);
    
    // 6. 设置 ChatOptions（强制 Structured Output）
    let chat_opts = ChatOptions {
        temperature: Some(0.7),
        response_format: Some(ChatResponseFormat::JsonSpec(json_spec)),
        ..Default::default()
    };
    
    // 7. 调用 LLM
    let chat_res = client
        .exec_chat(model, chat_req, Some(&chat_opts))
        .await
        .map_err(|e| format!("LLM 调用失败: {}", e))?;
    
    // 8. 解析响应
    let content = chat_res
        .content_text_as_str()
        .ok_or("响应缺少文本内容")?;
    
    let card: WordCard = serde_json::from_str(content)
        .map_err(|e| format!("JSON 解析失败: {}", e))?;
    
    // 9. 手动验证关键约束
    if card.examples.len() < 3 {
        return Err(format!("例句数量不足：{} < 3", card.examples.len()));
    }
    
    if !["A1", "A2", "B1", "B2", "C1", "C2"].contains(&card.difficulty_level.as_str()) {
        return Err(format!("无效难度等级: {}", card.difficulty_level));
    }
    
    Ok(card)
}
```

---

### Step 4: Tauri 命令集成

```rust
// src-tauri/src/commands/ai_cmd.rs

use crate::ai::word_card_generator::generate_word_card;
use crate::ai::models::WordCard;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn generate_ai_word_card(
    state: State<'_, AppState>,
    word: String,
    source_lang: String,
    target_lang: String,
) -> Result<WordCard, String> {
    let config = state.config.lock().await;
    
    // 从配置获取 API Key 和模型
    let api_key = &config.llm.api_key;
    let model = &config.llm.model;
    
    if api_key.is_empty() {
        return Err("未配置 LLM API Key，请前往设置页面配置".to_string());
    }
    
    // 调用生成函数
    generate_word_card(api_key, model, &word, &source_lang, &target_lang).await
}

// 注册命令
// src-tauri/src/lib.rs
tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
        // ... 其他命令
        commands::ai_cmd::generate_ai_word_card,
    ])
```

---

### Step 5: 前端调用

```typescript
// src/pages/Dictionary.tsx

import { invokeOrThrow } from '../services/invoke';

interface WordCard {
  phonetic: string;
  partOfSpeech: string;
  definitions: string[];
  examples: string[];
  etymology?: string;
  mnemonic: string;
  synonyms: string[];
  antonyms: string[];
  relatedWords: string[];
  difficultyLevel: string;
}

async function handleGenerateCard(word: string) {
  setGenerating(true);
  try {
    const card = await invokeOrThrow<WordCard>('generate_ai_word_card', {
      word,
      sourceLang: 'en',
      targetLang: 'zh',
    });
    
    // 显示卡片
    setCurrentCard(card);
    
    // 保存到 wordbook（更新 aiContent 字段）
    await invokeOrThrow('update_wordbook_ai_content', {
      id: wordId,
      aiContent: card,
    });
    
  } catch (err) {
    console.error('生成失败:', err);
    toast.error(`生成失败: ${err}`);
  } finally {
    setGenerating(false);
  }
}
```

---

## 📊 成本估算

### 使用 DeepSeek API
- 每个单词卡片 Prompt: ~600 tokens
- 每个单词卡片输出: ~800 tokens
- DeepSeek 价格: ¥0.001/1k tokens (输入) + ¥0.002/1k tokens (输出)
- **单个单词成本**: (600×0.001 + 800×0.002) / 1000 = ¥0.0022

**1000 个单词 = ¥2.2**

### 使用 OpenAI GPT-4
- 价格: $0.03/1k tokens (输入) + $0.06/1k tokens (输出)
- **单个单词成本**: (600×0.03 + 800×0.06) / 1000 = $0.066 ≈ ¥0.47

**1000 个单词 = ¥470** （贵很多，推荐用 DeepSeek）

---

## 🎯 实施计划

### Week 1: 基础集成（3-5 天）
1. ✅ 添加 genai 依赖
2. ✅ 实现 `generate_word_card` 函数
3. ✅ 测试 OpenAI/DeepSeek API
4. ✅ Tauri 命令注册

### Week 2: 前端连接（2-3 天）
5. ✅ Dictionary 页面集成
6. ✅ 单词卡片 UI 组件
7. ✅ 本地缓存（避免重复生成）
8. ✅ 错误处理和重试

### Week 3: 学习系统（1 周）
9. ✅ 集成 FSRS 算法（`fsrs-rs` crate）
10. ✅ 实现 `generate_learning_plan`（同样用 genai）
11. ✅ 闪卡学习界面
12. ✅ 熟练度追踪

---

## 💡 关键优势

### 1. 类型安全
```rust
// schemars 自动生成 JSON Schema，编译期检查
let schema = schema_for!(WordCard);
```

### 2. 多提供商支持
```rust
// 轻松切换提供商
client.exec_chat("gpt-4", ...)           // OpenAI
client.exec_chat("deepseek-chat", ...)   // DeepSeek
client.exec_chat("claude-3", ...)        // Anthropic
client.exec_chat("ollama::llama3", ...)  // 本地 Ollama
```

### 3. 自动验证
- JSON Schema 在 LLM 端约束输出格式
- Rust serde 反序列化时二次验证
- 手动验证业务规则

---

## 🔗 参考资源

- **rust-genai GitHub**: https://github.com/jeremychone/rust-genai
- **rust-genai 文档**: https://docs.rs/genai
- **schemars（JSON Schema 生成）**: https://github.com/GREsau/schemars
- **fsrs-rs（间隔重复算法）**: https://github.com/open-spaced-repetition/fsrs-rs

---

## 📝 总结

**rust-genai 是完美的选择**：
- ✅ 纯 Rust，无额外依赖
- ✅ Structured Output 原生支持
- ✅ 25+ 提供商，灵活切换
- ✅ 活跃维护，生产可用
- ✅ API 简洁，易于集成

**下一步行动**：
1. 今天添加 genai 依赖并测试基础功能
2. 明天实现 `generate_word_card` 完整逻辑
3. 周末完成前端集成

**2-3 周内即可完成完整的 AI 学习系统！** 🚀
