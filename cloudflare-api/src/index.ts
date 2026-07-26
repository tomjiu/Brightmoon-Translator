// MoonTranslator API - Cloudflare Workers 入口

import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { dictionaryRoutes } from './routes/dictionary';
import { learningRoutes } from './routes/learning';
import { syncRoutes } from './routes/sync';
import { aiRoutes } from './routes/ai';

type Bindings = {
  DB: D1Database;
  CACHE: KVNamespace;
  GITHUB_DATA_REPO: string;
  GITHUB_TOKEN: string;
};

const app = new Hono<{ Bindings: Bindings }>();

// CORS 中间件
app.use('/*', cors({
  origin: '*',
  allowMethods: ['GET', 'POST', 'PUT', 'DELETE', 'OPTIONS'],
  allowHeaders: ['Content-Type', 'Authorization'],
}));

// 健康检查
app.get('/', (c) => {
  return c.json({
    name: 'MoonTranslator API',
    version: '1.0.0',
    status: 'ok',
  });
});

// 路由挂载
app.route('/api/v1/dict', dictionaryRoutes);
app.route('/api/v1/learning', learningRoutes);
app.route('/api/v1/sync', syncRoutes);
app.route('/api/v1/ai', aiRoutes);

// 404 处理
app.notFound((c) => {
  return c.json({ error: 'Not Found', message: '请求的资源不存在' }, 404);
});

// 错误处理
app.onError((err, c) => {
  console.error('API Error:', err);
  return c.json({
    error: 'Internal Server Error',
    message: err.message,
  }, 500);
});

export default app;
