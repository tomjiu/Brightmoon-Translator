// AIGenerationProgress - AI内容批量生成进度通知

import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Sparkles, CheckCircle, XCircle, Loader2 } from 'lucide-react';
import { isTauriRuntime } from '../../services/tauriRuntime';

interface GenerationProgress {
  taskId: string;
  total: number;
  completed: number;
  failed: number;
  currentWord?: string;
  status: 'starting' | 'running' | 'completed' | 'failed';
}

export function AIGenerationProgress() {
  const [progress, setProgress] = useState<GenerationProgress | null>(null);
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    if (!isTauriRuntime()) return;

    let cancelled = false;
    const unlistenPromise = listen<GenerationProgress>('ai-generation-progress', (event) => {
      const data = event.payload;
      setProgress(data);
      setVisible(true);

      if (data.status === 'completed' || data.status === 'failed') {
        setTimeout(() => {
          setVisible(false);
        }, 3000);
      }
    });

    return () => {
      cancelled = true;
      void Promise.resolve(unlistenPromise).then((fn) => {
        if (!cancelled && typeof fn === 'function') fn();
      });
    };
  }, []);

  if (!visible || !progress) return null;

  const percentage =
    progress.total > 0 ? Math.round((progress.completed / progress.total) * 100) : 0;

  const isCompleted = progress.status === 'completed';
  const isFailed = progress.status === 'failed';
  const isRunning = progress.status === 'running';

  return (
    <div className="fixed bottom-4 right-4 z-50 animate-slideInRight">
      <div className="bg-white dark:bg-gray-800 rounded-xl shadow-2xl border border-border p-4 w-80">
        {/* 标题 */}
        <div className="flex items-center gap-2 mb-3">
          {isRunning && <Loader2 className="animate-spin text-primary" size={18} />}
          {isCompleted && <CheckCircle className="text-green-500" size={18} />}
          {isFailed && <XCircle className="text-red-500" size={18} />}

          <div className="flex-1">
            <h4 className="text-sm font-semibold text-text-primary">
              {isCompleted && '✨ AI内容生成完成'}
              {isFailed && '❌ 生成失败'}
              {isRunning && '🚀 AI内容预生成中...'}
              {progress.status === 'starting' && '⏳ 正在准备...'}
            </h4>
          </div>

          {/* 关闭按钮 */}
          <button
            onClick={() => setVisible(false)}
            className="text-text-tertiary hover:text-text-primary transition-colors"
          >
            <XCircle size={16} />
          </button>
        </div>

        {/* 进度条 */}
        <div className="mb-2">
          <div className="flex justify-between text-xs text-text-secondary mb-1">
            <span>
              {progress.completed} / {progress.total}
            </span>
            <span>{percentage}%</span>
          </div>
          <div className="h-2 bg-bg-tertiary rounded-full overflow-hidden">
            <div
              className={`h-full transition-all duration-300 ${
                isCompleted ? 'bg-green-500' : isFailed ? 'bg-red-500' : 'bg-primary'
              }`}
              style={{ width: `${percentage}%` }}
            />
          </div>
        </div>

        {/* 当前单词 */}
        {isRunning && progress.currentWord && (
          <div className="flex items-center gap-2 text-xs text-text-secondary">
            <Sparkles size={12} className="text-primary" />
            <span>
              正在生成:{' '}
              <span className="font-mono font-semibold text-text-primary">
                {progress.currentWord}
              </span>
            </span>
          </div>
        )}

        {/* 完成统计 */}
        {isCompleted && (
          <div className="mt-2 pt-2 border-t border-border flex items-center justify-between text-xs">
            <span className="text-green-600 dark:text-green-400">✅ 成功 {progress.completed}</span>
            {progress.failed > 0 && (
              <span className="text-red-600 dark:text-red-400">❌ 失败 {progress.failed}</span>
            )}
          </div>
        )}

        {/* 提示信息 */}
        {isRunning && (
          <div className="mt-2 text-xs text-text-tertiary">💡 学习时内容将自动加载，无需等待</div>
        )}
      </div>
    </div>
  );
}
