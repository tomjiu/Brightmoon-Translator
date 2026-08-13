import { useEffect, useState, type FC } from 'react';
import {
  Calendar,
  TrendingUp,
  Target,
  AlertCircle,
  BarChart3,
  Activity,
  RefreshCw,
  Download,
  TrendingDown,
  Minus,
  Heart,
  Star,
} from 'lucide-react';
import PageLayout from '../PageLayout';
import { WordDetailModal } from './WordDetailModal';
import {
  getLearningStatistics,
  getDailyActivity,
  getHeatmapData,
  getWeakWords,
  getRetentionCurve,
  getReviewForecastStats,
  type LearningStatistics,
  type DailyActivity,
  type HeatmapData,
  type WeakWord,
  type RetentionPoint,
  type ForecastPoint,
} from '../../services/statistics';
import {
  getWeakPointWords,
  resolveWeakPoint,
  type WeakPointWord,
} from '../../services/vocabulary';
import {
  getUserPreferences,
  getInferredWeakFields,
  type FieldPreference,
  type InferredWeakField,
} from '../../services/preference';
import { useToastStore } from '../../stores/toastStore';

export const LearningStatsDashboard: FC = () => {
  const addToast = useToastStore((s) => s.addToast);
  const [stats, setStats] = useState<LearningStatistics | null>(null);
  const [dailyActivity, setDailyActivity] = useState<DailyActivity[]>([]);
  const [heatmapData, setHeatmapData] = useState<HeatmapData[]>([]);
  const [weakWords, setWeakWords] = useState<WeakWord[]>([]);
  const [weakPointWords, setWeakPointWords] = useState<WeakPointWord[]>([]);
  const [retentionCurve, setRetentionCurve] = useState<RetentionPoint[]>([]);
  const [forecast, setForecast] = useState<ForecastPoint[]>([]);
  const [preferences, setPreferences] = useState<FieldPreference[]>([]);
  const [inferredWeak, setInferredWeak] = useState<InferredWeakField[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [selectedWord, setSelectedWord] = useState<string | null>(null);

  useEffect(() => {
    loadStatistics();
  }, []);

  const loadStatistics = async () => {
    try {
      setLoading(true);
      setRefreshing(true);
      const currentYear = new Date().getFullYear();

      const [statsData, activityData, heatmapData, weakWordsData, weakPointWordsData, prefs, inferred] =
        await Promise.all([
          getLearningStatistics(),
          getDailyActivity(30),
          getHeatmapData(currentYear),
          getWeakWords(10),
          getWeakPointWords(20),
          getUserPreferences(),
          getInferredWeakFields(),
        ]);

      // 并行获取新图表数据（不阻塞主流程）
      getRetentionCurve(90)
        .then(setRetentionCurve)
        .catch((error: unknown) => console.error('加载保留率曲线失败:', error));
      getReviewForecastStats(14)
        .then(setForecast)
        .catch((error: unknown) => console.error('加载复习量预测失败:', error));

      setStats(statsData);
      setDailyActivity(activityData);
      setHeatmapData(heatmapData);
      setWeakWords(weakWordsData);
      setWeakPointWords(weakPointWordsData);
      setPreferences(prefs);
      setInferredWeak(inferred);
    } catch (error) {
      console.error('加载统计数据失败:', error);
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  };

  const exportStatistics = () => {
    if (!stats) return;

    const exportData = {
      statistics: stats,
      dailyActivity,
      weakWords,
      exportedAt: new Date().toISOString(),
    };

    const blob = new Blob([JSON.stringify(exportData, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `学习统计_${new Date().toISOString().split('T')[0]}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const exportCSV = () => {
    if (!stats || dailyActivity.length === 0) return;

    const csv = [
      ['日期', '新学', '复习'],
      ...dailyActivity.map((d) => [d.date, d.newCards, d.reviewedCards]),
    ]
      .map((row) => row.join(','))
      .join('\n');

    const blob = new Blob([`\uFEFF${csv}`], { type: 'text/csv;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `学习活动_${new Date().toISOString().split('T')[0]}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center">
          <Activity className="w-12 h-12 mx-auto mb-4 animate-spin text-primary" />
          <p className="text-text-secondary">加载统计数据中...</p>
        </div>
      </div>
    );
  }

  if (!stats) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center">
          <AlertCircle className="w-12 h-12 mx-auto mb-4 text-text-secondary" />
          <p className="text-text-secondary">暂无统计数据</p>
        </div>
      </div>
    );
  }

  return (
    <PageLayout
      chrome="none"
      title="学习统计"
      icon={BarChart3}
      actions={
          <>
            <button
              onClick={loadStatistics}
              disabled={refreshing}
              className="flex items-center gap-2 px-4 py-2 bg-bg-tertiary hover:bg-bg-tertiary disabled:bg-bg-secondary disabled:text-text-secondary rounded-lg transition-colors"
            >
              <RefreshCw className={`w-4 h-4 ${refreshing ? 'animate-spin' : ''}`} />
              刷新
            </button>
            <div className="relative group">
              <button className="flex items-center gap-2 px-4 py-2 bg-primary hover:bg-primary-hover text-primary-fg rounded-lg transition-colors">
                <Download className="w-4 h-4" />
                导出
              </button>
              <div className="absolute right-0 mt-2 w-40 bg-bg-secondary rounded-lg shadow-lg opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all z-10">
                <button
                  onClick={exportCSV}
                  className="w-full px-4 py-2 text-left hover:bg-bg-tertiary rounded-t-lg transition-colors"
                >
                  导出 CSV
                </button>
                <button
                  onClick={exportStatistics}
                  className="w-full px-4 py-2 text-left hover:bg-bg-tertiary rounded-b-lg transition-colors"
                >
                  导出 JSON
                </button>
              </div>
            </div>
          </>
        }
      >

      {/* Statistics Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard
          icon={<Target className="w-6 h-6" />}
          label="总词汇量"
          value={stats.totalCards}
          color="blue"
        />
        <StatCard
          icon={<TrendingUp className="w-6 h-6" />}
          label="待复习"
          value={stats.dueCards}
          color="yellow"
        />
        <StatCard
          icon={<Calendar className="w-6 h-6" />}
          label="今日新学"
          value={stats.learnedToday}
          color="green"
        />
        <StatCard
          icon={<Activity className="w-6 h-6" />}
          label="今日复习"
          value={stats.reviewedToday}
          color="purple"
        />
      </div>

      {/* Detailed Stats */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <DetailCard label="连续学习天数" value={`${stats.streakDays} 天`} icon="🔥" />
        <DetailCard label="记忆保持率" value={`${stats.retentionRate.toFixed(1)}%`} icon="📊" />
        <DetailCard label="总复习次数" value={stats.totalReviews} icon="✅" />
      </div>

      {/* T12: 偏好概览 */}
      <div className="bg-bg-secondary rounded-lg p-6">
        <h3 className="text-lg font-semibold mb-4 flex items-center gap-2">
          <Heart className="w-5 h-5 text-pink-400" />
          偏好概览
        </h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          <div>
            <div className="flex items-center gap-2 mb-3">
              <Star className="w-4 h-4 text-yellow-400" />
              <h4 className="text-sm font-medium text-text-primary">表达偏好</h4>
            </div>
            <div className="space-y-2 text-sm">
              {preferences.map((p) => (
                <div key={p.field} className="flex items-center justify-between">
                  <span className="text-text-primary">{p.field}</span>
                  <div className="flex items-center gap-2">
                    <div className="flex gap-0.5">
                      {[1, 2, 3, 4, 5].map((i) => (
                        <Star
                          key={i}
                          size={12}
                          className={
                            i <= Math.round(p.avgRating)
                              ? 'text-yellow-400 fill-current'
                              : 'text-text-secondary'
                          }
                        />
                      ))}
                    </div>
                    <span className="text-text-secondary">×{p.ratedCount}</span>
                  </div>
                </div>
              ))}
              {preferences.length === 0 && (
                <p className="text-text-secondary">暂无评分，打开词详情给 AI 内容打分</p>
              )}
            </div>
          </div>
          <div>
            <div className="flex items-center gap-2 mb-3">
              <AlertCircle className="w-4 h-4 text-red-400" />
              <h4 className="text-sm font-medium text-text-primary">观察偏好</h4>
            </div>
            <div className="space-y-2 text-sm">
              {inferredWeak.map((w) => (
                <div key={w.field} className="flex items-center justify-between">
                  <span className="text-text-primary">{w.field}</span>
                  <span className="text-red-400">{Math.round(w.strength * 100)}% 错误率</span>
                </div>
              ))}
              {inferredWeak.length === 0 && <p className="text-text-secondary">暂无弱项，继续保持</p>}
            </div>
          </div>
        </div>
      </div>

      {/* Heatmap */}
      <div className="bg-bg-secondary rounded-lg p-6">
        <h3 className="text-lg font-semibold mb-4 flex items-center gap-2">
          <Calendar className="w-5 h-5" />
          学习热力图
        </h3>
        <Heatmap data={heatmapData} />
      </div>

      {/* Daily Activity Chart */}
      <div className="bg-bg-secondary rounded-lg p-6">
        <h3 className="text-lg font-semibold mb-4 flex items-center gap-2">
          <BarChart3 className="w-5 h-5" />
          最近30天学习趋势
        </h3>
        <DailyActivityChart data={dailyActivity} />
      </div>

      {/* Retention Curve */}
      <div className="bg-bg-secondary rounded-lg p-6">
        <h3 className="text-lg font-semibold mb-4 flex items-center gap-2">
          <TrendingUp className="w-5 h-5" />
          记忆保留率曲线
        </h3>
        <RetentionCurveChart data={retentionCurve} />
      </div>

      {/* Review Forecast */}
      <div className="bg-bg-secondary rounded-lg p-6">
        <h3 className="text-lg font-semibold mb-4 flex items-center gap-2">
          <Calendar className="w-5 h-5" />
          未来14天复习量预测
        </h3>
        <ForecastChart data={forecast} />
      </div>

      {/* Weak Words */}
      <div className="bg-bg-secondary rounded-lg p-6">
        <h3 className="text-lg font-semibold mb-4 flex items-center gap-2">
          <AlertCircle className="w-5 h-5" />
          薄弱词汇（需加强）
        </h3>
        <WeakWordsList words={weakWords} onWordClick={setSelectedWord} />
      </div>

      {/* T8: 弱点错误明细（答错追踪的精确错误点） */}
      <div className="bg-bg-secondary rounded-lg p-6">
        <h3 className="text-lg font-semibold mb-4 flex items-center gap-2">
          <Target className="w-5 h-5" />
          弱点错误明细
        </h3>
        <WeakPointWordsList
          words={weakPointWords}
          onWordClick={setSelectedWord}
          onResolve={async (cardId) => {
            try {
              await resolveWeakPoint(cardId);
              addToast({ type: 'success', message: '弱点已标记解决', duration: 3000 });
              loadStatistics();
            } catch (error) {
              addToast({ type: 'error', message: '标记失败，请重试', duration: 3000 });
            }
          }}
        />
      </div>

      {/* Word Detail Modal */}
      {selectedWord && (
        <WordDetailModal word={selectedWord} onClose={() => setSelectedWord(null)} />
      )}
    </PageLayout>
  );
};

// ============================================
// Sub-components
// ============================================

interface StatCardProps {
  icon: React.ReactNode;
  label: string;
  value: number;
  color: 'blue' | 'yellow' | 'green' | 'purple';
  trend?: 'up' | 'down' | 'stable';
  trendValue?: number;
}

const StatCard: FC<StatCardProps> = ({ icon, label, value, color, trend, trendValue }) => {
  const colorClasses = {
    blue: 'bg-primary/10 text-primary',
    yellow: 'bg-yellow-500/20 text-yellow-400',
    green: 'bg-green-500/20 text-green-400',
    purple: 'bg-primary/10 text-primary',
  };

  const getTrendIcon = () => {
    if (!trend) return null;
    if (trend === 'up') return <TrendingUp className="w-4 h-4 text-green-400" />;
    if (trend === 'down') return <TrendingDown className="w-4 h-4 text-red-400" />;
    return <Minus className="w-4 h-4 text-text-secondary" />;
  };

  return (
    <div className="bg-bg-secondary rounded-lg p-6 hover:bg-bg-tertiary transition-colors">
      <div className={`inline-flex p-3 rounded-lg mb-4 ${colorClasses[color]}`}>{icon}</div>
      <div className="ui-stat mb-1">{value}</div>
      <div className="flex items-center justify-between">
        <div className="ui-caption">{label}</div>
        {trend && trendValue !== undefined && (
          <div className="flex items-center gap-1 text-xs">
            {getTrendIcon()}
            <span
              className={
                trend === 'up'
                  ? 'text-green-400'
                  : trend === 'down'
                    ? 'text-red-400'
                    : 'text-text-secondary'
              }
            >
              {trendValue > 0 ? '+' : ''}
              {trendValue}
            </span>
          </div>
        )}
      </div>
    </div>
  );
};

interface DetailCardProps {
  label: string;
  value: string | number;
  icon: string;
}

const DetailCard: FC<DetailCardProps> = ({ label, value, icon }) => {
  return (
    <div className="bg-bg-secondary rounded-lg p-4 flex items-center gap-4">
      <div className="text-4xl">{icon}</div>
      <div>
        <div className="ui-stat text-[1.25rem]">{value}</div>
        <div className="ui-caption">{label}</div>
      </div>
    </div>
  );
};

interface HeatmapProps {
  data: HeatmapData[];
}

const Heatmap: FC<HeatmapProps> = ({ data }) => {
  const today = new Date();
  const startDate = new Date(today);
  startDate.setDate(today.getDate() - 364);

  const dataMap = new Map(data.map((d) => [d.date, d.count]));

  const weeks: Date[][] = [];
  let currentWeek: Date[] = [];
  const current = new Date(startDate);

  // Fill first week with padding
  while (current.getDay() !== 0) {
    currentWeek.push(new Date(current));
    current.setDate(current.getDate() + 1);
  }

  while (current <= today) {
    if (current.getDay() === 0 && currentWeek.length > 0) {
      weeks.push([...currentWeek]);
      currentWeek = [];
    }
    currentWeek.push(new Date(current));
    current.setDate(current.getDate() + 1);
  }
  if (currentWeek.length > 0) {
    weeks.push(currentWeek);
  }

  const getColor = (count: number) => {
    if (count === 0) return 'bg-bg-tertiary';
    if (count <= 2) return 'bg-green-900';
    if (count <= 5) return 'bg-green-700';
    if (count <= 10) return 'bg-green-500';
    return 'bg-green-400';
  };

  // Month labels
  const monthLabels: Array<{ month: string; weekIndex: number }> = [];
  let lastMonth = -1;
  weeks.forEach((week, idx) => {
    const firstDay = week[0];
    if (firstDay) {
      const month = firstDay.getMonth();
      if (month !== lastMonth) {
        monthLabels.push({
          month: [
            '1月',
            '2月',
            '3月',
            '4月',
            '5月',
            '6月',
            '7月',
            '8月',
            '9月',
            '10月',
            '11月',
            '12月',
          ][month],
          weekIndex: idx,
        });
        lastMonth = month;
      }
    }
  });

  return (
    <div className="overflow-x-auto">
      {/* Month labels */}
      <div className="flex gap-1 mb-2 text-xs text-text-secondary">
        {monthLabels.map((label, idx) => (
          <div key={idx} className="absolute" style={{ left: `${label.weekIndex * 16}px` }}>
            {label.month}
          </div>
        ))}
      </div>

      <div className="inline-flex gap-1 mt-6">
        {weeks.map((week, weekIdx) => (
          <div key={weekIdx} className="flex flex-col gap-1">
            {Array.from({ length: 7 }).map((_, dayIdx) => {
              const date = week[dayIdx];
              if (!date) {
                return <div key={dayIdx} className="w-3 h-3" />;
              }
              const dateStr = date.toISOString().split('T')[0];
              const count = dataMap.get(dateStr) || 0;
              return (
                <div
                  key={dayIdx}
                  className={`w-3 h-3 rounded-sm ${getColor(count)} hover:ring-2 hover:ring-primary/50 transition-all cursor-pointer`}
                  title={`${dateStr}: ${count} 个词`}
                />
              );
            })}
          </div>
        ))}
      </div>
      <div className="flex items-center gap-2 mt-4 text-xs text-text-secondary">
        <span>少</span>
        <div className="flex gap-1">
          <div className="w-3 h-3 rounded-sm bg-bg-tertiary" />
          <div className="w-3 h-3 rounded-sm bg-green-900" />
          <div className="w-3 h-3 rounded-sm bg-green-700" />
          <div className="w-3 h-3 rounded-sm bg-green-500" />
          <div className="w-3 h-3 rounded-sm bg-green-400" />
        </div>
        <span>多</span>
      </div>
    </div>
  );
};

interface DailyActivityChartProps {
  data: DailyActivity[];
}

const DailyActivityChart: FC<DailyActivityChartProps> = ({ data }) => {
  if (data.length === 0) {
    return <div className="text-text-secondary text-center py-8">暂无数据</div>;
  }

  const maxValue = Math.max(...data.map((d) => d.newCards + d.reviewedCards), 1);

  return (
    <div className="space-y-4">
      <div className="flex items-end gap-1 h-48">
        {data.map((day, idx) => {
          const total = day.newCards + day.reviewedCards;
          const heightPercent = (total / maxValue) * 100;
          const newPercent = total > 0 ? (day.newCards / total) * 100 : 0;

          return (
            <div key={idx} className="flex-1 flex flex-col justify-end group relative">
              <div
                className="w-full rounded-t transition-all duration-200 hover:opacity-80"
                style={{ height: `${heightPercent}%` }}
              >
                <div
                  className="bg-green-500 rounded-t"
                  style={{ height: `${newPercent}%` }}
                  title={`新学: ${day.newCards}`}
                />
                <div
                  className="bg-primary"
                  style={{ height: `${100 - newPercent}%` }}
                  title={`复习: ${day.reviewedCards}`}
                />
              </div>
              <div className="absolute bottom-0 left-1/2 transform -translate-x-1/2 translate-y-full mt-2 px-2 py-1 bg-bg-primary rounded text-xs whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10">
                {day.date.slice(5)}
                <br />
                新: {day.newCards} | 复: {day.reviewedCards}
              </div>
            </div>
          );
        })}
      </div>
      <div className="flex items-center gap-4 text-sm">
        <div className="flex items-center gap-2">
          <div className="w-3 h-3 rounded bg-green-500" />
          <span className="text-text-secondary">新学</span>
        </div>
        <div className="flex items-center gap-2">
          <div className="w-3 h-3 rounded bg-primary" />
          <span className="text-text-secondary">复习</span>
        </div>
      </div>
    </div>
  );
};

interface RetentionCurveChartProps {
  data: RetentionPoint[];
}

const RetentionCurveChart: FC<RetentionCurveChartProps> = ({ data }) => {
  if (data.length === 0) {
    return <div className="text-text-secondary text-center py-8">暂无足够的复习数据</div>;
  }

  const maxX = Math.max(...data.map((d) => d.intervalDays), 1);
  const minY = Math.min(...data.map((d) => d.retention), 0);
  const maxY = 100;

  const points = data
    .map((d) => {
      const x = (d.intervalDays / maxX) * 100;
      const y = 100 - ((d.retention - minY) / (maxY - minY || 1)) * 100;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(' ');

  return (
    <div>
      <svg viewBox="0 0 100 100" preserveAspectRatio="none" className="w-full h-48">
        <polyline
          points={points}
          fill="none"
          stroke="var(--color-primary)"
          strokeWidth="1"
          strokeLinecap="round"
          strokeLinejoin="round"
          vectorEffect="non-scaling-stroke"
        />
        {data.map((d, idx) => {
          const x = (d.intervalDays / maxX) * 100;
          const y = 100 - ((d.retention - minY) / (maxY - minY || 1)) * 100;
          return (
            <circle
              key={idx}
              cx={x}
              cy={y}
              r="1.2"
              fill="var(--color-primary)"
              vectorEffect="non-scaling-stroke"
            />
          );
        })}
      </svg>
      <div className="flex justify-between text-xs text-text-secondary mt-2">
        <span>间隔 {data[0]?.intervalDays} 天</span>
        <span>间隔 {maxX}+ 天</span>
      </div>
      <div className="mt-4 flex flex-wrap gap-3">
        {data.map((d, idx) => (
          <div key={idx} className="text-xs bg-bg-tertiary rounded px-2 py-1">
            <span className="text-text-secondary">{d.intervalDays}+ 天:</span>{' '}
            <span className="text-primary">{d.retention.toFixed(1)}%</span>
            <span className="text-text-secondary"> ({d.reviewCount}次)</span>
          </div>
        ))}
      </div>
    </div>
  );
};

interface ForecastChartProps {
  data: ForecastPoint[];
}

const ForecastChart: FC<ForecastChartProps> = ({ data }) => {
  if (data.length === 0) {
    return <div className="text-text-secondary text-center py-8">暂无数据</div>;
  }

  const maxValue = Math.max(...data.map((d) => d.dueCount), 1);

  return (
    <div>
      <div className="flex items-end gap-1 h-40">
        {data.map((d, idx) => {
          const heightPercent = (d.dueCount / maxValue) * 100;
          return (
            <div key={idx} className="flex-1 flex flex-col justify-end group relative">
              <div
                className="w-full bg-primary/70 rounded-t transition-all duration-200 hover:bg-primary"
                style={{ height: `${heightPercent}%` }}
                title={`${d.date}: ${d.dueCount} 个待复习`}
              />
              <div className="absolute bottom-0 left-1/2 transform -translate-x-1/2 translate-y-full mt-2 px-2 py-1 bg-bg-primary rounded text-xs whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10">
                {d.date.slice(5)}
                <br />
                {d.dueCount} 个待复习
              </div>
            </div>
          );
        })}
      </div>
      <div className="text-xs text-text-secondary mt-4">
        <span>未来 {data.length} 天预计复习: </span>
        <span className="text-primary">
          {data.reduce((sum, d) => sum + d.dueCount, 0)} 次
        </span>
      </div>
    </div>
  );
};

interface WeakWordsListProps {
  words: WeakWord[];
  onWordClick?: (word: string) => void;
}const WeakWordsList: FC<WeakWordsListProps> = ({ words, onWordClick }) => {
  if (words.length === 0) {
    return <div className="text-text-secondary text-center py-8">太棒了！暂无薄弱词汇 🎉</div>;
  }

  const handleWordClick = (word: string) => {
    if (onWordClick) {
      onWordClick(word);
    }
  };

  return (
    <div className="space-y-3">
      {words.map((word, idx) => {
        const errorRate = (word.againCount / word.totalReviews) * 100;
        return (
          <div
            key={idx}
            onClick={() => handleWordClick(word.word)}
            className="bg-bg-tertiary rounded-lg p-4 hover:bg-bg-tertiary transition-colors cursor-pointer group"
          >
            <div className="flex items-center justify-between mb-2">
              <div className="flex items-center gap-3">
                <span className="text-lg font-semibold group-hover:text-primary transition-colors">
                  {word.word}
                </span>
                <span className="text-xs px-2 py-1 rounded bg-red-500/20 text-red-400">
                  错误率 {errorRate.toFixed(1)}%
                </span>
              </div>
              <div className="text-sm text-text-secondary">
                {word.againCount} / {word.totalReviews} 次
              </div>
            </div>
            <div className="flex items-center gap-4 text-xs text-text-secondary">
              <span>难度: {word.difficulty.toFixed(2)}</span>
              <span>稳定性: {word.stability.toFixed(2)}</span>
              <span>最后复习: {new Date(word.lastReview * 1000).toLocaleDateString()}</span>
            </div>
            <div className="mt-2 opacity-0 group-hover:opacity-100 transition-opacity text-xs text-primary">
              点击查看详情 →
            </div>
          </div>
        );
      })}
    </div>
  );
};

interface WeakPointWordsListProps {
  words: WeakPointWord[];
  onWordClick?: (word: string) => void;
  onResolve?: (cardId: string) => Promise<void>;
}

const WeakPointWordsList: FC<WeakPointWordsListProps> = ({ words, onWordClick, onResolve }) => {
  if (words.length === 0) {
    return <div className="text-text-secondary text-center py-8">暂无弱点错误记录</div>;
  }

  const getErrorTypeLabel = (errorType: string) => {
    switch (errorType) {
      case 'definition':
        return '释义';
      case 'pronunciation':
        return '发音';
      case 'spelling':
        return '拼写';
      case 'usage':
        return '用法';
      default:
        return errorType;
    }
  };

  return (
    <div className="space-y-3">
      {words.map((word, idx) => (
        <div
          key={idx}
          onClick={() => onWordClick?.(word.word)}
          className="bg-bg-tertiary rounded-lg p-4 hover:bg-bg-tertiary transition-colors cursor-pointer group"
        >
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-3">
              <span className="text-lg font-semibold group-hover:text-primary transition-colors">
                {word.word}
              </span>
              <span className="text-xs px-2 py-1 rounded bg-red-500/20 text-red-400">
                {getErrorTypeLabel(word.error_type)} × {word.count}
              </span>
            </div>
            <button
              onClick={(e) => {
                e.stopPropagation();
                void onResolve?.(word.card_id);
              }}
              className="text-xs px-2 py-1 rounded bg-green-500/20 text-green-300 hover:bg-green-500/30 transition-colors"
            >
              已解决
            </button>
          </div>
          <div className="text-xs text-text-secondary">
            最近出错: {new Date(word.last_occurred_at * 1000).toLocaleString()}
          </div>
        </div>
      ))}
    </div>
  );
};

