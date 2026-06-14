-- Learning Plans Schema
-- 学习计划相关表

-- 学习计划表
CREATE TABLE IF NOT EXISTS learning_plans (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,                    -- 计划名称 (如 "CET-4")
    description TEXT,                       -- 计划描述
    plan_type TEXT NOT NULL,               -- 计划类型: preset, custom, imported
    target_exam TEXT,                       -- 目标考试: cet4, cet6, kaoyan, ielts, toefl, gre, custom
    total_words INTEGER NOT NULL,          -- 总词汇数
    daily_target INTEGER NOT NULL,         -- 每日目标
    start_date INTEGER,                    -- 开始日期 (timestamp)
    end_date INTEGER,                      -- 结束日期 (timestamp)
    status TEXT NOT NULL,                  -- 状态: active, paused, completed, archived
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- 计划词汇关联表
CREATE TABLE IF NOT EXISTS plan_words (
    plan_id TEXT NOT NULL,
    word TEXT NOT NULL,
    word_order INTEGER NOT NULL,          -- 词汇顺序
    added_at INTEGER NOT NULL,
    PRIMARY KEY (plan_id, word),
    FOREIGN KEY (plan_id) REFERENCES learning_plans(id) ON DELETE CASCADE
);

-- 计划进度表
CREATE TABLE IF NOT EXISTS plan_progress (
    plan_id TEXT NOT NULL,
    date INTEGER NOT NULL,                 -- 日期 (timestamp, 00:00:00)
    words_learned INTEGER NOT NULL,        -- 当日学习词数
    words_reviewed INTEGER NOT NULL,       -- 当日复习词数
    created_at INTEGER NOT NULL,
    PRIMARY KEY (plan_id, date),
    FOREIGN KEY (plan_id) REFERENCES learning_plans(id) ON DELETE CASCADE
);

-- 预设词汇表元数据
CREATE TABLE IF NOT EXISTS preset_wordlists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,                    -- 名称
    name_zh TEXT NOT NULL,                 -- 中文名称
    exam_type TEXT NOT NULL,               -- 考试类型
    word_count INTEGER NOT NULL,           -- 词汇数量
    difficulty_level INTEGER NOT NULL,     -- 难度等级 1-5
    description TEXT,
    source_url TEXT,                       -- 来源URL
    version TEXT,
    created_at INTEGER NOT NULL
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_plan_words_plan_id ON plan_words(plan_id);
CREATE INDEX IF NOT EXISTS idx_plan_words_order ON plan_words(plan_id, word_order);
CREATE INDEX IF NOT EXISTS idx_plan_progress_plan_date ON plan_progress(plan_id, date);
CREATE INDEX IF NOT EXISTS idx_learning_plans_status ON learning_plans(status);
CREATE INDEX IF NOT EXISTS idx_preset_wordlists_exam ON preset_wordlists(exam_type);
