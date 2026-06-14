# Phase 1.1: Event Store 实现完成

完成时间: 2026-06-14
状态: **基础实现完成，等待编译测试**

---

## ✅ 已完成

### 1. Domain Layer（领域层）

#### `domain/event.rs` - 事件定义
```rust
pub enum CardEvent {
    WordImported,           // 单词导入
    AiAnalysisRequested,   // AI 分析请求
    AiContentGenerated,    // AI 内容生成
    OptimizationRequested, // 优化请求
    PatchProposed,         // Patch 提议
    PatchApplied,          // Patch 应用
    RolledBack,            // 回退
    UserRated,             // 用户打分
    QuizStarted,           // 测验开始
    QuizCompleted,         // 测验完成
    AnnotationRequested,   // 批注请求
    AnnotationGenerated,   // 批注生成
    FsrsUpdated,           // FSRS 更新
}
```

**核心数据结构**:
- `AiContent`: AI 生成内容（词源/助记法/例句/场景）
- `CardPatch`: 变更提议（AI 不直接改，而是提议）
- `Annotation`: AI 批注
- `CardState`: FSRS 状态

#### `domain/card.rs` - 卡牌实体
```rust
pub struct WordCard {
    pub id: String,
    pub word: String,
    pub current_version: u32,
    pub base_data: BaseData,
    pub ai_content: Option<AiContent>,
    pub fsrs_state: CardState,
    // ...
}

impl WordCard {
    // 核心：从事件流重建
    pub fn from_events(events: &[CardEvent]) -> Result<Self>
    
    // 应用单个事件
    pub fn apply_event(&mut self, event: &CardEvent) -> Result<()>
    
    // 回退到指定版本
    pub fn rollback_to_version(events: &[CardEvent], target_version: u32) -> Result<Self>
}
```

---

### 2. Infrastructure Layer（基础设施层）

#### `infrastructure/event_store.rs` - 事件存储
```rust
pub struct EventStore {
    pool: SqlitePool,  // 使用 sqlx 异步
}

impl EventStore {
    // 追加事件（Event Sourcing 核心）
    pub async fn append_event(&self, card_id: &str, event: &CardEvent) -> Result<i64>
    
    // 加载事件流
    pub async fn load_events(&self, card_id: &str) -> Result<Vec<CardEvent>>
    
    // 从事件流重建卡牌
    pub async fn rebuild_card(&self, card_id: &str) -> Result<WordCard>
    
    // 时间旅行
    pub async fn get_card_at_time(&self, card_id: &str, timestamp: i64) -> Result<WordCard>
    
    // 快照优化
    pub async fn update_snapshot(&self, card: &WordCard) -> Result<()>
    pub async fn load_snapshot(&self, card_id: &str) -> Result<Option<WordCard>>
}
```

---

### 3. 数据库 Schema

```sql
-- 事件流表（唯一真相）
CREATE TABLE card_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_data TEXT NOT NULL,  -- JSON
    timestamp INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

-- 卡牌快照表（性能优化）
CREATE TABLE cards (
    id TEXT PRIMARY KEY,
    word TEXT NOT NULL,
    current_version INTEGER NOT NULL,
    ai_content TEXT,  -- JSON
    fsrs_state TEXT,  -- JSON
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Patch 历史表
CREATE TABLE card_patches (
    id TEXT PRIMARY KEY,
    card_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    target_field TEXT NOT NULL,
    operation TEXT NOT NULL,
    old_value TEXT,
    new_value TEXT,
    reasoning TEXT,
    confidence REAL,
    generated_by TEXT,
    applied_at INTEGER
);
```

---

### 4. 依赖更新

```toml
[dependencies]
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "macros"] }
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
```

---

## 🎯 核心设计原则

### 1. Event Sourcing（事件溯源）
```
事件流是唯一真相
  ↓
卡牌是派生状态（可重建）
  ↓
支持时间旅行、版本回退
```

### 2. AI 生成 Patch（不直接修改）
```
AI 生成内容
  ↓
提议 Patch
  ↓
验证器检查
  ↓
应用 Patch
  ↓
记录到事件流
```

### 3. 性能优化
```
常用查询: 快照表（< 1ms）
完整历史: 事件流（< 10ms）
时间旅行: 按需重放事件
```

---

## 📊 功能对比

| 功能 | 传统方案 | Event Sourcing |
|------|---------|---------------|
| **数据一致性** | 覆盖写，历史丢失 | ✅ 完整历史 |
| **版本回退** | ❌ 不支持 | ✅ 任意版本 |
| **时间旅行** | ❌ 不支持 | ✅ 任意时间点 |
| **审计日志** | 需额外实现 | ✅ 自动完整 |
| **模型升级** | ❌ 无法重放 | ✅ 完全重放 |
| **A/B 测试** | 困难 | ✅ 对比不同模型 |

---

## 🚀 使用示例

### 基础流程
```rust
// 1. 创建 Event Store
let store = EventStore::new("sqlite:cards.db").await?;
store.init_schema().await?;

// 2. 导入单词
let card_id = uuid::Uuid::new_v4().to_string();
let event = CardEvent::WordImported {
    word: "brilliant".to_string(),
    source: "manual".to_string(),
    timestamp: now(),
};
store.append_event(&card_id, &event).await?;

// 3. AI 生成内容
let event = CardEvent::AiContentGenerated {
    content: AiContent { /* ... */ },
    model: "gpt-4".to_string(),
    confidence: 0.9,
    timestamp: now(),
};
store.append_event(&card_id, &event).await?;

// 4. 从事件流重建卡牌
let card = store.rebuild_card(&card_id).await?;
```

### 时间旅行
```rust
// 查看昨天的卡牌状态
let yesterday = now() - 86400;
let card_at_yesterday = store.get_card_at_time(&card_id, yesterday).await?;
```

### 版本回退
```rust
// 回退到版本 2
let events = store.load_events(&card_id).await?;
let card_v2 = WordCard::rollback_to_version(&events, 2)?;
```

---

## 📋 下一步（Phase 1.2）

### Patch 系统
- [ ] `PatchValidator` - Patch 验证器
- [ ] `PatchApplicator` - Patch 应用器
- [ ] Patch 冲突检测

### FSRS 集成
- [ ] 添加 `fsrs` crate 依赖
- [ ] `FsrsEngine` 实现
- [ ] 复习调度逻辑

### 数据库迁移
- [ ] 创建迁移脚本
- [ ] 初始化核心词库表
- [ ] 索引优化

---

## 🧪 测试

### 单元测试
```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_event_store_basic() {
        // 测试事件追加和加载
    }
    
    #[test]
    fn test_card_from_events() {
        // 测试从事件重建卡牌
    }
}
```

### 运行测试
```bash
cd src-tauri
cargo test --lib
```

### 运行示例
```bash
cargo run --example event_store_demo
```

---

## ✅ 完成标准

- [x] CardEvent 枚举定义（13种事件）
- [x] WordCard 从事件重放
- [x] EventStore 基础实现
- [x] 数据库 Schema 设计
- [x] sqlx 依赖集成
- [x] 模块结构组织
- [x] 使用示例
- [ ] 编译通过（等待中）
- [ ] 单元测试通过

---

**预计完成时间**: 今天（2026-06-14）

**当前状态**: 等待编译测试结果 🔄
