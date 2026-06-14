-- Database Schema for MoonTranslator
-- 基于 Event Sourcing 架构

-- ============================================
-- 1. Event Store 表
-- ============================================

-- 卡牌事件流（主表）
CREATE TABLE IF NOT EXISTS card_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_data TEXT NOT NULL,  -- JSON
    timestamp INTEGER NOT NULL,
    created_at INTEGER NOT NULL,

    CONSTRAINT card_events_timestamp_check CHECK (timestamp > 0),
    CONSTRAINT card_events_created_at_check CHECK (created_at > 0)
);

CREATE INDEX IF NOT EXISTS idx_card_events_card_id ON card_events(card_id);
CREATE INDEX IF NOT EXISTS idx_card_events_timestamp ON card_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_card_events_type ON card_events(event_type);

-- 卡牌快照表（性能优化）
CREATE TABLE IF NOT EXISTS cards (
    id TEXT PRIMARY KEY,
    word TEXT NOT NULL,
    current_version INTEGER NOT NULL DEFAULT 1,
    base_data TEXT,  -- JSON: BaseData
    ai_content TEXT,  -- JSON: AiContent
    fsrs_state TEXT NOT NULL,  -- JSON: CardState
    error_records TEXT,  -- JSON: Vec<ErrorRecord>
    annotations TEXT,  -- JSON: Vec<Annotation>
    learning_state TEXT,  -- JSON: LearningState (新增)
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,

    CONSTRAINT cards_version_check CHECK (current_version > 0)
);

CREATE INDEX IF NOT EXISTS idx_cards_word ON cards(word);
CREATE INDEX IF NOT EXISTS idx_cards_updated_at ON cards(updated_at);

-- Patch 历史表
CREATE TABLE IF NOT EXISTS card_patches (
    id TEXT PRIMARY KEY,
    card_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    target_field TEXT NOT NULL,
    operation TEXT NOT NULL,  -- "replace" | "append" | "update"
    old_value TEXT,  -- JSON
    new_value TEXT NOT NULL,  -- JSON
    reasoning TEXT,
    confidence REAL,
    generated_by TEXT,
    applied_at INTEGER,

    FOREIGN KEY (card_id) REFERENCES cards(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_patches_card_version ON card_patches(card_id, version);
CREATE INDEX IF NOT EXISTS idx_patches_applied_at ON card_patches(applied_at);

-- ============================================
-- 2. 词典数据表（从 ECDICT 导入）
-- ============================================

-- ECDICT 词典表（已存在于 ecdict.db）
-- 这里不重复创建，直接使用

-- 核心词库表（1.5万高频词）
CREATE TABLE IF NOT EXISTS core_vocabulary (
    word TEXT PRIMARY KEY,
    frequency_rank INTEGER NOT NULL,
    frq INTEGER,  -- ECDICT 的词频
    bnc INTEGER,  -- 英国国家语料库词频
    collins INTEGER,  -- 柯林斯星级
    oxford INTEGER,  -- 牛津3000
    tag TEXT,  -- 标签 (zk, gk, cet4, cet6, ielts, toefl, gre)

    CONSTRAINT core_vocab_rank_check CHECK (frequency_rank > 0 AND frequency_rank <= 15000)
);

CREATE INDEX IF NOT EXISTS idx_core_vocab_rank ON core_vocabulary(frequency_rank);
CREATE INDEX IF NOT EXISTS idx_core_vocab_tag ON core_vocabulary(tag);

-- ============================================
-- 3. 词根数据表（从 MorphoLex 导入）
-- ============================================

CREATE TABLE IF NOT EXISTS morphology (
    word TEXT PRIMARY KEY,
    segmentation TEXT NOT NULL,  -- 如 "brill.i.ant"
    pos TEXT,  -- 词性
    parts TEXT NOT NULL,  -- JSON: Vec<MorphologyPart>

    CONSTRAINT morphology_word_check CHECK (length(word) > 0)
);

CREATE INDEX IF NOT EXISTS idx_morphology_word ON morphology(word);

-- ============================================
-- 4. 词源数据表（从 Etymology-DB 导入）
-- ============================================

CREATE TABLE IF NOT EXISTS etymology_data (
    word TEXT PRIMARY KEY,
    origin_language TEXT,
    origin_word TEXT,
    meaning TEXT,
    historical_notes TEXT,

    CONSTRAINT etymology_word_check CHECK (length(word) > 0)
);

CREATE INDEX IF NOT EXISTS idx_etymology_word ON etymology_data(word);

-- ============================================
-- 5. 用户数据表
-- ============================================

-- 学习会话表
CREATE TABLE IF NOT EXISTS learning_sessions (
    id TEXT PRIMARY KEY,
    start_time INTEGER NOT NULL,
    end_time INTEGER,
    cards_reviewed INTEGER DEFAULT 0,
    cards_correct INTEGER DEFAULT 0,
    total_time_spent INTEGER DEFAULT 0,  -- 毫秒

    CONSTRAINT session_start_check CHECK (start_time > 0)
);

CREATE INDEX IF NOT EXISTS idx_sessions_start ON learning_sessions(start_time);

-- 复习记录表
CREATE TABLE IF NOT EXISTS review_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id TEXT NOT NULL,
    session_id TEXT,
    review_time INTEGER NOT NULL,
    rating TEXT NOT NULL,  -- "again" | "hard" | "good" | "easy"
    time_spent INTEGER,  -- 毫秒

    FOREIGN KEY (card_id) REFERENCES cards(id) ON DELETE CASCADE,
    FOREIGN KEY (session_id) REFERENCES learning_sessions(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_review_logs_card ON review_logs(card_id);
CREATE INDEX IF NOT EXISTS idx_review_logs_time ON review_logs(review_time);

-- ============================================
-- 6. 统计和分析表
-- ============================================

-- 每日统计表
CREATE TABLE IF NOT EXISTS daily_stats (
    date TEXT PRIMARY KEY,  -- YYYY-MM-DD
    cards_learned INTEGER DEFAULT 0,
    cards_reviewed INTEGER DEFAULT 0,
    total_time_spent INTEGER DEFAULT 0,  -- 毫秒
    average_accuracy REAL DEFAULT 0.0,

    CONSTRAINT daily_stats_date_check CHECK (length(date) = 10)
);

CREATE INDEX IF NOT EXISTS idx_daily_stats_date ON daily_stats(date);

-- ============================================
-- 7. 系统配置表
-- ============================================

CREATE TABLE IF NOT EXISTS system_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

-- 插入初始配置
INSERT OR IGNORE INTO system_config (key, value, updated_at) VALUES
    ('schema_version', '1', strftime('%s', 'now')),
    ('initialized_at', strftime('%s', 'now'), strftime('%s', 'now')),
    ('core_vocab_count', '0', strftime('%s', 'now')),
    ('morphology_count', '0', strftime('%s', 'now')),
    ('etymology_count', '0', strftime('%s', 'now'));
