# AI Agent 驱动的智能学习系统 - 轻量级方案

生成时间: 2026-06-13
目标: 用 LLM Agent 生成高质量单词卡内容（例句、词根、助记法），无需复杂框架

---

## 🎯 核心理念

**不需要 Codex 那种重量级数据集**。我们用：
1. **单个 LLM API** (OpenAI/DeepSeek) 生成所有内容
2. **结构化 Prompt** 一次性生成完整卡片
3. **本地缓存** 避免重复调用
4. **简单 Agent 模式** 而非复杂工作流

---

## 📊 当前问题

### Dictionary 页面现状
```typescript
// src/pages/Dictionary.tsx
const [entries] = useState<VocabularyEntry[]>([/* mock 数据 */]);
// ❌ 未连接后端 wordbook
// ❌ 只有基础字段（word, translation, examples）
// ❌ 无 AI 增强内容（词根、助记法、相关词）
```

### WordBookItem 现状
```rust
// src-tauri/src/models/memory.rs
pub struct WordBookItem {
    pub id: String,
    pub word: String,
    pub translation: String,
    pub from_lang: String,
    pub to_lang: String,
    pub note: String,        // ❌ 只有用户手动笔记
    pub timestamp: i64,
}
// ❌ 缺少：例句、词根、助记法、相关词、熟练度
```

---

## 🚀 轻量级方案：3 步搞定

### Step 1: 扩展 WordBookItem 数据结构

```rust
// src-tauri/src/models/memory.rs

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordBookItem {
    // ── 基础信息 ──
    pub id: String,
    pub word: String,
    pub translation: String,
    pub from_lang: String,
    pub to_lang: String,
    pub timestamp: i64,
    
    // ── AI 增强内容 ──
    pub ai_content: Option<AiEnhancedContent>, // LLM 生成的内容
    
    // ── 学习数据 ──
    pub proficiency: i32,           // 熟练度 0-100
    pub review_count: i32,          // 复习次数
    pub last_review_at: Option<i64>, // 上次复习时间
    pub next_review_at: Option<i64>, // 下次复习时间（SM-2 算法）
    
    // ── 上下文 ──
    pub source_context: Option<String>, // 原始句子/网页 URL
    pub user_note: String,              // 用户笔记
    pub tags: Vec<String>,              // 标签
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEnhancedContent {
    pub phonetic: String,              // 音标
    pub part_of_speech: String,        // 词性
    pub definitions: Vec<String>,      // 多个释义
    pub examples: Vec<String>,         // AI 生成的例句（3-5 个）
    pub etymology: Option<String>,     // 词源/词根
    pub mnemonic: Option<String>,      // 助记法
    pub synonyms: Vec<String>,         // 同义词
    pub antonyms: Vec<String>,         // 反义词
    pub related_words: Vec<String>,    // 相关词（同词根）
    pub usage_notes: Option<String>,   // 用法说明
    pub difficulty_level: String,      // 难度（A1-C2 / 初中高）
}
```

---

### Step 2: 单个 LLM 调用生成所有内容

**核心思想**：一次性 Prompt 生成完整卡片，避免多次 API 调用

```rust
// src-tauri/src/commands/ai_cmd.rs

#[tauri::command]
pub async fn generate_word_card(
    state: State<'_, AppState>,
    word: String,
    source_lang: String, // "en" | "ja" 等
    target_lang: String, // "zh"
) -> Result<AiEnhancedContent, String> {
    let config = state.config.lock().await;
    
    // 构建结构化 Prompt
    let prompt = build_word_card_prompt(&word, &source_lang, &target_lang);
    
    // 调用 LLM（强制 JSON 输出）
    let response = crate::engine::llm::call_llm_json(
        &config.llm,
        &prompt,
    ).await.map_err(|e| e.to_string())?;
    
    // 解析 JSON
    let content: AiEnhancedContent = serde_json::from_str(&response)
        .map_err(|e| format!("JSON 解析失败: {}", e))?;
    
    Ok(content)
}

fn build_word_card_prompt(word: &str, source: &str, target: &str) -> String {
    let lang_name = match source {
        "en" => "英语",
        "ja" => "日语",
        "ko" => "韩语",
        _ => "外语",
    };
    
    format!(r#"
你是一个专业的语言学习助手。为单词 "{word}" 生成完整的学习卡片。

要求：
1. 提供准确的音标、词性、释义
2. 生成 3-5 个实用例句（从易到难）
3. 如果有词根/词源，简要说明（不超过 50 字）
4. 提供一个生动的助记法（联想/谐音/词根拆解）
5. 列出 2-3 个同义词和反义词
6. 列出同词根的相关词（如 architecture → architect, architectural）
7. 标注 CEFR 难度等级（A1/A2/B1/B2/C1/C2）

严格按以下 JSON 格式返回（不要有 markdown 代码块）：

{{
  "phonetic": "/fəˈnetɪk/",
  "partOfSpeech": "n./v./adj.",
  "definitions": ["释义1", "释义2"],
  "examples": [
    "例句1（简单）",
    "例句2（中等）",
    "例句3（复杂）"
  ],
  "etymology": "来自拉丁语 architectura，由 archi-(主要) + tectura(建造) 组成",
  "mnemonic": "architecture = archi(拱门) + tecture(建造) → 建造拱门的学问 → 建筑学",
  "synonyms": ["同义词1", "同义词2"],
  "antonyms": ["反义词1"],
  "relatedWords": ["相关词1", "相关词2"],
  "usageNotes": "正式场合使用，可用于抽象概念（如软件架构）",
  "difficultyLevel": "B2"
}}

单词: {word}
源语言: {lang_name}
目标语言: 中文
"#)
}
```

**关键优化**：
- ✅ 一次 API 调用生成所有内容
- ✅ 强制 JSON 输出（OpenAI `response_format: json_object`）
- ✅ 详细 Prompt 保证内容质量
- ✅ 无需复杂的 Agent 框架

---

### Step 3: 前端 Dictionary 页面连接真实数据

```typescript
// src/pages/Dictionary.tsx

import { useEffect, useState } from 'react';
import { invokeOrThrow } from '../services/invoke';

interface WordBookItem {
  id: string;
  word: string;
  translation: string;
  fromLang: string;
  toLang: string;
  timestamp: number;
  aiContent?: AiEnhancedContent; // AI 增强内容
  proficiency: number;
  reviewCount: number;
  lastReviewAt?: number;
  sourceContext?: string;
  userNote: string;
  tags: string[];
}

interface AiEnhancedContent {
  phonetic: string;
  partOfSpeech: string;
  definitions: string[];
  examples: string[];
  etymology?: string;
  mnemonic?: string;
  synonyms: string[];
  antonyms: string[];
  relatedWords: string[];
  usageNotes?: string;
  difficultyLevel: string;
}

export default function Dictionary() {
  const [entries, setEntries] = useState<WordBookItem[]>([]);
  const [selectedEntry, setSelectedEntry] = useState<WordBookItem | null>(null);
  const [loading, setLoading] = useState(true);
  const [generating, setGenerating] = useState(false);

  // 加载生词本
  useEffect(() => {
    loadWordbook();
  }, []);

  const loadWordbook = async () => {
    try {
      const data = await invokeOrThrow<WordBookItem[]>('get_wordbook');
      setEntries(data);
    } catch (err) {
      console.error('Failed to load wordbook:', err);
    } finally {
      setLoading(false);
    }
  };

  // 为单词生成 AI 内容
  const generateAiContent = async (word: string, fromLang: string, toLang: string) => {
    setGenerating(true);
    try {
      const content = await invokeOrThrow<AiEnhancedContent>('generate_word_card', {
        word,
        sourceLang: fromLang,
        targetLang: toLang,
      });
      
      // 更新生词本条目
      // TODO: 需要添加 update_wordbook_ai_content 命令
      
      return content;
    } catch (err) {
      console.error('AI generation failed:', err);
      throw err;
    } finally {
      setGenerating(false);
    }
  };

  return (
    <div className="dictionary-page">
      {/* 左侧：生词列表 */}
      <div className="word-list">
        {entries.map(entry => (
          <div 
            key={entry.id}
            className="word-item"
            onClick={() => setSelectedEntry(entry)}
          >
            <span className="word">{entry.word}</span>
            <span className="translation">{entry.translation}</span>
            {!entry.aiContent && (
              <button 
                className="generate-btn"
                onClick={(e) => {
                  e.stopPropagation();
                  generateAiContent(entry.word, entry.fromLang, entry.toLang);
                }}
              >
                ✨ 生成卡片
              </button>
            )}
          </div>
        ))}
      </div>

      {/* 右侧：单词详情卡片 */}
      <div className="word-detail">
        {selectedEntry ? (
          <WordCard entry={selectedEntry} />
        ) : (
          <p>选择一个单词查看详情</p>
        )}
      </div>
    </div>
  );
}

// 单词卡片组件
function WordCard({ entry }: { entry: WordBookItem }) {
  const { aiContent } = entry;

  if (!aiContent) {
    return (
      <div className="empty-card">
        <h2>{entry.word}</h2>
        <p>{entry.translation}</p>
        <button>✨ 生成 AI 学习卡片</button>
      </div>
    );
  }

  return (
    <div className="word-card">
      {/* 基础信息 */}
      <div className="header">
        <h1>{entry.word}</h1>
        <span className="phonetic">{aiContent.phonetic}</span>
        <span className="pos">{aiContent.partOfSpeech}</span>
        <span className="level">{aiContent.difficultyLevel}</span>
      </div>

      {/* 释义 */}
      <section className="definitions">
        <h3>📖 释义</h3>
        <ol>
          {aiContent.definitions.map((def, i) => (
            <li key={i}>{def}</li>
          ))}
        </ol>
      </section>

      {/* 例句 */}
      <section className="examples">
        <h3>💬 例句</h3>
        {aiContent.examples.map((ex, i) => (
          <div key={i} className="example">
            <span className="bullet">•</span>
            <span>{ex}</span>
          </div>
        ))}
      </section>

      {/* 词根词源 */}
      {aiContent.etymology && (
        <section className="etymology">
          <h3>🌱 词根词源</h3>
          <p>{aiContent.etymology}</p>
        </section>
      )}

      {/* 助记法 */}
      {aiContent.mnemonic && (
        <section className="mnemonic">
          <h3>💡 助记法</h3>
          <p className="highlight">{aiContent.mnemonic}</p>
        </section>
      )}

      {/* 同义词/反义词 */}
      <section className="synonyms-antonyms">
        <div>
          <h4>👥 同义词</h4>
          <div className="tags">
            {aiContent.synonyms.map((syn, i) => (
              <span key={i} className="tag">{syn}</span>
            ))}
          </div>
        </div>
        <div>
          <h4>⚡ 反义词</h4>
          <div className="tags">
            {aiContent.antonyms.map((ant, i) => (
              <span key={i} className="tag">{ant}</span>
            ))}
          </div>
        </div>
      </section>

      {/* 相关词 */}
      {aiContent.relatedWords.length > 0 && (
        <section className="related">
          <h3>🔗 相关词</h3>
          <div className="tags">
            {aiContent.relatedWords.map((word, i) => (
              <span key={i} className="tag related">{word}</span>
            ))}
          </div>
        </section>
      )}

      {/* 用法说明 */}
      {aiContent.usageNotes && (
        <section className="usage">
          <h3>📝 用法说明</h3>
          <p>{aiContent.usageNotes}</p>
        </section>
      )}

      {/* 学习进度 */}
      <section className="progress">
        <h3>📊 学习进度</h3>
        <div className="progress-bar">
          <div 
            className="fill" 
            style={{ width: `${entry.proficiency}%` }}
          />
        </div>
        <p>熟练度: {entry.proficiency}% | 复习次数: {entry.reviewCount}</p>
      </section>
    </div>
  );
}
```

---

## 🎨 用户体验流程

### 场景 1: 浏览器扩展添加生词
```
1. 用户在网页划词翻译 "architecture"
2. 点击 ⭐ 收藏按钮
3. 后端调用: add_wordbook_entry(word, translation, ...)
4. 存储基础信息（无 AI 内容）
```

### 场景 2: 桌面端生成学习卡片
```
1. 打开 Dictionary 页面，看到生词列表
2. 点击 "architecture"，看到基础信息
3. 点击 "✨ 生成 AI 学习卡片"
4. 调用: generate_word_card("architecture", "en", "zh")
5. LLM 返回完整 JSON（1-2 秒）
6. 显示：
   - 音标: /ˈɑːkɪtektʃə/
   - 词根: archi-(主要) + tectura(建造)
   - 助记: architecture = 拱门建造学问 → 建筑学
   - 例句: "Software architecture is important."
   - 同义词: design, structure
   - 相关词: architect, architectural
```

### 场景 3: 闪卡学习模式
```
1. 进入学习模式，看到 "architecture"
2. 尝试回忆释义
3. 点击"显示答案"，看到完整卡片
4. 选择 ✅认识 / ❌不认识
5. 系统更新熟练度和下次复习时间
```

---

## 💾 数据库迁移

```rust
// src-tauri/src/memory.rs - 更新 WordBook 存储

impl WordBook {
    // 新增：更新 AI 内容
    pub fn update_ai_content(&self, id: &str, content: AiEnhancedContent) -> Result<(), String> {
        let mut items = self.items.lock().unwrap();
        if let Some(item) = items.iter_mut().find(|i| i.id == id) {
            item.ai_content = Some(content);
            self.save_to_disk()?;
            Ok(())
        } else {
            Err("Word not found".to_string())
        }
    }
    
    // 新增：批量生成 AI 内容
    pub async fn batch_generate_ai_content(
        &self,
        llm_config: &LlmConfig,
    ) -> Result<Vec<String>, String> {
        let items = self.items.lock().unwrap();
        let words_to_generate: Vec<_> = items.iter()
            .filter(|item| item.ai_content.is_none())
            .map(|item| item.id.clone())
            .collect();
        
        // 返回待生成的单词 ID 列表
        Ok(words_to_generate)
    }
}
```

---

## 🚀 优势对比

### ❌ 不采用 Codex 方案的原因
- 需要大规模数据集（几十万单词预生成）
- 需要向量数据库（Pinecone/Weaviate）
- 需要复杂的评估框架
- 冷启动成本高

### ✅ 我们的轻量级方案
- **按需生成**: 只为用户添加的单词生成卡片（节省成本）
- **一次 API 调用**: 生成所有内容，2-3 秒完成
- **本地缓存**: 生成后永久保存，无需重复调用
- **渐进增强**: 基础功能（翻译+收藏）立即可用，AI 卡片可选生成

---

## 📊 成本估算

### LLM API 调用成本（以 DeepSeek 为例）
- 每个单词卡片 Prompt: ~500 tokens
- 每个单词卡片输出: ~800 tokens
- DeepSeek 价格: ¥0.001/1k tokens (输入) + ¥0.002/1k tokens (输出)
- **单个单词成本**: ~0.002 元

**用户收藏 1000 个单词，全部生成 AI 卡片 = ¥2**

---

## 🎯 实现优先级

### Phase 1: 基础连接（3-5 天）
1. ✅ 扩展 `WordBookItem` 数据结构
2. ✅ 实现 `generate_word_card` 命令
3. ✅ Dictionary 页面连接真实 wordbook
4. ✅ 显示基础单词列表（无 AI 内容）

### Phase 2: AI 卡片生成（1 周）
5. ✅ 单词详情页 "生成 AI 卡片" 按钮
6. ✅ 显示完整卡片（词根/例句/助记法）
7. ✅ 本地缓存 AI 内容
8. ✅ 批量生成功能（后台任务）

### Phase 3: 学习模式（1 周）
9. ✅ 闪卡学习界面
10. ✅ 熟练度更新算法
11. ✅ 今日学习计划（基于 AI 内容）

---

## 🔍 Prompt 优化技巧

### 技巧 1: 强制 JSON 输出
```rust
// OpenAI API
let request = json!({
    "model": "gpt-4",
    "messages": [...],
    "response_format": { "type": "json_object" }, // 强制 JSON
    "temperature": 0.7
});

// DeepSeek 同样支持
```

### 技巧 2: Few-Shot 示例
```rust
let prompt = format!(r#"
以下是两个示例：

示例1:
单词: brilliant
输出: {{
  "phonetic": "/ˈbrɪliənt/",
  "mnemonic": "brill(闪耀) + iant → 闪耀的 → 出色的"
}}

示例2:
单词: abandon
输出: {{
  "phonetic": "/əˈbændən/",
  "mnemonic": "a(离开) + band(乐队) + on → 离开乐队 → 放弃"
}}

现在为以下单词生成卡片：
单词: {word}
"#);
```

### 技巧 3: 分步生成（可选）
如果单次 Token 超限，可拆分：
```rust
// 第1次调用：生成基础信息 + 例句
// 第2次调用：生成词根 + 助记法
// 合并结果
```

---

## 📝 总结

### 为什么不需要 Codex？
- Codex 是**大规模评估框架** + **预训练数据集**
- 我们只需要**为用户单词生成卡片**，按需生成即可

### 我们的方案核心
1. **轻量级**: 单个 LLM API + 结构化 Prompt
2. **按需生成**: 用户点击才生成，成本可控
3. **高质量**: 详细 Prompt 保证内容专业性
4. **渐进增强**: 基础功能立即可用，AI 可选

### 下一步
1. 先完成 Dictionary 页面连接真实 wordbook（1-2 天）
2. 再实现 `generate_word_card` 命令（2-3 天）
3. 最后加上学习模式（1 周）

**总开发时间: 2-3 周完成完整 AI 学习系统**
