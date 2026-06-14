# AI 学习系统架构设计

生成时间: 2026-06-14
状态: **已确定，进入开发阶段**

---

## 🎯 核心设计原则

> **"事件流 + 可回放状态 + AI 生成 Patch"是唯一真相，卡牌只是派生视图**

1. **Event Sourcing** - 所有变更都是不可变事件，永不覆盖历史
2. **卡牌是派生状态** - 从事件流重放得出（可选快照加速查询）
3. **AI 生成 Patch** - AI 只提议变更，由核心引擎验证后应用
4. **State Machine** - Learning Workflow 是状态机，AI 只是其中一个 Worker
5. **Skill > MCP** - Skill 是稳定 API，MCP 是其中一个 Provider

---

## 🏗️ 系统分层

```
┌─────────────────────────────────────┐
│         Tauri UI (React)             │
└───────────────┬─────────────────────┘
                │ Tauri IPC Commands
                ▼
┌─────────────────────────────────────┐
│     Application Service Layer        │
│  (协调各 Domain，处理跨域逻辑)        │
└───────────────┬─────────────────────┘
                │
                ▼
┌─────────────────────────────────────┐
│      Learning Workflow               │
│      (State Machine + Event Bus)     │
│                                      │
│  Import → Analyze → Generate →       │
│  Review → Rate → Optimize →          │
│  Quiz → Annotate → FSRS → Schedule   │
└───────┬──────────────────┬───────────┘
        │                  │
        ▼                  ▼
┌──────────────┐   ┌──────────────────┐
│ Domain Core  │   │  AI Coordinator   │
│              │   │                  │
│ FSRS Engine  │   │ LLM Provider     │
│ Event Store  │   │ Skill Registry   │
│ Card Engine  │   │ Patch Generator  │
│ Version Mgr  │   │ MCP Client       │
│ User Profile │   └──────────────────┘
└──────────────┘
        │
        ▼
┌─────────────────────────────────────┐
│         SQLite (sqlx)                │
│  card_events | cards | patches |     │
│  user_profile | review_logs |        │
│  annotations | embeddings            │
└─────────────────────────────────────┘
```

---

## 📦 技术栈（已确定）

| 层级 | 选型 | 用途 |
|------|------|------|
| **LLM 客户端** | `rust-genai` | 多提供商统一接口（OpenAI/DeepSeek/Claude/Ollama） |
| **Structured Output** | `schemars` + `rust-genai JsonSpec` | 自动生成 JSON Schema，强制 LLM 输出格式 |
| **FSRS 算法** | `fsrs`（官方 Rust 实现） | 间隔重复调度，Anki 同款算法 |
| **MCP 协议** | `modelcontextprotocol/rust-sdk` | 外部工具集成 |
| **数据库** | `sqlx` + SQLite | 事件存储 + FTS5 全文搜索 + WAL |
| **State Machine** | 自实现（enum + match） | 轻量，无需框架 |
| **Event Bus** | `tokio::sync::broadcast` | Tokio 内置 |
| **词典数据** | ECDICT SQLite | 见词典层 |

---

## 🗃️ 数据库 Schema

```sql
-- === 事件流（唯一真相） ===
CREATE TABLE card_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id     TEXT    NOT NULL,
    event_type  TEXT    NOT NULL,
    event_data  TEXT    NOT NULL,   -- JSON
    timestamp   INTEGER NOT NULL,
    INDEX idx_card_events_card_id (card_id),
    INDEX idx_card_events_timestamp (timestamp)
);

-- === 卡牌快照（派生视图，可从事件重建） ===
CREATE TABLE cards (
    id               TEXT    PRIMARY KEY,
    word             TEXT    NOT NULL,
    current_version  INTEGER NOT NULL DEFAULT 1,
    ai_content       TEXT,           -- JSON: { etymology, mnemonics, examples, scenes }
    fsrs_state       TEXT,           -- JSON: { stability, difficulty, elapsed_days }
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL,
    INDEX idx_cards_word (word)
);

-- === Patch 历史（版本控制） ===
CREATE TABLE card_patches (
    id             TEXT    PRIMARY KEY,
    card_id        TEXT    NOT NULL,
    version        INTEGER NOT NULL,
    target_field   TEXT    NOT NULL,
    operation      TEXT    NOT NULL,
    old_value      TEXT,
    new_value      TEXT,
    reasoning      TEXT,
    confidence     REAL,
    generated_by   TEXT,             -- "gpt-4" | "deepseek-chat"
    applied_at     INTEGER,
    INDEX idx_patches_card_version (card_id, version)
);

-- === 用户档案 ===
CREATE TABLE user_profile (
    id             TEXT    PRIMARY KEY,
    occupation     TEXT,
    interests      TEXT,             -- JSON array
    native_lang    TEXT    DEFAULT 'zh',
    memory_type    TEXT,             -- visual | auditory | kinesthetic
    preferred_mnemonics TEXT,        -- JSON array: [etymology, scene, homophone]
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL
);

-- === 弱点记录 ===
CREATE TABLE weak_points (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     TEXT    NOT NULL,
    pattern     TEXT    NOT NULL,    -- "混淆 -able/-ible"
    frequency   INTEGER DEFAULT 1,
    last_seen   INTEGER NOT NULL
);

-- === 复习日志 ===
CREATE TABLE review_logs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id     TEXT    NOT NULL,
    grade       TEXT    NOT NULL,    -- again | hard | good | easy
    time_spent  INTEGER,             -- 毫秒
    timestamp   INTEGER NOT NULL,
    INDEX idx_review_logs_card_id (card_id)
);

-- === 错题记录 ===
CREATE TABLE quiz_errors (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id        TEXT    NOT NULL,
    user_answer    TEXT,
    correct_answer TEXT,
    error_type     TEXT,             -- spelling | meaning | usage
    timestamp      INTEGER NOT NULL
);

-- === AI 批注 ===
CREATE TABLE annotations (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id     TEXT    NOT NULL,
    trigger     TEXT,                -- after_error | low_rating | periodic
    content     TEXT    NOT NULL,
    highlights  TEXT,                -- JSON array
    timestamp   INTEGER NOT NULL
);

-- === 词书 ===
CREATE TABLE wordbooks (
    id          TEXT    PRIMARY KEY,
    name        TEXT    NOT NULL,
    source_type TEXT,                -- toefl | ielts | gre | custom
    words       TEXT    NOT NULL,    -- JSON array
    total       INTEGER NOT NULL,
    learned     INTEGER DEFAULT 0,
    mastered    INTEGER DEFAULT 0,
    created_at  INTEGER NOT NULL
);

-- === 嵌入向量（可选，后期加） ===
-- CREATE TABLE embeddings (
--     card_id    TEXT PRIMARY KEY,
--     vector     BLOB NOT NULL       -- float32 array
-- );
```

---

## 🔄 事件定义

```rust
pub enum CardEvent {
    // 生命周期
    WordImported { word: String, source: String },
    
    // AI 生成
    AiAnalysisRequested,
    AiContentGenerated { content: AiContent, model: String },
    
    // Patch 流程
    OptimizationRequested { field: String, reason: String },
    PatchProposed { patch: CardPatch },
    PatchApplied { version: u32, patch: CardPatch },
    RolledBack { to_version: u32 },
    
    // 用户行为
    UserRated { field: String, score: f32, feedback: Option<String> },
    
    // 考核
    QuizStarted,
    QuizCompleted { correct: bool, user_answer: String, time_spent: u32 },
    
    // AI 批注
    AnnotationRequested { trigger: String },
    AnnotationGenerated { content: Annotation },
    
    // FSRS
    FsrsUpdated { grade: Rating, new_state: CardState },
}
```

---

## 🔧 Skill 列表

```rust
// 内置 Skills
DictionarySkill         // 查本地 ECDICT + 云端 API
EtymologySkill          // 词源/词根查询
GenerateCardSkill       // LLM 生成初始卡牌
OptimizeMnemonicSkill   // 根据用户反馈优化助记法
OptimizeExamplesSkill   // 根据用户反馈优化例句
GenerateAnnotationSkill // 考试后生成批注
AnalyzeWeakPointsSkill  // 分析用户弱点
GenerateLearningPlan    // AI 生成今日学习计划
FsrsScheduleSkill       // FSRS 计算下次复习时间
WordbookImportSkill     // 词书格式转换导入

// MCP 扩展（用户自定义）
McpSkill { endpoint, schema }
```

---

## 📊 完整学习流程

```
1. 导入词书
   用户选择词书（TOEFL/自定义） → 解析 → 写入 WordImported 事件

2. 生成卡牌
   触发 AiAnalysisRequested
   → DictionarySkill（查 ECDICT 获取基础信息）
   → EtymologySkill（查词根词源）
   → GenerateCardSkill（LLM 生成个性化内容）
   → PatchProposed + PatchApplied
   → 卡牌快照更新

3. 学习阶段
   展示卡牌 → 用户选择助记法 → UserRated
   低分（< 3）→ OptimizationRequested → AI 重新生成 → PatchApplied

4. 考核阶段
   Quiz 展示 → 用户答题 → QuizCompleted
   答错 → AnnotationRequested → LLM 分析 → AnnotationGenerated
   → FsrsUpdated（降低稳定性，加速复习）

5. 持续优化
   定期 AnalyzeWeakPoints → 更新 UserProfile
   每日 GenerateLearningPlan → 基于 FSRS + 弱点 → 今日计划
```

---

## 📚 词典数据层

详见：`docs/DICTIONARY_RESOURCES.md`

推荐组合：
- **主词库**: ECDICT（SQLite，324万词，含考试标签/词频）
- **词根分析**: Ceelog/DictionaryByGPT4（8000核心词，详细词根，CC-BY-SA）
- **深度词源**: droher/etymology-db（420万词源关系，CC-SA）
- **考试词表**: ECDICT tag 字段直接过滤（toefl/ielts/gre/cet4/cet6）

---

## 🚀 开发阶段计划

### Phase 1: 核心基础设施（2周）
- [ ] Event Store（SQLite + sqlx）
- [ ] CardEvent 枚举定义
- [ ] WordCard 从事件重放
- [ ] Patch 系统（CardPatch + PatchValidator）
- [ ] FSRS 集成（fsrs crate）
- [ ] 数据库 schema 迁移

### Phase 2: Skill + LLM（1周）
- [ ] Skill trait + SkillRegistry
- [ ] rust-genai Provider 封装（LlmProvider trait）
- [ ] DictionarySkill（接入 ECDICT）
- [ ] GenerateCardSkill（LLM + JSON Schema）
- [ ] ECDICT SQLite 导入项目

### Phase 3: Learning Workflow（1周）
- [ ] State Machine 实现
- [ ] Event Bus（tokio broadcast）
- [ ] 完整学习流程：Import → Generate → Review → Quiz → FSRS
- [ ] Tauri Commands 接口

### Phase 4: 前端 Dictionary 页面（1周）
- [ ] 连接真实 wordbook（替换 mock 数据）
- [ ] 单词卡片 UI（词根/例句/助记法展示）
- [ ] 用户打分交互
- [ ] AI 生成卡片按钮

### Phase 5: 完整学习系统（2周）
- [ ] 闪卡学习模式
- [ ] 考核模式（选择题/拼写）
- [ ] AI 批注展示
- [ ] 今日学习计划
- [ ] 学习统计仪表盘

### Phase 6: 扩展（后续）
- [ ] 浏览器扩展生词同步
- [ ] MCP 协议支持
- [ ] 多语言支持（日语/韩语词典）
- [ ] 词典格式转换（MDX/StarDict）
