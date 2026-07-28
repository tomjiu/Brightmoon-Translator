import { useState, useCallback } from 'react';
import { useConfigStore } from '../../stores/configStore';
import type { CollectionConfig } from '../../types';
import {
  BookMarked,
  Save,
  Loader2,
  CheckCircle,
  AlertCircle,
  Eye,
  EyeOff,
  ExternalLink,
} from 'lucide-react';
import { collectionTestTarget, summarizeReport } from '../../hooks/useCollectionPush';

const DEFAULT_COLLECTION: CollectionConfig = {
  eudic: { enabled: false, token: '', bookName: 'Moon' },
  anki: { enabled: false, port: 8765, deck: 'Moon', model: 'Moon Card' },
  shanbay: { enabled: false, credential: '', wordbookId: '' },
  youdao: { enabled: false, cookie: '', lan: 'en' },
  maimemo: { enabled: false, token: '', notepadId: '', notepadTitle: 'Moon' },
  autoPushOnSave: true,
};

export default function CollectionSettings() {
  const { config, saved, saveConfig, updateConfig } = useConfigStore();
  const collection = config.collection ?? DEFAULT_COLLECTION;

  const [showEudicToken, setShowEudicToken] = useState(false);
  const [showShanbay, setShowShanbay] = useState(false);
  const [showYoudaoCookie, setShowYoudaoCookie] = useState(false);
  const [showMaimemoToken, setShowMaimemoToken] = useState(false);
  const [testing, setTesting] = useState<string | null>(null);
  const [testMsg, setTestMsg] = useState<{ target: string; ok: boolean; text: string } | null>(
    null,
  );

  const updateCollection = useCallback(
    (updater: (prev: CollectionConfig) => CollectionConfig) => {
      updateConfig((prev) => ({
        ...prev,
        collection: updater(prev.collection ?? DEFAULT_COLLECTION),
      }));
    },
    [updateConfig],
  );

  const handleTest = async (target: string) => {
    setTesting(target);
    setTestMsg(null);
    try {
      await saveConfig();
      const report = await collectionTestTarget(target);
      const first = report.results[0];
      setTestMsg({
        target,
        ok: first.ok ?? false,
        text: summarizeReport(report),
      });
    } catch (err) {
      setTestMsg({
        target,
        ok: false,
        text: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setTesting(null);
    }
  };

  return (
    <div className="space-y-5 animate-fadeIn">
      <div>
        <h1 className="ui-page-title">生词本外送</h1>
        <p className="ui-page-desc">
          将单词同步到欧陆词典、Anki、扇贝、有道单词本或墨墨。与应用内学习记录相互独立。
        </p>
      </div>

      <section className="bg-bg-secondary border border-border rounded-lg p-4 space-y-3">
        <label className="flex items-center gap-2 text-sm cursor-pointer">
          <input
            type="checkbox"
            className="rounded border-border"
            checked={collection.autoPushOnSave}
            onChange={(e) => updateCollection((c) => ({ ...c, autoPushOnSave: e.target.checked }))}
          />
          <span>保存到本地生词本时自动外送到已启用目标</span>
        </label>
      </section>

      {/* Eudic */}
      <section className="bg-bg-secondary border border-border rounded-lg p-4 space-y-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <BookMarked size={16} />
            <h2 className="font-medium text-sm">欧陆词典</h2>
          </div>
          <label className="flex items-center gap-2 text-xs cursor-pointer">
            <input
              type="checkbox"
              checked={collection.eudic.enabled}
              onChange={(e) =>
                updateCollection((c) => ({
                  ...c,
                  eudic: { ...c.eudic, enabled: e.target.checked },
                }))
              }
            />
            启用
          </label>
        </div>
        <p className="text-xs text-text-secondary">
          使用欧路开放 API（api.frdic.com）。Token 在欧陆开放平台申请。
        </p>
        <div className="space-y-2">
          <label className="block text-xs text-text-secondary">Token</label>
          <div className="flex gap-2">
            <input
              type={showEudicToken ? 'text' : 'password'}
              className="flex-1 bg-bg-primary border border-border rounded-md px-3 py-2 text-sm"
              value={collection.eudic.token}
              onChange={(e) =>
                updateCollection((c) => ({
                  ...c,
                  eudic: { ...c.eudic, token: e.target.value },
                }))
              }
              placeholder="Authorization token"
            />
            <button
              type="button"
              className="border border-border rounded-md px-2"
              onClick={() => setShowEudicToken((v) => !v)}
            >
              {showEudicToken ? <EyeOff size={14} /> : <Eye size={14} />}
            </button>
          </div>
          <label className="block text-xs text-text-secondary">词本名称</label>
          <input
            className="w-full bg-bg-primary border border-border rounded-md px-3 py-2 text-sm"
            value={collection.eudic.bookName}
            onChange={(e) =>
              updateCollection((c) => ({
                ...c,
                eudic: { ...c.eudic, bookName: e.target.value },
              }))
            }
            placeholder="Moon"
          />
          <button
            type="button"
            disabled={testing === 'eudic'}
            onClick={() => void handleTest('eudic')}
            className="text-xs border border-border rounded-md px-3 py-1.5 hover:bg-bg-tertiary disabled:opacity-50"
          >
            {testing === 'eudic' ? <Loader2 size={12} className="inline animate-spin" /> : null}{' '}
            测试连接
          </button>
        </div>
      </section>

      {/* Anki */}
      <section className="bg-bg-secondary border border-border rounded-lg p-4 space-y-3">
        <div className="flex items-center justify-between">
          <h2 className="font-medium text-sm">AnkiConnect</h2>
          <label className="flex items-center gap-2 text-xs cursor-pointer">
            <input
              type="checkbox"
              checked={collection.anki.enabled}
              onChange={(e) =>
                updateCollection((c) => ({
                  ...c,
                  anki: { ...c.anki, enabled: e.target.checked },
                }))
              }
            />
            启用
          </label>
        </div>
        <p className="text-xs text-text-secondary">
          需本机运行 Anki + AnkiConnect 插件（默认 127.0.0.1:8765）。
        </p>
        <div className="grid grid-cols-3 gap-2">
          <div>
            <label className="block text-xs text-text-secondary mb-1">端口</label>
            <input
              type="number"
              className="w-full bg-bg-primary border border-border rounded-md px-3 py-2 text-sm"
              value={collection.anki.port}
              onChange={(e) =>
                updateCollection((c) => ({
                  ...c,
                  anki: { ...c.anki, port: Number(e.target.value) || 8765 },
                }))
              }
            />
          </div>
          <div>
            <label className="block text-xs text-text-secondary mb-1">Deck</label>
            <input
              className="w-full bg-bg-primary border border-border rounded-md px-3 py-2 text-sm"
              value={collection.anki.deck}
              onChange={(e) =>
                updateCollection((c) => ({
                  ...c,
                  anki: { ...c.anki, deck: e.target.value },
                }))
              }
            />
          </div>
          <div>
            <label className="block text-xs text-text-secondary mb-1">Model</label>
            <input
              className="w-full bg-bg-primary border border-border rounded-md px-3 py-2 text-sm"
              value={collection.anki.model}
              onChange={(e) =>
                updateCollection((c) => ({
                  ...c,
                  anki: { ...c.anki, model: e.target.value },
                }))
              }
            />
          </div>
        </div>
        <button
          type="button"
          disabled={testing === 'anki'}
          onClick={() => void handleTest('anki')}
          className="text-xs border border-border rounded-md px-3 py-1.5 hover:bg-bg-tertiary disabled:opacity-50"
        >
          {testing === 'anki' ? <Loader2 size={12} className="inline animate-spin" /> : null}{' '}
          测试连接
        </button>
      </section>

      {/* Shanbay */}
      <section className="bg-bg-secondary border border-border rounded-lg p-4 space-y-3">
        <div className="flex items-center justify-between">
          <h2 className="font-medium text-sm">扇贝单词</h2>
          <label className="flex items-center gap-2 text-xs cursor-pointer">
            <input
              type="checkbox"
              checked={collection.shanbay.enabled}
              onChange={(e) =>
                updateCollection((c) => ({
                  ...c,
                  shanbay: { ...c.shanbay, enabled: e.target.checked },
                }))
              }
            />
            启用
          </label>
        </div>
        <p className="ui-caption">
          在浏览器登录扇贝后，从 Cookie 复制 <code className="text-xs">auth_token</code>
          。登录过期需重新复制。也可使用生词本 CSV 导出后在扇贝中导入。
        </p>
        <div className="space-y-2">
          <label className="block text-xs text-text-secondary">auth_token</label>
          <div className="flex gap-2">
            <input
              type={showShanbay ? 'text' : 'password'}
              className="flex-1 bg-bg-primary border border-border rounded-md px-3 py-2 text-sm"
              value={collection.shanbay.credential}
              onChange={(e) =>
                updateCollection((c) => ({
                  ...c,
                  shanbay: { ...c.shanbay, credential: e.target.value },
                }))
              }
              placeholder="从浏览器 Cookie 复制 auth_token"
            />
            <button
              type="button"
              className="border border-border rounded-md px-2"
              onClick={() => setShowShanbay((v) => !v)}
            >
              {showShanbay ? <EyeOff size={14} /> : <Eye size={14} />}
            </button>
          </div>
          <button
            type="button"
            disabled={testing === 'shanbay'}
            onClick={() => void handleTest('shanbay')}
            className="text-xs border border-border rounded-md px-3 py-1.5 hover:bg-bg-tertiary disabled:opacity-50"
          >
            {testing === 'shanbay' ? <Loader2 size={12} className="inline animate-spin" /> : null}{' '}
            测试连接
          </button>
        </div>
      </section>

      {/* Youdao wordbook */}
      <section className="bg-bg-secondary border border-border rounded-lg p-4 space-y-3">
        <div className="flex items-center justify-between">
          <h2 className="font-medium text-sm">有道单词本</h2>
          <label className="flex items-center gap-2 text-xs cursor-pointer">
            <input
              type="checkbox"
              checked={collection.youdao.enabled ?? false}
              onChange={(e) =>
                updateCollection((c) => ({
                  ...c,
                  youdao: {
                    ...(c.youdao ?? DEFAULT_COLLECTION.youdao),
                    enabled: e.target.checked,
                  },
                }))
              }
            />
            启用
          </label>
        </div>
        <p className="ui-caption">
          浏览器打开{' '}
          <a className="underline" href="https://www.youdao.com/" target="_blank" rel="noreferrer">
            youdao.com
          </a>{' '}
          并登录，在开发者工具的网络请求中找到 accountinfo，复制完整 Cookie 填入下方。
        </p>
        <div className="space-y-2">
          <label className="block text-xs text-text-secondary">Cookie</label>
          <div className="flex gap-2">
            <input
              type={showYoudaoCookie ? 'text' : 'password'}
              className="flex-1 bg-bg-primary border border-border rounded-md px-3 py-2 text-sm"
              value={collection.youdao.cookie ?? ''}
              onChange={(e) =>
                updateCollection((c) => ({
                  ...c,
                  youdao: {
                    ...(c.youdao ?? DEFAULT_COLLECTION.youdao),
                    cookie: e.target.value,
                  },
                }))
              }
              placeholder="完整 Cookie 字符串"
            />
            <button
              type="button"
              className="border border-border rounded-md px-2"
              onClick={() => setShowYoudaoCookie((v) => !v)}
            >
              {showYoudaoCookie ? <EyeOff size={14} /> : <Eye size={14} />}
            </button>
          </div>
          <div>
            <label className="block text-xs text-text-secondary mb-1">语言 lan</label>
            <input
              className="w-full bg-bg-primary border border-border rounded-md px-3 py-2 text-sm"
              value={collection.youdao.lan ?? 'en'}
              onChange={(e) =>
                updateCollection((c) => ({
                  ...c,
                  youdao: {
                    ...(c.youdao ?? DEFAULT_COLLECTION.youdao),
                    lan: e.target.value,
                  },
                }))
              }
              placeholder="en"
            />
          </div>
          <button
            type="button"
            disabled={testing === 'youdao'}
            onClick={() => void handleTest('youdao')}
            className="text-xs border border-border rounded-md px-3 py-1.5 hover:bg-bg-tertiary disabled:opacity-50"
          >
            {testing === 'youdao' ? <Loader2 size={12} className="inline animate-spin" /> : null}{' '}
            测试连接
          </button>
        </div>
      </section>

      {/* Maimemo */}
      <section className="bg-bg-secondary border border-border rounded-lg p-4 space-y-3">
        <div className="flex items-center justify-between">
          <h2 className="font-medium text-sm">墨墨背单词</h2>
          <label className="flex items-center gap-2 text-xs cursor-pointer">
            <input
              type="checkbox"
              checked={collection.maimemo.enabled ?? false}
              onChange={(e) =>
                updateCollection((c) => ({
                  ...c,
                  maimemo: {
                    ...(c.maimemo ?? DEFAULT_COLLECTION.maimemo),
                    enabled: e.target.checked,
                  },
                }))
              }
            />
            启用
          </label>
        </div>
        <p className="text-xs text-text-secondary">
          官方开放 API：App → 我的 → 更多设置 → 实验功能 → 开放 API，复制 Token。云词本 ID
          可空（首次外送会创建并在结果中返回 id，请填回保存）。
        </p>
        <div className="space-y-2">
          <label className="block text-xs text-text-secondary">API Token</label>
          <div className="flex gap-2">
            <input
              type={showMaimemoToken ? 'text' : 'password'}
              className="flex-1 bg-bg-primary border border-border rounded-md px-3 py-2 text-sm"
              value={collection.maimemo.token ?? ''}
              onChange={(e) =>
                updateCollection((c) => ({
                  ...c,
                  maimemo: {
                    ...(c.maimemo ?? DEFAULT_COLLECTION.maimemo),
                    token: e.target.value,
                  },
                }))
              }
              placeholder="开放 API Token"
            />
            <button
              type="button"
              className="border border-border rounded-md px-2"
              onClick={() => setShowMaimemoToken((v) => !v)}
            >
              {showMaimemoToken ? <EyeOff size={14} /> : <Eye size={14} />}
            </button>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <div>
              <label className="block text-xs text-text-secondary mb-1">云词本 ID</label>
              <input
                className="w-full bg-bg-primary border border-border rounded-md px-3 py-2 text-sm"
                value={collection.maimemo.notepadId ?? ''}
                onChange={(e) =>
                  updateCollection((c) => ({
                    ...c,
                    maimemo: {
                      ...(c.maimemo ?? DEFAULT_COLLECTION.maimemo),
                      notepadId: e.target.value,
                    },
                  }))
                }
                placeholder="可选"
              />
            </div>
            <div>
              <label className="block text-xs text-text-secondary mb-1">新建时标题</label>
              <input
                className="w-full bg-bg-primary border border-border rounded-md px-3 py-2 text-sm"
                value={collection.maimemo.notepadTitle ?? 'Moon'}
                onChange={(e) =>
                  updateCollection((c) => ({
                    ...c,
                    maimemo: {
                      ...(c.maimemo ?? DEFAULT_COLLECTION.maimemo),
                      notepadTitle: e.target.value,
                    },
                  }))
                }
              />
            </div>
          </div>
          <button
            type="button"
            disabled={testing === 'maimemo'}
            onClick={() => void handleTest('maimemo')}
            className="text-xs border border-border rounded-md px-3 py-1.5 hover:bg-bg-tertiary disabled:opacity-50"
          >
            {testing === 'maimemo' ? <Loader2 size={12} className="inline animate-spin" /> : null}{' '}
            测试连接
          </button>
        </div>
      </section>

      {testMsg && (
        <div
          className={`flex items-start gap-2 text-sm border rounded-lg p-3 ${
            testMsg.ok
              ? 'border-border bg-bg-secondary text-text-primary'
              : 'border-error/40 bg-error/5 text-error'
          }`}
        >
          {testMsg.ok ? <CheckCircle size={16} /> : <AlertCircle size={16} />}
          <pre className="whitespace-pre-wrap text-xs flex-1">{testMsg.text}</pre>
        </div>
      )}

      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={() => void saveConfig()}
          className="inline-flex items-center gap-2 bg-primary text-primary-fg rounded-md px-4 py-2 text-sm"
        >
          <Save size={14} />
          保存
        </button>
        {saved && <span className="text-xs text-text-secondary">已保存</span>}
        <a
          className="text-xs text-text-secondary inline-flex items-center gap-1 hover:underline"
          href="https://api.frdic.com"
          target="_blank"
          rel="noreferrer"
        >
          欧陆开放平台 <ExternalLink size={10} />
        </a>
      </div>
    </div>
  );
}
