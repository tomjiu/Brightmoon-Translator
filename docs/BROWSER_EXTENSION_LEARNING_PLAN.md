# 浏览器扩展 AI 学习功能集成方案

生成时间: 2026-06-13
目标: 实现扩展翻译记录 → 桌面端生词本 → AI 智能学习计划的完整闭环

---

## 📊 现状分析

### ✅ 已实现
1. **桌面端 API Server** (`http://127.0.0.1:60828`)
   - ✅ 翻译接口: `/translate`, `/browser/translate`
   - ✅ 配置接口: `/config` (GET/POST)
   - ✅ 历史接口: `/history` (GET)
   - ✅ 缓存接口: `/cache/stats`, `/cache/clear`
   - ⚠️ **缺失**: 生词本接口未暴露给 HTTP API

2. **桌面端生词本命令** (Tauri commands，仅桌面端可用)
   - ✅ `add_wordbook_entry` - 添加生词
   - ✅ `get_wordbook` - 获取生词列表
   - ✅ `update_wordbook_note` - 更新笔记
   - ✅ `delete_wordbook_entry` / `batch_delete_wordbook` - 删除
   - ✅ `search_wordbook` - 搜索
   - ✅ `export_wordbook_csv` - 导出

3. **浏览器扩展 Desktop Bridge**
   - ✅ 连接检测: `DesktopBridge.checkHealth()`
   - ✅ 翻译调用: `translateViaDesktop(text, from, to)`
   - ❌ **未记录历史**: 翻译结果直接返回，未调用 `/history` API
   - ❌ **未生词本集成**: 无收藏按钮，无生词本 API

### ❌ 缺失环节
1. 扩展翻译后**不记录历史**到桌面端
2. 扩展**无生词本功能**（无收藏按钮、无生词列表）
3. 桌面端生词本命令**未暴露 HTTP API**
4. AI 学习计划**完全未实现**

---

## 🎯 完整方案：3 阶段实现

### Phase 1: 数据打通（1-2 周）

#### 1.1 后端：暴露生词本 HTTP API

**目标**: 让浏览器扩展能访问生词本

```rust
// src-tauri/src/api_server.rs

// 新增路由
.route("/wordbook", get(get_wordbook).post(add_wordbook))
.route("/wordbook/:id", put(update_wordbook_note).delete(delete_wordbook))
.route("/wordbook/search", post(search_wordbook))

// 实现 handlers
async fn get_wordbook(AxumState(state): AxumState<ApiState>) -> impl IntoResponse {
    let store = state.wordbook_store.lock().await;
    let items = store.get_all().unwrap_or_default();
    Json(items).into_response()
}

async fn add_wordbook(
    AxumState(state): AxumState<ApiState>,
    Json(req): Json<AddWordRequest>
) -> impl IntoResponse {
    // req: { word, translation, sourceText, sourceLang, targetLang, context }
    let mut store = state.wordbook_store.lock().await;
    let entry = WordBookItem {
        id: uuid::Uuid::new_v4().to_string(),
        word: req.word,
        translation: req.translation,
        source_text: req.source_text,
        source_lang: req.source_lang,
        target_lang: req.target_lang,
        context: req.context,
        added_at: chrono::Utc::now().timestamp(),
        review_count: 0,
        last_review_at: None,
        proficiency: 0, // 0-100
        tags: vec![],
    };
    store.add(entry.clone()).map_err(|e| ...)?;
    Json(entry).into_response()
}
```

**需要添加的字段**（如果 `WordBookItem` 缺失）:
```rust
// src-tauri/src/models/wordbook.rs
pub struct WordBookItem {
    pub id: String,
    pub word: String,
    pub translation: String,
    pub source_text: String,      // 原始句子
    pub source_lang: String,       // 源语言
    pub target_lang: String,       // 目标语言
    pub context: Option<String>,   // 网页 URL 或上下文
    pub added_at: i64,             // 添加时间戳
    pub review_count: i32,         // 复习次数
    pub last_review_at: Option<i64>, // 上次复习时间
    pub proficiency: i32,          // 熟练度 0-100
    pub tags: Vec<String>,         // 标签（如 "技术"、"日常"）
    pub notes: Option<String>,     // 用户笔记
}
```

#### 1.2 后端：自动记录翻译历史

**目标**: 扩展翻译时自动保存历史

```rust
// src-tauri/src/api_server.rs - 修改现有 browser_translate handler

async fn browser_translate(
    AxumState(state): AxumState<ApiState>,
    Json(req): Json<BrowserTranslateRequest>
) -> impl IntoResponse {
    // ... 现有翻译逻辑 ...
    let response = handle_browser_request(&req, &state).await?;
    
    // 新增：自动记录历史
    let history_item = HistoryItem {
        id: uuid::Uuid::new_v4().to_string(),
        source_text: req.payload.data.text.clone(),
        translated_text: response.results[0].text.clone(),
        source_lang: req.from.clone(),
        target_lang: req.to.clone(),
        engine: response.results[0].engine.clone(),
        timestamp: chrono::Utc::now().timestamp(),
        source: "browser_extension".to_string(), // 标记来源
        context: Some(req.payload.data.url.clone()), // 网页 URL
    };
    
    let mut history = state.history_store.lock().await;
    let _ = history.add(history_item); // 忽略错误，不阻塞翻译
    
    Json(response).into_response()
}
```

#### 1.3 前端：扩展添加"收藏"按钮

**目标**: 划词翻译弹窗添加收藏到生词本功能

```javascript
// extension/content/hover-translator.js

function createTranslationPopup(text, translation, detectedLang) {
  const popup = document.createElement('div');
  popup.className = 'moon-translator-popup';
  popup.innerHTML = `
    <div class="result">
      <div class="source">${escapeHtml(text)}</div>
      <div class="translation">${escapeHtml(translation)}</div>
      <div class="lang-hint">${detectedLang || 'auto'} → zh</div>
    </div>
    <div class="actions">
      <button class="copy-btn" title="复制">📋</button>
      <button class="star-btn" title="收藏到生词本">⭐</button>
      <button class="close-btn" title="关闭">✕</button>
    </div>
  `;
  
  // 收藏按钮事件
  popup.querySelector('.star-btn').addEventListener('click', async () => {
    const success = await addToWordbook({
      word: text,
      translation: translation,
      sourceText: text,
      sourceLang: detectedLang || 'auto',
      targetLang: 'zh',
      context: window.location.href
    });
    
    if (success) {
      popup.querySelector('.star-btn').textContent = '✅';
      popup.querySelector('.star-btn').disabled = true;
    }
  });
  
  return popup;
}

// 调用桌面端生词本 API
async function addToWordbook(entry) {
  try {
    const resp = await fetch('http://127.0.0.1:60828/wordbook', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(entry)
    });
    return resp.ok;
  } catch (err) {
    console.error('Failed to add to wordbook:', err);
    return false;
  }
}
```

#### 1.4 前端：扩展 Popup 显示生词本预览

```javascript
// extension/popup/popup.js

async function loadWordbookPreview() {
  try {
    const resp = await fetch('http://127.0.0.1:60828/wordbook', {
      method: 'GET',
      signal: AbortSignal.timeout(3000)
    });
    
    if (!resp.ok) throw new Error('Desktop not connected');
    
    const wordbook = await resp.json();
    const recent = wordbook.slice(0, 5); // 最近 5 个
    
    const listEl = document.getElementById('wordbookPreview');
    listEl.innerHTML = recent.map(item => `
      <div class="word-item">
        <span class="word">${item.word}</span>
        <span class="translation">${item.translation}</span>
      </div>
    `).join('');
    
    document.getElementById('wordbookCount').textContent = `${wordbook.length} 个生词`;
    
  } catch (err) {
    document.getElementById('wordbookPreview').innerHTML = 
      '<p class="error">桌面端未连接，无法显示生词本</p>';
  }
}

// 页面加载时调用
document.addEventListener('DOMContentLoaded', () => {
  // ... 现有逻辑 ...
  loadWordbookPreview();
});
```

---

### Phase 2: AI 学习计划（2-3 周）

#### 2.1 后端：学习计划生成

**目标**: 基于用户生词本数据，LLM 生成个性化学习计划

```rust
// src-tauri/src/commands/ai_cmd.rs

#[derive(Serialize, Deserialize)]
pub struct LearningPlan {
    pub today_words: Vec<String>,      // 今日学习单词
    pub review_words: Vec<String>,     // 今日复习单词
    pub difficulty_level: String,      // 难度等级（简单/中等/困难）
    pub estimated_minutes: u32,        // 预计学习时长
    pub plan_date: String,             // 计划日期
    pub reasoning: String,             // AI 推理说明
}

#[tauri::command]
pub async fn generate_learning_plan(
    state: State<'_, AppState>
) -> Result<LearningPlan, String> {
    // 1. 获取生词本数据
    let wordbook = state.wordbook_store.lock().await;
    let words = wordbook.get_all().map_err(|e| e.to_string())?;
    
    // 2. 分析用户学习数据
    let low_proficiency: Vec<_> = words.iter()
        .filter(|w| w.proficiency < 50)
        .collect();
    
    let needs_review: Vec<_> = words.iter()
        .filter(|w| {
            let days_since_review = w.last_review_at.map(|t| {
                (chrono::Utc::now().timestamp() - t) / 86400
            }).unwrap_or(999);
            
            // 根据熟练度调整复习间隔（艾宾浩斯曲线）
            let interval = match w.proficiency {
                0..=30 => 1,    // 1 天
                31..=60 => 3,   // 3 天
                61..=80 => 7,   // 7 天
                _ => 14,        // 14 天
            };
            
            days_since_review >= interval
        })
        .collect();
    
    // 3. 构建 LLM Prompt
    let prompt = format!(
        "你是一个专业的语言学习助手。请根据以下数据为用户生成今日学习计划：

**生词本统计**:
- 总单词数: 
- 低熟练度单词（< 50%）: {}
- 需要复习的单词: {}

**单词详情**（熟练度 < 50%）:
{}

**复习单词**:
{}

请生成一个科学的学习计划，包括：
1. 今日学习的新单词（3-5个，从低熟练度中选择）
2. 今日复习的旧单词（5-10个，从需复习中选择）
3. 预计学习时长（分钟）
4. 简短的学习建议

以 JSON 格式返回：
{{
  \"today_words\": [\"word1\", \"word2\", ...],
  \"review_words\": [\"word3\", \"word4\", ...],
  \"estimated_minutes\": 15,
  \"reasoning\": \"根据你的学习进度...\"
}}",
        words.len(),
        low_proficiency.len(),
        needs_review.len(),
        low_proficiency.iter().take(10)
            .map(|w| format!("- {} (熟练度: {}%, 复习{}次)", w.word, w.proficiency, w.review_count))
            .collect::<Vec<_>>()
            .join("\n"),
        needs_review.iter().take(10)
            .map(|w| format!("- {} (上次复习: {}天前)", w.word, 
                w.last_review_at.map(|t| (chrono::Utc::now().timestamp() - t) / 86400)
                    .unwrap_or(999)))
            .collect::<Vec<_>>()
            .join("\n")
    );
    
    // 4. 调用 LLM
    let config = state.config.lock().await;
    let llm_response = crate::engine::llm::call_llm(
        &config.llm,
        &prompt,
        None, // 无流式
    ).await.map_err(|e| e.to_string())?;
    
    // 5. 解析 JSON 响应
    let plan: LearningPlan = serde_json::from_str(&llm_response)
        .map_err(|e| format!("LLM 响应解析失败: {}", e))?;
    
    Ok(plan)
}
```

#### 2.2 后端：学习进度更新

```rust
#[tauri::command]
pub async fn update_word_progress(
    state: State<'_, AppState>,
    word_id: String,
    correct: bool, // 用户是否答对
) -> Result<(), String> {
    let mut wordbook = state.wordbook_store.lock().await;
    let mut item = wordbook.get(&word_id)
        .ok_or("Word not found")?
        .clone();
    
    // 更新熟练度（简化的 SM-2 算法）
    if correct {
        item.proficiency = (item.proficiency + 10).min(100);
    } else {
        item.proficiency = (item.proficiency.saturating_sub(5)).max(0);
    }
    
    item.review_count += 1;
    item.last_review_at = Some(chrono::Utc::now().timestamp());
    
    wordbook.update(item).map_err(|e| e.to_string())?;
    Ok(())
}
```

#### 2.3 前端：桌面端学习界面

```typescript
// src/pages/Learn.tsx - 新页面

import { useState, useEffect } from 'react';
import { invokeOrThrow } from '../services/invoke';

interface LearningPlan {
  todayWords: string[];
  reviewWords: string[];
  estimatedMinutes: number;
  reasoning: string;
}

export default function Learn() {
  const [plan, setPlan] = useState<LearningPlan | null>(null);
  const [currentWord, setCurrentWord] = useState<WordBookItem | null>(null);
  const [showAnswer, setShowAnswer] = useState(false);

  useEffect(() => {
    loadPlan();
  }, []);

  const loadPlan = async () => {
    const plan = await invokeOrThrow<LearningPlan>('generate_learning_plan');
    setPlan(plan);
    // 加载第一个单词
    if (plan.todayWords.length > 0) {
      // loadWordDetails(plan.todayWords[0]);
    }
  };

  const handleAnswer = async (correct: boolean) => {
    if (!currentWord) return;
    await invokeOrThrow('update_word_progress', {
      wordId: currentWord.id,
      correct
    });
    // 加载下一个单词
    setShowAnswer(false);
  };

  return (
    <div className="learn-page">
      <div className="plan-summary">
        <h2>📚 今日学习计划</h2>
        <p>新单词: {plan?.todayWords.length || 0} 个</p>
        <p>复习: {plan?.reviewWords.length || 0} 个</p>
        <p>预计时长: {plan?.estimatedMinutes || 0} 分钟</p>
      </div>

      {currentWord && (
        <div className="flashcard">
          <div className="question">
            <h3>{currentWord.word}</h3>
            <p className="context">{currentWord.sourceText}</p>
          </div>

          {showAnswer ? (
            <div className="answer">
              <p className="translation">{currentWord.translation}</p>
              <div className="actions">
                <button onClick={() => handleAnswer(false)}>❌ 不认识</button>
                <button onClick={() => handleAnswer(true)}>✅ 认识</button>
              </div>
            </div>
          ) : (
            <button onClick={() => setShowAnswer(true)}>
              显示答案
            </button>
          )}
        </div>
      )}
    </div>
  );
}
```

#### 2.4 前端：扩展 Popup 显示今日计划

```javascript
// extension/popup/popup.js

async function loadTodayPlan() {
  try {
    const resp = await fetch('http://127.0.0.1:60828/learning/plan', {
      method: 'GET'
    });
    const plan = await resp.json();
    
    document.getElementById('todayPlan').innerHTML = `
      <p>📖 今日新学: ${plan.todayWords.length} 个</p>
      <p>🔄 今日复习: ${plan.reviewWords.length} 个</p>
      <p>⏱️ 预计 ${plan.estimatedMinutes} 分钟</p>
      <a href="#" id="openDesktop">打开桌面端学习 →</a>
    `;
    
    document.getElementById('openDesktop').addEventListener('click', () => {
      // 通知 service worker 打开桌面端学习页面
      chrome.runtime.sendMessage({ type: 'openDesktopLearn' });
    });
    
  } catch (err) {
    console.error('Failed to load plan:', err);
  }
}
```

---

### Phase 3: 智能优化（1-2 周）

#### 3.1 间隔重复算法（Spaced Repetition）

```rust
// src-tauri/src/learning/spaced_repetition.rs

/// 实现 SM-2 算法（SuperMemo 2）
pub fn calculate_next_review(
    proficiency: i32,
    review_count: i32,
    correct: bool
) -> i64 {
    let easiness = 2.5 + (proficiency as f64 - 50.0) / 100.0;
    
    let interval = if review_count == 0 {
        1 // 第一次复习：1 天后
    } else if review_count == 1 {
        6 // 第二次复习：6 天后
    } else {
        let prev_interval = calculate_interval(review_count - 1, easiness);
        (prev_interval as f64 * easiness) as i64
    };
    
    let now = chrono::Utc::now();
    (now + chrono::Duration::days(interval)).timestamp()
}
```

#### 3.2 学习统计

```rust
#[derive(Serialize)]
pub struct LearningStats {
    pub total_words: usize,
    pub mastered: usize,         // 熟练度 >= 80
    pub learning: usize,         // 30 < 熟练度 < 80
    pub difficult: usize,        // 熟练度 <= 30
    pub streak_days: i32,        // 连续学习天数
    pub total_reviews: i32,
    pub avg_proficiency: f32,
}

#[tauri::command]
pub async fn get_learning_stats(
    state: State<'_, AppState>
) -> Result<LearningStats, String> {
    // 实现统计逻辑
}
```

#### 3.3 前端：学习统计仪表盘

```typescript
// src/pages/Dashboard.tsx

export default function Dashboard() {
  const [stats, setStats] = useState<LearningStats | null>(null);

  return (
    <div className="dashboard">
      <div className="stat-card">
        <h3>📊 学习统计</h3>
        <p>总词汇量: {stats?.totalWords}</p>
        <p>已掌握: {stats?.mastered}</p>
        <p>学习中: {stats?.learning}</p>
        <p>需加强: {stats?.difficult}</p>
        <p>🔥 连续学习: {stats?.streakDays} 天</p>
      </div>
      
      <div className="progress-chart">
        {/* 使用 Chart.js 或 Recharts 显示进度曲线 */}
      </div>
    </div>
  );
}
```

---

## 📋 实现优先级

### P0 - 核心闭环（必须完成）
1. ✅ 后端暴露生词本 HTTP API (`/wordbook`)
2. ✅ 扩展翻译自动记录历史
3. ✅ 扩展划词翻译添加"收藏"按钮
4. ✅ 桌面端生成学习计划命令

### P1 - 用户体验（重要）
5. ✅ 桌面端学习界面（闪卡模式）
6. ✅ 扩展 Popup 显示生词本预览
7. ✅ 学习进度更新（熟练度算法）
8. ✅ 今日计划提醒

### P2 - 智能优化（增强）
9. ⚠️ 间隔重复算法（SM-2）
10. ⚠️ 学习统计仪表盘
11. ⚠️ 多设备同步（云端 API）

### P3 - 高级功能（可选）
12. ❌ 例句生成（LLM）
13. ❌ 发音朗读（TTS）
14. ❌ 词汇测试（选择题/填空）
15. ❌ 学习报告（周报/月报）

---

## 🔍 技术细节

### 数据流

```
┌─────────────────┐
│ 浏览器扩展       │
│ (划词翻译)      │
└────┬────────────┘
     │ POST /browser/translate
     │ (自动记录历史)
     ▼
┌─────────────────┐
│ Desktop API     │
│ (127.0.0.1:60828)│
│ - /translate    │
│ - /wordbook ✨   │ ← 新增
│ - /history      │
│ - /learning ✨   │ ← 新增
└────┬────────────┘
     │
     ▼
┌─────────────────┐     ┌──────────────┐
│ AppState        │────▶│ LLM Engine   │
│ - wordbook_store│     │ (学习计划)    │
│ - history_store │     └──────────────┘
└─────────────────┘
     │
     ▼
┌─────────────────┐
│ SQLite / JSON   │
│ (本地存储)      │
└─────────────────┘
```

### API 规范

#### POST /wordbook
```json
Request:
{
  "word": "brilliant",
  "translation": "出色的",
  "sourceText": "It was a brilliant performance.",
  "sourceLang": "en",
  "targetLang": "zh",
  "context": "https://example.com/article"
}

Response:
{
  "id": "uuid-123",
  "word": "brilliant",
  "translation": "出色的",
  "addedAt": 1686712800,
  "proficiency": 0,
  "reviewCount": 0
}
```

#### GET /learning/plan
```json
Response:
{
  "todayWords": ["brilliant", "architecture"],
  "reviewWords": ["abandon", "legacy"],
  "estimatedMinutes": 15,
  "reasoning": "根据你的学习进度，建议今日重点复习低熟练度单词..."
}
```

---

## 📝 总结

### 核心价值
1. **无缝衔接**: 扩展翻译 → 一键收藏 → 桌面端学习，零摩擦
2. **智能驱动**: AI 根据学习数据自动生成个性化计划
3. **科学复习**: 间隔重复算法优化记忆曲线
4. **全平台覆盖**: 浏览器学习 + 桌面端深度学习

### 技术亮点
- **轻量级 API**: 复用现有 Desktop Bridge，无需额外服务
- **离线优先**: 数据本地存储，可选云端同步
- **LLM 赋能**: 不仅是翻译工具，更是 AI 学习助手

### 与竞品差异
- Immersive Translate: ❌ 无生词本，❌ 无学习计划
- LunaTranslator: ❌ 无浏览器扩展，❌ 无 AI 学习
- **Moon Translator**: ✅ 浏览器 + 桌面端 + AI 学习，全链路闭环

---

**下一步**: 优先完成 Phase 1（数据打通），1-2 周内实现扩展收藏 + 桌面端生词本 API。
