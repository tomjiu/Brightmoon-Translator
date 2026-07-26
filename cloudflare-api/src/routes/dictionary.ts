// 词典查询路由

import { Hono } from 'hono';

type Bindings = {
  DB: D1Database;
  CACHE: KVNamespace;
  GITHUB_DATA_REPO: string;
  GITHUB_TOKEN: string;
};

export const dictionaryRoutes = new Hono<{ Bindings: Bindings }>();

// 搜索建议（联想词）
dictionaryRoutes.get('/suggest', async (c) => {
  const query = c.req.query('q');
  const limit = parseInt(c.req.query('limit') || '10');

  if (!query || query.length < 2) {
    return c.json({ suggestions: [] });
  }

  const { DB, CACHE } = c.env;

  // 先查缓存
  const cacheKey = `suggest:${query.toLowerCase()}`;
  const cached = await CACHE.get(cacheKey, 'json');
  if (cached) {
    return c.json({ suggestions: cached, source: 'cache' });
  }

  // 查数据库
  const results = await DB.prepare(
    `SELECT word, translation FROM stardict
     WHERE word LIKE ? OR word LIKE ?
     ORDER BY
       CASE WHEN word = ? THEN 0
            WHEN word LIKE ? THEN 1
            ELSE 2 END,
       frq ASC NULLS LAST
     LIMIT ?`
  )
  .bind(`${query}%`, `${query.toLowerCase()}%`, query, `${query}%`, limit)
  .all();

  const suggestions = results.results.map((row: any) => ({
    word: row.word,
    translation: row.translation?.split('\n')[0]?.substring(0, 50) || '',
  }));

  // 缓存 1 小时
  await CACHE.put(cacheKey, JSON.stringify(suggestions), { expirationTtl: 3600 });

  return c.json({ suggestions, source: 'db' });
});

// 单词详情查询
dictionaryRoutes.get('/lookup/:word', async (c) => {
  const word = c.req.param('word').toLowerCase();

  const { DB, CACHE } = c.env;

  // 先查缓存
  const cacheKey = `lookup:${word}`;
  const cached = await CACHE.get(cacheKey, 'json');
  if (cached) {
    return c.json({ ...cached, source: 'cache' });
  }

  // 查数据库
  const result = await DB.prepare(
    'SELECT * FROM stardict WHERE word = ? LIMIT 1'
  )
  .bind(word)
  .first();

  if (!result) {
    return c.json({ error: 'Word not found', word }, 404);
  }

  const entry = {
    word: result.word,
    phonetic: result.phonetic,
    definition: result.definition,
    translation: result.translation,
    pos: result.pos,
    frq: result.frq,
  };

  // 缓存 24 小时
  await CACHE.put(cacheKey, JSON.stringify(entry), { expirationTtl: 86400 });

  return c.json({ entry, source: 'db' });
});

// 批量查询
dictionaryRoutes.post('/batch', async (c) => {
  const { words } = await c.req.json<{ words: string[] }>();

  if (!words || !Array.isArray(words) || words.length === 0) {
    return c.json({ error: '请提供单词数组' }, 400);
  }

  if (words.length > 50) {
    return c.json({ error: '单次最多查询 50 个单词' }, 400);
  }

  const { DB } = c.env;

  const placeholders = words.map(() => '?').join(',');
  const results = await DB.prepare(
    `SELECT word, phonetic, translation, pos FROM stardict
     WHERE word IN (${placeholders})`
  )
  .bind(...words.map(w => w.toLowerCase()))
  .all();

  const entries = new Map(results.results.map((row: any) => [row.word, row]));

  const response = words.map(word => ({
    word: word.toLowerCase(),
    entry: entries.get(word.toLowerCase()) || null,
    found: entries.has(word.toLowerCase()),
  }));

  return c.json({ results: response });
});
