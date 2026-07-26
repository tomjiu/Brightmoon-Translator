# Moon Translator - 数据库设计文档

**版本**：v1.0  
**更新时间**：2026-06-17

---

## 📊 架构概览

### 多端数据存储策略

```
┌──────────────────────────────────────────────────────────┐
│                    数据分层架构                            │
└──────────────────────────────────────────────────────────┘

Layer 1: 静态词典数据（只读，GitHub托管）
  ├─ ECDICT (180MB) → 分片26个 × ~7MB
  ├─ Oxford (41K词条)
  └─ GPT4-Dict (预生成AI内容)

Layer 2: 本地数据库（桌面端/移动端 SQLite）
  ├─ 用户配置
  ├─ 学习计划
  ├─ 卡牌状态（Event Sourcing）
  └─ 离线缓存

Layer 3: 云端数据库（Cloudflare D1）
  ├─ 用户账户
  ├─ 学习记录（跨设备同步）
  ├─ AI内容缓存（KV存储）
  └─ 统计数据
```

---

## 🗄️ 本地数据库 Schema（SQLite）

### 1. 用户配置表

```sql
CREATE TABLE user_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

-- 示例数据
INSERT INTO user_config (key, value, updated_at) VALUES
    ('user_id', 'anon-uuid-1', 1718611200),
    ('default_from_lang', 'en', 1718611200),
    ('default_to_lang', 'zh', 1718611200),
    ('daily_reminder', 'true', 1718611200),
    ('reminder_time', '09:00', 1718611200);
```

---

### 2. 学习计划表

```sql
CREATE TABLE learning_plans (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    plan_type TEXT NOT NULL,              -- preset | imported | custom
    target_exam TEXT,                     -- cet4 | cet6 | ky | ielts | toefl | gre
    total_words INTEGER NOT NULL,
    daily_target INTEGER NOT NULL DEFAULT 30,
    start_date INTEGER NOT NULL,
    end_date INTEGER,
    status TEXT NOT NULL DEFAULT 'active', -- active | completed | archived
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    synced_at INTEGER,                     -- 最后同步到云端时间
    dirty INTEGER NOT NULL DEFAULT 0       -- 是否有未同步更改
);

CREATE INDEX idx_plans_status ON learning_plans(status);
CREATE INDEX idx_plans_dirty ON learning_plans(dirty);
```

---

### 3. 计划单词表

```sql
CREATE TABLE plan_words (
    plan_id TEXT NOT NULL,
    word TEXT NOT NULL,
    word_order INTEGER NOT NULL,           -- 学习顺序
    learned INTEGER NOT NULL DEFAULT 0,    -- 是否已学（0/1）
    learned_at INTEGER,                    -- 学习时间戳
    added_at INTEGER NOT NULL,
    synced_at INTEGER,
    dirty INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (plan_id, word),
    FOREIGN KEY (plan_id) REFERENCES learning_plans(id) ON DELETE CASCADE
);

CREATE INDEX idx_plan_words_learned ON plan_words(plan_id, learned);
CREATE INDEX idx_plan_words_order ON plan_words(plan_id, word_order);
```

---

### 4. 卡牌事件流（Event Sourcing）

```sql
CREATE TABLE card_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id TEXT NOT NULL,
    event_type TEXT NOT NULL,              -- word_imported | fsrs_updated | ai_generated | user_edited
    event_data TEXT NOT NULL,              -- JSON格式
    timestamp INTEGER NOT NULL,
    synced_at INTEGER,
    dirty INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_events_card ON card_events(card_id, timestamp);
CREATE INDEX idx_events_dirty ON card_events(dirty);

-- 事件示例
-- event_type: word_imported
{
  "word": "abandon",
  "source": "plan-uuid-1"
}

-- event_type: fsrs_updated
{
  "rating": "Good",
  "old_state": {"stability": 5.2, "difficulty": 6.1, "next_review": 1718611200},
  "new_state": {"stability": 7.8, "difficulty": 5.9, "next_review": 1718870400}
}

-- event_type: ai_generated
{
  "content": {
    "mnemonics": [...],
    "etymology": {...},
    "examples": [...]
  },
  "model": "gpt-4",
  "confidence": 0.95
}
```

---

### 5. 卡牌快照表（性能优化）

```sql
CREATE TABLE cards (
    id TEXT PRIMARY KEY,
    word TEXT NOT NULL UNIQUE,
    fsrs_state TEXT NOT NULL,              -- JSON: {stability, difficulty, next_review, reps}
    learning_state TEXT,                   -- JSON: {phase, step}
    ai_content TEXT,                       -- JSON: 缓存的AI生成内容
    base_data TEXT,                        -- JSON: 词典基础数据（音标、释义）
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    synced_at INTEGER,
    dirty INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_cards_word ON cards(word);
CREATE INDEX idx_cards_next_review ON cards(json_extract(fsrs_state, '$.next_review'));
CREATE INDEX idx_cards_dirty ON cards(dirty);

-- fsrs_state 示例
{
  "stability": 7.8,
  "difficulty": 5.9,
  "next_review": 1718870400,   -- Unix timestamp
  "reps": 4,
  "lapses": 0
}

-- learning_state 示例
{
  "phase": "Review",            -- New | Learning | Review | Relearning
  "step": 0
}
```

---

### 6. 复习记录表（统计用）

```sql
CREATE TABLE review_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id TEXT NOT NULL,
    word TEXT NOT NULL,
    rating TEXT NOT NULL,                  -- Again | Hard | Good | Easy
    time_spent INTEGER,                    -- 毫秒
    reviewed_at INTEGER NOT NULL,
    synced_at INTEGER,
    dirty INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (card_id) REFERENCES cards(id)
);

CREATE INDEX idx_reviews_date ON review_logs(reviewed_at);
CREATE INDEX idx_reviews_card ON review_logs(card_id);
```

---

### 7. 学习统计缓存表

```sql
CREATE TABLE daily_stats (
    date TEXT PRIMARY KEY,                 -- YYYY-MM-DD
    new_cards INTEGER NOT NULL DEFAULT 0,
    reviewed_cards INTEGER NOT NULL DEFAULT 0,
    time_spent INTEGER NOT NULL DEFAULT 0, -- 秒
    correct_rate REAL,                     -- 正确率（0-1）
    synced_at INTEGER
);

CREATE INDEX idx_stats_date ON daily_stats(date DESC);
```

---

### 8. 词典缓存表（移动端）

```sql
CREATE TABLE dict_cache (
    word TEXT PRIMARY KEY,
    data TEXT NOT NULL,                    -- JSON格式（压缩后的词典数据）
    source TEXT NOT NULL,                  -- ecdict | youdao | online
    cached_at INTEGER NOT NULL,
    expires_at INTEGER                     -- 过期时间
);

CREATE INDEX idx_cache_expires ON dict_cache(expires_at);
```

---

### 9. AI内容缓存表

```sql
CREATE TABLE ai_cache (
    word TEXT PRIMARY KEY,
    content TEXT NOT NULL,                 -- JSON格式
    model TEXT NOT NULL,
    generated_at INTEGER NOT NULL,
    expires_at INTEGER,
    synced_to_cloud INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_ai_expires ON ai_cache(expires_at);
```

---

### 10. 同步队列表

```sql
CREATE TABLE sync_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    table_name TEXT NOT NULL,              -- 需要同步的表名
    record_id TEXT NOT NULL,               -- 记录ID
    operation TEXT NOT NULL,               -- insert | update | delete
    retry_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_sync_retry ON sync_queue(retry_count);
```

---

## ☁️ 云端数据库 Schema（Cloudflare D1）

### 1. 用户表

```sql
CREATE TABLE users (
    id TEXT PRIMARY KEY,
    platform TEXT NOT NULL,                -- wechat | email | device
    platform_id TEXT,                      -- 微信openid、邮箱或设备ID
    nickname TEXT,
    avatar_url TEXT,
    created_at INTEGER NOT NULL,
    last_login INTEGER NOT NULL,
    settings TEXT                          -- JSON: 用户偏好设置
);

CREATE UNIQUE INDEX idx_users_platform ON users(platform, platform_id);
CREATE INDEX idx_users_last_login ON users(last_login);
```

---

### 2. 学习计划表（云端）

```sql
CREATE TABLE cloud_learning_plans (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    plan_type TEXT NOT NULL,
    target_exam TEXT,
    words_json TEXT NOT NULL,              -- 完整单词列表（JSON数组）
    total_words INTEGER NOT NULL,
    daily_target INTEGER NOT NULL,
    start_date INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX idx_cloud_plans_user ON cloud_learning_plans(user_id, status);
```

---

### 3. 卡牌状态表（云端）

```sql
CREATE TABLE cloud_card_states (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    word TEXT NOT NULL,
    fsrs_state TEXT NOT NULL,
    learning_state TEXT,
    last_review INTEGER,
    next_review INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX idx_cloud_cards_user ON cloud_card_states(user_id, next_review);
CREATE INDEX idx_cloud_cards_word ON cloud_card_states(user_id, word);
```

---

### 4. 复习记录表（云端）

```sql
CREATE TABLE cloud_review_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    card_id TEXT NOT NULL,
    word TEXT NOT NULL,
    rating TEXT NOT NULL,
    time_spent INTEGER,
    reviewed_at INTEGER NOT NULL,
    device_id TEXT,                        -- 记录来自哪个设备
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX idx_cloud_reviews_user_date ON cloud_review_logs(user_id, reviewed_at);
```

---

### 5. AI内容共享表（云端缓存）

```sql
CREATE TABLE cloud_ai_content (
    word TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    model TEXT NOT NULL,
    usage_count INTEGER NOT NULL DEFAULT 1, -- 使用次数（热度指标）
    generated_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_ai_usage ON cloud_ai_content(usage_count DESC);
```

---

### 6. 同步日志表

```sql
CREATE TABLE sync_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    sync_type TEXT NOT NULL,               -- full | incremental
    records_synced INTEGER NOT NULL,
    conflicts_resolved INTEGER NOT NULL DEFAULT 0,
    started_at INTEGER NOT NULL,
    completed_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX idx_sync_logs_user ON sync_logs(user_id, started_at DESC);
```

---

## 🔄 数据同步机制

### 同步策略

```
┌─────────────────────────────────────────┐
│         离线优先 (Offline-First)         │
└─────────────────────────────────────────┘

1. 本地写入：所有操作先写本地SQLite
2. 标记脏数据：设置 dirty=1
3. 后台同步：定时上传 dirty=1 的记录
4. 冲突解决：Last-Write-Wins（基于 updated_at）
5. 清除脏标记：同步成功后 dirty=0, synced_at=now
```

### 增量同步流程

```sql
-- 1. 客户端：查询待同步数据
SELECT * FROM cards WHERE dirty = 1;

-- 2. 上传到云端
POST /api/v1/user/:userId/cards/sync
Body: [{cardId, word, fsrsState, updatedAt}, ...]

-- 3. 云端：检查冲突
SELECT updated_at FROM cloud_card_states 
WHERE id = ? AND user_id = ?;

-- 4. 冲突处理
IF server.updated_at > client.updated_at THEN
    -- 服务器版本更新，返回冲突
    RETURN {conflict: true, latestState: ...}
ELSE
    -- 客户端版本更新，写入服务器
    UPDATE cloud_card_states SET ...
END IF

-- 5. 客户端：清除脏标记
UPDATE cards SET dirty = 0, synced_at = ? WHERE id = ?;
```

---

## 📦 静态词典数据格式

### ECDICT 分片格式（JSON）

```json
{
  "shard": "a",
  "version": "2026.06",
  "words": [
    {
      "word": "abandon",
      "phonetic": "əˈbændən",
      "definition": "vt. to leave somebody...\nvt. to leave a thing...",
      "translation": "v. 放弃；抛弃；n. 放纵",
      "frq": 3521,
      "collins": 5,
      "oxford": 1,
      "tag": "cet4 cet6 ky"
    }
  ],
  "count": 1245
}
```

### AI内容预生成格式

```json
{
  "word": "abandon",
  "content": {
    "mnemonics": [
      {
        "type": "etymology",
        "content": "a-ban-don：一个（a）禁令（ban）被捐赠（don）= 放弃原有禁令"
      }
    ],
    "etymology": {
      "origin": "来自古法语 'à bandon'（自由支配）",
      "evolution": "14世纪进入英语，意为'放弃控制'"
    },
    "examples": [
      {
        "text": "The crew abandoned the sinking ship.",
        "translation": "船员们弃船逃生。",
        "context": "紧急情况"
      }
    ]
  },
  "model": "gpt-4",
  "version": "2026.06"
}
```

---

## 🔍 查询性能优化

### 1. 复习队列查询（高频）

```sql
-- 优化前（慢）
SELECT * FROM cards WHERE json_extract(fsrs_state, '$.next_review') <= ?;

-- 优化后：添加虚拟列 + 索引
ALTER TABLE cards ADD COLUMN next_review_ts INTEGER 
    GENERATED ALWAYS AS (json_extract(fsrs_state, '$.next_review')) VIRTUAL;

CREATE INDEX idx_cards_next_review_fast ON cards(next_review_ts);

-- 查询
SELECT * FROM cards WHERE next_review_ts <= ? ORDER BY next_review_ts LIMIT 100;
```

### 2. 今日统计查询

```sql
-- 使用daily_stats缓存表（每日凌晨更新）
SELECT * FROM daily_stats WHERE date = '2026-06-17';

-- 实时查询（慢，仅用于当日）
SELECT 
    COUNT(DISTINCT card_id) AS reviewed_count,
    SUM(time_spent) AS total_time,
    AVG(CASE WHEN rating IN ('Good', 'Easy') THEN 1.0 ELSE 0.0 END) AS correct_rate
FROM review_logs
WHERE reviewed_at >= strftime('%s', 'now', 'start of day');
```

---

## 💾 数据清理策略

### 自动清理规则

```sql
-- 1. 清理过期词典缓存（移动端）
DELETE FROM dict_cache WHERE expires_at < strftime('%s', 'now');

-- 2. 清理过期AI缓存
DELETE FROM ai_cache WHERE expires_at < strftime('%s', 'now');

-- 3. 归档旧复习记录（保留1年）
DELETE FROM review_logs 
WHERE reviewed_at < strftime('%s', 'now', '-1 year');

-- 4. 清理失败的同步队列（重试超过5次）
DELETE FROM sync_queue WHERE retry_count > 5;
```

### 数据库大小估算

| 数据类型           | 单条大小 | 1年数据量 | 总大小    |
|-------------------|---------|----------|----------|
| 卡牌状态           | ~500B   | 5000     | ~2.5MB   |
| 复习记录           | ~200B   | 50000    | ~10MB    |
| 卡牌事件流         | ~300B   | 100000   | ~30MB    |
| AI内容缓存         | ~1KB    | 5000     | ~5MB     |
| 词典缓存（移动端）  | ~800B   | 10000    | ~8MB     |
| **总计**          |         |          | **~56MB**|

---

## 🔐 数据安全

### 1. 敏感数据加密

```sql
-- 用户LLM API Key（AES-256-GCM加密）
CREATE TABLE encrypted_credentials (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    service TEXT NOT NULL,         -- openai | anthropic | custom
    encrypted_key BLOB NOT NULL,
    nonce BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
```

### 2. 数据备份

- **本地**：每日自动备份到 `~/.moontranslator/backups/`
- **云端**：Cloudflare D1 自动日备份（保留7天）
- **用户导出**：支持JSON格式完整导出

---

## 📊 数据迁移脚本

### 版本升级迁移（v1.0 → v1.1）

```sql
-- migration_v1_1.sql

BEGIN TRANSACTION;

-- 1. 添加新字段
ALTER TABLE cards ADD COLUMN image_url TEXT;
ALTER TABLE cards ADD COLUMN audio_cached INTEGER DEFAULT 0;

-- 2. 数据迁移
UPDATE cards SET audio_cached = 1 
WHERE ai_content LIKE '%audioUrl%';

-- 3. 更新版本号
INSERT OR REPLACE INTO user_config (key, value, updated_at)
VALUES ('db_version', '1.1', strftime('%s', 'now'));

COMMIT;
```

---

## 📝 开发工具

### SQLite查看工具
- **桌面**：DB Browser for SQLite
- **VSCode**：SQLite Viewer 插件
- **CLI**：`sqlite3 ~/.moontranslator/data.db`

### 测试数据生成

```sql
-- scripts/generate_test_data.sql

-- 生成1000个测试卡牌
INSERT INTO cards (id, word, fsrs_state, created_at, updated_at)
SELECT 
    'card-' || seq,
    'word' || seq,
    json_object('stability', 5.0, 'difficulty', 6.0, 'next_review', strftime('%s', 'now')),
    strftime('%s', 'now'),
    strftime('%s', 'now')
FROM (SELECT value AS seq FROM generate_series(1, 1000));
```

---

**维护者**：数据库架构师  
**Schema版本**：v1.0  
**下次Review**：重大功能更新时
