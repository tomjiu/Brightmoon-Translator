// AI 代理路由 - 代理 LLM 请求，缓存高频词内容

import { Hono } from 'hono';

type Bindings = {
  DB: D1Database;
  CACHE: KVNamespace;
  GITHUB_DATA_REPO: string;
  GITHUB_TOKEN: string;
};

export const aiRoutes = new Hono<{ Bindings: Bindings }>();

// 获取单词 AI 内容（优先缓存）
aiRoutes.get('/content/:word', async (c) => {
  const word = c.req.param('word').toLowerCase();
  const { CACHE } = c.env;

  // 先查 KV 缓存
  const cacheKey = `ai:${word}`;
  const cached = await CACHE.get(cacheKey, 'json');
  if (cached) {
    return c.json({ content: cached, source: 'cache' });
  }

  // 缓存未命中，返回 404 让客户端生成
  return c.json({ error: 'AI content not cached', word }, 404);
});

// 保存 AI 内容到缓存
aiRoutes.post('/content', async (c) => {
  const { word, content } = await c.req.json<{
    word: string;
    content: any;
  }>();

  if (!word || !content) {
    return c.json({ error: '缺少必要参数' }, 400);
  }

  const { CACHE } = c.env;

  // 保存到 KV，1 年 TTL
  const cacheKey = `ai:${word.toLowerCase()}`;
  await CACHE.put(cacheKey, JSON.stringify(content), {
    expirationTtl: 365 * 24 * 3600,
  });

  return c.json({ success: true, word });
});

// 批量获取 AI 内容
aiRoutes.post('/content/batch', async (c) => {
  const { words } = await c.req.json<{ words: string[] }>();

  if (!words || !Array.isArray(words) || words.length === 0) {
    return c.json({ error: '请提供单词数组' }, 400);
  }

  if (words.length > 20) {
    return c.json({ error: '单次最多查询 20 个单词' }, 400);
  }

  const { CACHE } = c.env;

  const results = await Promise.all(
    words.map(async (word) => {
      const cacheKey = `ai:${word.toLowerCase()}`;
      const content = await CACHE.get(cacheKey, 'json');
      return {
        word: word.toLowerCase(),
        content,
        cached: !!content,
      };
    })
  );

  return c.json({ results });
});

// 统计缓存命中率
aiRoutes.get('/stats', async (c) => {
  const { CACHE } = c.env;

  // KV 不支持直接统计，这里返回配置信息
  return c.json({
    message: 'AI 内容缓存统计',
    note: 'KV 命名空间不支持直接统计，请在 Cloudflare Dashboard 查看',
  });
});
