// LearningStatsPanel - 学习统计面板

import { useLearningStats, useDueCards } from '../../hooks/useVocabulary';
import { useVocabularyStore, selectSessionStats } from '../../stores/vocabularyStore';

interface LearningStatsPanelProps {
  className?: string;
}

export function LearningStatsPanel({ className = '' }: LearningStatsPanelProps) {
  const { data: stats, isLoading } = useLearningStats();
  const { data: dueCards } = useDueCards();
  const sessionStats = useVocabularyStore(selectSessionStats);

  if (isLoading) {
    return (
      <div className={`bg-white rounded-lg shadow p-6 ${className}`}>
        <div className="text-gray-500">加载中...</div>
      </div>
    );
  }

  return (
    <div className={`bg-white rounded-lg shadow ${className}`}>
      {/* 总体统计 */}
      <div className="p-6 border-b">
        <h2 className="text-lg font-semibold mb-4">学习统计</h2>
        <div className="grid grid-cols-2 gap-4">
          <StatCard
            label="总卡牌数"
            value={stats?.total_cards ?? 0}
            icon="📚"
            color="blue"
          />
          <StatCard
            label="待复习"
            value={dueCards?.length ?? 0}
            icon="⏰"
            color="orange"
          />
          <StatCard
            label="今日学习"
            value={stats?.learned_today ?? 0}
            icon="📖"
            color="green"
          />
          <StatCard
            label="今日复习"
            value={stats?.reviewed_today ?? 0}
            icon="✅"
            color="purple"
          />
        </div>
      </div>

      {/* 当前会话统计 */}
      {sessionStats && (
        <div className="p-6">
          <h3 className="text-md font-semibold mb-4">本次会话</h3>
          <div className="space-y-3">
            <div className="flex justify-between items-center">
              <span className="text-gray-600">学习时长</span>
              <span className="font-medium">{formatDuration(sessionStats.duration)}</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-gray-600">已复习</span>
              <span className="font-medium">{sessionStats.cardsReviewed} 张</span>
            </div>
            <div className="flex justify-between items-center">
              <span className="text-gray-600">正确率</span>
              <span className="font-medium">{sessionStats.accuracy.toFixed(1)}%</span>
            </div>
            {/* 进度条 */}
            <div className="pt-2">
              <div className="h-2 bg-gray-200 rounded-full overflow-hidden">
                <div
                  className="h-full bg-green-500 transition-all duration-300"
                  style={{ width: `${sessionStats.accuracy}%` }}
                />
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

interface StatCardProps {
  label: string;
  value: number;
  icon: string;
  color: 'blue' | 'orange' | 'green' | 'purple';
}

function StatCard({ label, value, icon, color }: StatCardProps) {
  const colorClasses = {
    blue: 'bg-blue-50 text-blue-600',
    orange: 'bg-orange-50 text-orange-600',
    green: 'bg-green-50 text-green-600',
    purple: 'bg-purple-50 text-purple-600',
  };

  return (
    <div className={`p-4 rounded-lg ${colorClasses[color]}`}>
      <div className="flex items-center justify-between mb-2">
        <span className="text-2xl">{icon}</span>
        <span className="text-2xl font-bold">{value}</span>
      </div>
      <div className="text-sm opacity-80">{label}</div>
    </div>
  );
}

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);

  if (hours > 0) {
    return `${hours}小时${minutes % 60}分钟`;
  } else if (minutes > 0) {
    return `${minutes}分钟`;
  } else {
    return `${seconds}秒`;
  }
}
