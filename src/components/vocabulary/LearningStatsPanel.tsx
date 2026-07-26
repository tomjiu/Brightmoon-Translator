// LearningStatsPanel - 学习统计面板

import { useLearningStats, useDueCards } from '../../hooks/useVocabulary';
import { useVocabularyStore, selectSessionStats } from '../../stores/vocabularyStore';
import { BookOpen, Clock, CheckCircle, Target } from 'lucide-react';

interface LearningStatsPanelProps {
  className?: string;
}

export function LearningStatsPanel({ className = '' }: LearningStatsPanelProps) {
  const { data: stats, isLoading } = useLearningStats();
  const { data: dueCards } = useDueCards();
  const sessionStats = useVocabularyStore(selectSessionStats);

  if (isLoading) {
    return (
      <div className={className}>
        <div className="text-text-secondary text-sm">加载中...</div>
      </div>
    );
  }

  return (
    <div className={className}>
      {/* 总体统计 */}
      <div className="space-y-2">
        <h3 className="text-xs font-semibold text-text-secondary uppercase tracking-wide">
          学习统计
        </h3>
        <div className="grid grid-cols-2 gap-2">
          <StatCard label="总卡牌" value={stats?.total_cards ?? 0} icon={<BookOpen size={14} />} />
          <StatCard label="待复习" value={dueCards?.length ?? 0} icon={<Clock size={14} />} />
          <StatCard
            label="今日学习"
            value={stats?.learned_today ?? 0}
            icon={<Target size={14} />}
          />
          <StatCard
            label="今日复习"
            value={stats?.reviewed_today ?? 0}
            icon={<CheckCircle size={14} />}
          />
        </div>
      </div>

      {/* 当前会话统计 */}
      {sessionStats && (
        <div className="mt-4 space-y-2">
          <h3 className="text-xs font-semibold text-text-secondary uppercase tracking-wide">
            本次会话
          </h3>
          <div className="space-y-1.5 text-xs">
            <div className="flex justify-between">
              <span className="text-text-secondary">时长</span>
              <span className="text-text-primary font-medium">
                {formatDuration(sessionStats.duration)}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-text-secondary">已复习</span>
              <span className="text-text-primary font-medium">{sessionStats.cardsReviewed} 张</span>
            </div>
            <div className="flex justify-between">
              <span className="text-text-secondary">正确率</span>
              <span className="text-text-primary font-medium">
                {sessionStats.accuracy.toFixed(0)}%
              </span>
            </div>
            <div className="h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
              <div
                className="h-full bg-primary transition-all duration-300 rounded-full"
                style={{ width: `${Math.min(100, sessionStats.accuracy)}%` }}
              />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function StatCard({ label, value, icon }: { label: string; value: number; icon: React.ReactNode }) {
  return (
    <div className="p-2.5 bg-bg-primary rounded-lg border border-border">
      <div className="flex items-center justify-between mb-1">
        <span className="text-text-tertiary">{icon}</span>
        <span className="text-lg font-bold text-text-primary">{value}</span>
      </div>
      <div className="text-xs text-text-secondary">{label}</div>
    </div>
  );
}

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  if (hours > 0) return `${hours}h${minutes % 60}m`;
  if (minutes > 0) return `${minutes}m`;
  return `${seconds}s`;
}
