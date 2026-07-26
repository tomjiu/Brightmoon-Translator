-- D1 数据库 Schema - MoonTranslator 云端学习数据

-- 用户表
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);

-- 学习计划
CREATE TABLE IF NOT EXISTS learning_plans (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    plan_type TEXT NOT NULL DEFAULT 'preset',
    target_exam TEXT,
    total_words INTEGER NOT NULL DEFAULT 0,
    daily_target INTEGER NOT NULL DEFAULT 30,
    start_date INTEGER,
    status TEXT NOT NULL DEFAULT 'active',
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- 计划单词
CREATE TABLE IF NOT EXISTS plan_words (
    plan_id TEXT NOT NULL,
    word TEXT NOT NULL,
    word_order INTEGER NOT NULL DEFAULT 0,
    learned INTEGER NOT NULL DEFAULT 0,
    added_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (plan_id, word),
    FOREIGN KEY (plan_id) REFERENCES learning_plans(id) ON DELETE CASCADE
);

-- 卡牌状态（核心同步数据）
CREATE TABLE IF NOT EXISTS cards (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    word TEXT NOT NULL,
    fsrs_state TEXT NOT NULL DEFAULT '{}',
    ai_content TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    synced_at INTEGER,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- 卡牌事件（同步用）
CREATE TABLE IF NOT EXISTS card_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_data TEXT NOT NULL,
    timestamp INTEGER NOT NULL DEFAULT (unixepoch()),
    synced_at INTEGER,
    FOREIGN KEY (card_id) REFERENCES cards(id) ON DELETE CASCADE
);

-- 学习会话
CREATE TABLE IF NOT EXISTS study_sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    plan_id TEXT,
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
    cards_reviewed INTEGER NOT NULL DEFAULT 0,
    new_cards INTEGER NOT NULL DEFAULT 0,
    correct_count INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_cards_user ON cards(user_id);
CREATE INDEX IF NOT EXISTS idx_cards_word ON cards(word);
CREATE INDEX IF NOT EXISTS idx_cards_next_review ON cards(user_id, json_extract(fsrs_state, '$.next_review'));
CREATE INDEX IF NOT EXISTS idx_card_events_card ON card_events(card_id);
CREATE INDEX IF NOT EXISTS idx_card_events_type ON card_events(event_type);
CREATE INDEX IF NOT EXISTS idx_plan_words_plan ON plan_words(plan_id);
CREATE INDEX IF NOT EXISTS idx_learning_plans_user ON learning_plans(user_id);
