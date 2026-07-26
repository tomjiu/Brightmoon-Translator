// 数据同步路由

import { Hono } from 'hono';

type Bindings = {
  DB: D1Database;
  CACHE: KVNamespace;
};

export const syncRoutes = new Hono<{ Bindings: Bindings }>();

// 获取待同步数据（客户端拉取）
syncRoutes.get('/pull/:userId', async (c) => {
  const userId = c.req.param('userId');
  const since = parseInt(c.req.query('since') || '0');

  const { DB } = c.env;

  // 获取更新过的卡牌
  const cards = await DB.prepare(
    `SELECT id, word, fsrs_state, ai_content, created_at, updated_at
     FROM cards
     WHERE user_id = ? AND updated_at > ?
     ORDER BY updated_at ASC`
  )
  .bind(userId, since)
  .all();

  // 获取更新过的计划
  const plans = await DB.prepare(
    `SELECT id, name, description, plan_type, target_exam, total_words, daily_target, status, created_at, updated_at
     FROM learning_plans
     WHERE user_id = ? AND updated_at > ?
     ORDER BY updated_at ASC`
  )
  .bind(userId, since)
  .all();

  // 获取更新过的事件
  const events = await DB.prepare(
    `SELECT ce.id, ce.card_id, ce.event_type, ce.event_data, ce.timestamp
     FROM card_events ce
     JOIN cards c ON ce.card_id = c.id
     WHERE c.user_id = ? AND ce.timestamp > ?
     ORDER BY ce.timestamp ASC`
  )
  .bind(userId, since)
  .all();

  const serverTime = Math.floor(Date.now() / 1000);

  return c.json({
    cards: cards.results,
    plans: plans.results,
    events: events.results,
    serverTime,
  });
});

// 推送同步数据（客户端推送）
syncRoutes.post('/push/:userId', async (c) => {
  const userId = c.req.param('userId');
  const { cards, events, plans } = await c.req.json<{
    cards?: any[];
    events?: any[];
    plans?: any[];
  }>();

  const { DB } = c.env;
  const now = Math.floor(Date.now() / 1000);
  const syncedIds: string[] = [];
  const errors: string[] = [];

  // 同步卡牌（Last-Write-Wins 策略）
  if (cards && cards.length > 0) {
    for (const card of cards) {
      try {
        const existing = await DB.prepare(
          'SELECT updated_at FROM cards WHERE id = ?'
        )
        .bind(card.id)
        .first();

        if (!existing || (existing.updated_at as number) < card.updated_at) {
          await DB.prepare(
            `INSERT OR REPLACE INTO cards (id, user_id, word, fsrs_state, ai_content, created_at, updated_at, synced_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)`
          )
          .bind(
            card.id,
            userId,
            card.word,
            card.fsrs_state,
            card.ai_content || null,
            card.created_at,
            card.updated_at,
            now
          )
          .run();
          syncedIds.push(card.id);
        }
      } catch (e) {
        errors.push(`Card ${card.id}: ${e}`);
      }
    }
  }

  // 同步计划
  if (plans && plans.length > 0) {
    for (const plan of plans) {
      try {
        const existing = await DB.prepare(
          'SELECT updated_at FROM learning_plans WHERE id = ?'
        )
        .bind(plan.id)
        .first();

        if (!existing || (existing.updated_at as number) < plan.updated_at) {
          await DB.prepare(
            `INSERT OR REPLACE INTO learning_plans (id, user_id, name, description, plan_type, target_exam, total_words, daily_target, status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
          )
          .bind(
            plan.id,
            userId,
            plan.name,
            plan.description || '',
            plan.plan_type,
            plan.target_exam,
            plan.total_words,
            plan.daily_target,
            plan.status,
            plan.created_at,
            plan.updated_at
          )
          .run();
        }
      } catch (e) {
        errors.push(`Plan ${plan.id}: ${e}`);
      }
    }
  }

  // 同步事件
  if (events && events.length > 0) {
    for (const event of events) {
      try {
        // 检查事件是否已存在（基于 card_id + timestamp 去重）
        const existing = await DB.prepare(
          'SELECT id FROM card_events WHERE card_id = ? AND timestamp = ? AND event_type = ?'
        )
        .bind(event.card_id, event.timestamp, event.event_type)
        .first();

        if (!existing) {
          await DB.prepare(
            `INSERT INTO card_events (card_id, event_type, event_data, timestamp, synced_at)
             VALUES (?, ?, ?, ?, ?)`
          )
          .bind(event.card_id, event.event_type, event.event_data, event.timestamp, now)
          .run();
        }
      } catch (e) {
        errors.push(`Event: ${e}`);
      }
    }
  }

  return c.json({
    success: true,
    syncedCount: syncedIds.length,
    errors: errors.length > 0 ? errors : undefined,
    serverTime: now,
  });
});

// 获取同步状态
syncRoutes.get('/status/:userId', async (c) => {
  const userId = c.req.param('userId');
  const { DB } = c.env;

  const cardCount = await DB.prepare(
    'SELECT COUNT(*) as count FROM cards WHERE user_id = ?'
  )
  .bind(userId)
  .first();

  const planCount = await DB.prepare(
    'SELECT COUNT(*) as count FROM learning_plans WHERE user_id = ? AND status = ?'
  )
  .bind(userId, 'active')
  .first();

  const lastSync = await DB.prepare(
    'SELECT MAX(synced_at) as last_sync FROM cards WHERE user_id = ?'
  )
  .bind(userId)
  .first();

  return c.json({
    userId,
    cardCount: cardCount?.count || 0,
    planCount: planCount?.count || 0,
    lastSync: lastSync?.last_sync || null,
    serverTime: Math.floor(Date.now() / 1000),
  });
});
