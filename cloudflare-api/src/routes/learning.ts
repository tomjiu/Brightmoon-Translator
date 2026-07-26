// 学习数据路由

import { Hono } from 'hono';

type Bindings = {
  DB: D1Database;
  CACHE: KVNamespace;
};

export const learningRoutes = new Hono<{ Bindings: Bindings }>();

// 获取用户学习计划
learningRoutes.get('/plans/:userId', async (c) => {
  const userId = c.req.param('userId');
  const { DB } = c.env;

  const plans = await DB.prepare(
    `SELECT lp.*,
            COUNT(pw.word) as total_words,
            SUM(CASE WHEN pw.learned = 1 THEN 1 ELSE 0 END) as learned_words
     FROM learning_plans lp
     LEFT JOIN plan_words pw ON lp.id = pw.plan_id
     WHERE lp.user_id = ? AND lp.status = 'active'
     GROUP BY lp.id
     ORDER BY lp.created_at DESC`
  )
  .bind(userId)
  .all();

  return c.json({ plans: plans.results });
});

// 创建学习计划
learningRoutes.post('/plans', async (c) => {
  const { userId, name, exam, dailyTarget, words } = await c.req.json<{
    userId: string;
    name: string;
    exam: string;
    dailyTarget: number;
    words: string[];
  }>();

  if (!userId || !name || !words || words.length === 0) {
    return c.json({ error: '缺少必要参数' }, 400);
  }

  const { DB } = c.env;
  const planId = crypto.randomUUID();
  const now = Math.floor(Date.now() / 1000);

  // 插入计划
  await DB.prepare(
    `INSERT INTO learning_plans (id, user_id, name, plan_type, target_exam, total_words, daily_target, start_date, status, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
  )
  .bind(planId, userId, name, 'preset', exam, words.length, dailyTarget, now, 'active', now, now)
  .run();

  // 批量插入单词
  const stmt = DB.prepare(
    'INSERT OR IGNORE INTO plan_words (plan_id, word, word_order, learned, added_at) VALUES (?, ?, ?, 0, ?)'
  );

  const batch = words.map((word, idx) =>
    stmt.bind(planId, word, idx, now)
  );

  // D1 批量插入（每批最多 100 条）
  for (let i = 0; i < batch.length; i += 100) {
    await DB.batch(batch.slice(i, i + 100));
  }

  return c.json({ planId, totalWords: words.length });
});

// 获取待复习卡牌
learningRoutes.get('/cards/due/:userId', async (c) => {
  const userId = c.req.param('userId');
  const limit = parseInt(c.req.query('limit') || '20');
  const now = Math.floor(Date.now() / 1000);

  const { DB } = c.env;

  const cards = await DB.prepare(
    `SELECT id, word, fsrs_state, ai_content
     FROM cards
     WHERE user_id = ? AND json_extract(fsrs_state, '$.next_review') <= ?
     ORDER BY json_extract(fsrs_state, '$.next_review') ASC
     LIMIT ?`
  )
  .bind(userId, now, limit)
  .all();

  return c.json({ cards: cards.results });
});

// 提交复习结果
learningRoutes.post('/review', async (c) => {
  const { userId, cardId, rating } = await c.req.json<{
    userId: string;
    cardId: string;
    rating: 'again' | 'hard' | 'good' | 'easy';
  }>();

  if (!userId || !cardId || !rating) {
    return c.json({ error: '缺少必要参数' }, 400);
  }

  const { DB } = c.env;
  const now = Math.floor(Date.now() / 1000);

  // 获取当前卡牌状态
  const card = await DB.prepare(
    'SELECT fsrs_state FROM cards WHERE id = ? AND user_id = ?'
  )
  .bind(cardId, userId)
  .first();

  if (!card) {
    return c.json({ error: '卡牌不存在' }, 404);
  }

  const fsrsState = JSON.parse(card.fsrs_state as string);

  // 简化版 FSRS 计算（完整版需移植 Rust 逻辑）
  const newState = calculateNextReview(fsrsState, rating);

  // 更新卡牌状态
  await DB.prepare(
    'UPDATE cards SET fsrs_state = ?, updated_at = ? WHERE id = ?'
  )
  .bind(JSON.stringify(newState), now, cardId)
  .run();

  // 记录事件
  await DB.prepare(
    `INSERT INTO card_events (card_id, event_type, event_data, timestamp)
     VALUES (?, 'fsrs_updated', ?, ?)`
  )
  .bind(cardId, JSON.stringify({ grade: rating, fsrs_state: newState }), now)
  .run();

  return c.json({ success: true, nextReview: newState.next_review });
});

// 简化版 FSRS 计算
function calculateNextReview(state: any, rating: string) {
  const ratingMap: Record<string, number> = {
    'again': 1, 'hard': 2, 'good': 3, 'easy': 4
  };
  const grade = ratingMap[rating] || 3;

  let stability = state.stability || 1.0;
  let difficulty = state.difficulty || 5.0;
  let reps = (state.reps || 0) + 1;
  let lapses = state.lapses || 0;

  if (rating === 'again') {
    lapses += 1;
    stability = Math.max(0.1, stability * 0.5);
    difficulty = Math.min(10, difficulty + 0.5);
  } else if (rating === 'hard') {
    stability = stability * 1.2;
    difficulty = Math.min(10, difficulty + 0.2);
  } else if (rating === 'good') {
    stability = stability * 2.5;
  } else if (rating === 'easy') {
    stability = stability * 4.0;
    difficulty = Math.max(1, difficulty - 0.3);
  }

  const intervalDays = Math.max(1, Math.round(stability * 9 * (1 / 0.9 - 1)));
  const nextReview = Math.floor(Date.now() / 1000) + intervalDays * 86400;

  return {
    stability,
    difficulty,
    reps,
    lapses,
    elapsed_days: state.elapsed_days || 0,
    scheduled_days: intervalDays,
    last_review: Math.floor(Date.now() / 1000),
    next_review: nextReview,
  };
}
