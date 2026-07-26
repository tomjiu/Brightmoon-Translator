import { useEffect, useState } from 'react';
import {
  BarChart3,
  TrendingUp,
  Clock,
  Brain,
  Loader2,
  AlertTriangle,
  CheckCircle,
  Info,
} from 'lucide-react';
import {
  getFsrsAnalysis,
  getForgettingCurve,
  getReviewForecast,
  getBestStudyTime,
  getDifficultyDistribution,
  type FsrsAnalysis,
  type ForgettingCurvePoint,
  type ReviewForecast,
  type StudyTimeSlot,
  type DifficultyBucket,
} from '../services/fsrsOptimization';

type Tab = 'overview' | 'forgetting' | 'forecast' | 'timing' | 'difficulty';

const PARAM_LABELS = [
  'S0(again)',
  'S0(hard)',
  'S0(good)',
  'S0(easy)',
  'D0',
  'D0_factor',
  'D_factor',
  'stab_factor',
  'stab_exp',
  'easy_factor',
  'easy_exp',
  'fail_factor',
  'fail_diff',
  'fail_stab',
  'hard_factor',
  'hard_exp',
  'hard_mult',
];

export default function FsrsOptimization() {
  const [analysis, setAnalysis] = useState<FsrsAnalysis | null>(null);
  const [forgettingCurve, setForgettingCurve] = useState<ForgettingCurvePoint[]>([]);
  const [forecast, setForecast] = useState<ReviewForecast[]>([]);
  const [bestTime, setBestTime] = useState<StudyTimeSlot[]>([]);
  const [difficulty, setDifficulty] = useState<DifficultyBucket[]>([]);
  const [tab, setTab] = useState<Tab>('overview');
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      setLoading(true);
      const [analysisData, forecastData, bestTimeData, difficultyData] = await Promise.all([
        getFsrsAnalysis(),
        getReviewForecast(30),
        getBestStudyTime(),
        getDifficultyDistribution(),
      ]);

      setAnalysis(analysisData);
      setForecast(forecastData);
      setBestTime(bestTimeData);
      setDifficulty(difficultyData);

      // 使用平均稳定性加载遗忘曲线
      if (analysisData.avgStability > 0) {
        const curveData = await getForgettingCurve(analysisData.avgStability);
        setForgettingCurve(curveData);
      }
    } catch (error) {
      console.error('加载 FSRS 数据失败:', error);
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2 className="w-8 h-8 animate-spin text-primary" />
        <span className="ml-3 text-text-secondary">加载 FSRS 分析数据...</span>
      </div>
    );
  }

  if (!analysis) {
    return (
      <div className="flex items-center justify-center h-full">
        <AlertTriangle className="w-8 h-8 text-yellow-400" />
        <span className="ml-3">暂无学习数据，无法进行分析</span>
      </div>
    );
  }

  const maxForecast = Math.max(...forecast.map((f) => f.dueCount), 1);
  const maxDifficulty = Math.max(...difficulty.map((d) => d.count), 1);
  const bestHour =
    bestTime.length > 0 ? bestTime.reduce((a, b) => (a.correctRate > b.correctRate ? a : b)) : null;

  return (
    <div className="h-full overflow-y-auto p-6 space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold flex items-center gap-2">
          <Brain className="w-7 h-7" />
          FSRS 算法分析
        </h1>
        <p className="text-sm text-text-secondary mt-1">
          基于你的学习数据，分析记忆算法效果并提供优化建议
        </p>
      </div>

      {/* Tabs */}
      <div className="flex gap-2 border-b border-border pb-2">
        {[
          { id: 'overview' as Tab, label: '总览', icon: <BarChart3 className="w-4 h-4" /> },
          { id: 'forgetting' as Tab, label: '遗忘曲线', icon: <TrendingUp className="w-4 h-4" /> },
          { id: 'forecast' as Tab, label: '复习预测', icon: <Clock className="w-4 h-4" /> },
          { id: 'timing' as Tab, label: '最佳时段', icon: <Clock className="w-4 h-4" /> },
          { id: 'difficulty' as Tab, label: '难度分布', icon: <BarChart3 className="w-4 h-4" /> },
        ].map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
              tab === t.id
                ? 'bg-primary text-primary-fg'
                : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
            }`}
          >
            {t.icon}
            {t.label}
          </button>
        ))}
      </div>

      {/* Tab Content */}
      {tab === 'overview' && (
        <div className="space-y-6">
          {/* Key Metrics */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <MetricCard
              label="记忆保持率"
              value={`${analysis.retentionRate.toFixed(1)}%`}
              color={
                analysis.retentionRate >= 80
                  ? 'green'
                  : analysis.retentionRate >= 60
                    ? 'yellow'
                    : 'red'
              }
            />
            <MetricCard
              label="平均稳定性"
              value={`${(analysis.avgStability / 86400).toFixed(1)}天`}
              color="blue"
            />
            <MetricCard
              label="平均难度"
              value={analysis.avgDifficulty.toFixed(2)}
              color={analysis.avgDifficulty <= 5 ? 'green' : 'yellow'}
            />
            <MetricCard label="总遗忘次数" value={analysis.totalLapses.toString()} color="red" />
          </div>

          {/* Optimization Suggestion */}
          <div className="bg-bg-secondary rounded-lg p-6 border border-border">
            <div className="flex items-center gap-2 mb-4">
              <Info className="w-5 h-5 text-primary" />
              <h3 className="font-semibold">优化建议</h3>
            </div>
            <div className="space-y-3 text-sm text-text-secondary">
              {analysis.retentionRate < 70 && (
                <div className="flex items-start gap-2">
                  <AlertTriangle className="w-4 h-4 text-yellow-400 mt-0.5 flex-shrink-0" />
                  <p>
                    记忆保持率偏低（{analysis.retentionRate.toFixed(1)}
                    %），建议：每天坚持复习，减少每次学习的新词数量，确保消化已学内容
                  </p>
                </div>
              )}
              {analysis.retentionRate >= 80 && (
                <div className="flex items-start gap-2">
                  <CheckCircle className="w-4 h-4 text-green-400 mt-0.5 flex-shrink-0" />
                  <p>
                    记忆保持率优秀（{analysis.retentionRate.toFixed(1)}%）！可以适当增加每日学习量
                  </p>
                </div>
              )}
              {analysis.avgDifficulty > 7 && (
                <div className="flex items-start gap-2">
                  <AlertTriangle className="w-4 h-4 text-yellow-400 mt-0.5 flex-shrink-0" />
                  <p>
                    平均难度偏高（{analysis.avgDifficulty.toFixed(2)}），说明学习的词较难，建议搭配
                    AI 助记法强化记忆
                  </p>
                </div>
              )}
              {bestHour && (
                <div className="flex items-start gap-2">
                  <CheckCircle className="w-4 h-4 text-primary mt-0.5 flex-shrink-0" />
                  <p>
                    你的最佳学习时段是 <strong>{bestHour.label}</strong>（{bestHour.hour}
                    :00），正确率 {bestHour.correctRate.toFixed(0)}%
                  </p>
                </div>
              )}
            </div>
          </div>

          {/* FSRS Parameters */}
          <div className="bg-bg-secondary rounded-lg p-6 border border-border">
            <div className="flex items-center gap-2 mb-4">
              <Brain className="w-5 h-5 text-primary" />
              <h3 className="font-semibold">当前 FSRS-4.5 参数</h3>
            </div>
            <div className="grid grid-cols-4 md:grid-cols-6 gap-2">
              {analysis.currentParams.map((param, idx) => (
                <div key={idx} className="bg-bg-primary rounded px-2 py-1.5">
                  <div className="text-xs text-text-secondary truncate">
                    {PARAM_LABELS[idx] || `w${idx}`}
                  </div>
                  <div className="text-sm font-mono">{param.toFixed(3)}</div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {tab === 'forgetting' && (
        <div className="space-y-6">
          <div className="bg-bg-secondary rounded-lg p-6 border border-border">
            <h3 className="font-semibold mb-4">遗忘曲线（理论模型）</h3>
            <p className="text-sm text-text-secondary mb-6">
              基于当前平均稳定性 {(analysis.avgStability / 86400).toFixed(1)} 天计算的记忆保持率
            </p>

            {/* Simple SVG Chart */}
            <div className="h-64 relative">
              <svg viewBox="0 0 600 200" className="w-full h-full">
                {/* Grid lines */}
                {[0, 0.25, 0.5, 0.75, 1].map((y) => (
                  <line
                    key={y}
                    x1="40"
                    y1={200 - y * 180}
                    x2="580"
                    y2={200 - y * 180}
                    stroke="currentColor"
                    strokeOpacity="0.1"
                  />
                ))}

                {/* Y axis labels */}
                {[0, 25, 50, 75, 100].map((v) => (
                  <text
                    key={v}
                    x="35"
                    y={202 - (v / 100) * 180}
                    textAnchor="end"
                    className="fill-text-secondary"
                    fontSize="10"
                  >
                    {v}%
                  </text>
                ))}

                {/* Curve */}
                {forgettingCurve.length > 1 && (
                  <path
                    d={forgettingCurve
                      .map((p, i) => {
                        const x = 40 + (i / 89) * 540;
                        const y = 200 - p.retention * 180;
                        return `${i === 0 ? 'M' : 'L'}${x},${y}`;
                      })
                      .join(' ')}
                    fill="none"
                    stroke="var(--color-primary)"
                    strokeWidth="2"
                  />
                )}

                {/* X axis labels */}
                {[0, 15, 30, 45, 60, 75, 90].map((d) => (
                  <text
                    key={d}
                    x={40 + (d / 89) * 540}
                    y="215"
                    textAnchor="middle"
                    className="fill-text-secondary"
                    fontSize="10"
                  >
                    {d}天
                  </text>
                ))}

                {/* 80% threshold line */}
                <line
                  x1="40"
                  y1={200 - 0.8 * 180}
                  x2="580"
                  y2={200 - 0.8 * 180}
                  stroke="var(--color-success)"
                  strokeDasharray="4"
                  strokeOpacity="0.5"
                />
                <text x="585" y={202 - 0.8 * 180} className="fill-text-secondary" fontSize="10">
                  80%
                </text>
              </svg>
            </div>
          </div>
        </div>
      )}

      {tab === 'forecast' && (
        <div className="space-y-6">
          <div className="bg-bg-secondary rounded-lg p-6 border border-border">
            <h3 className="font-semibold mb-2">未来 30 天复习量预测</h3>
            <p className="text-sm text-text-secondary mb-6">
              基于当前卡牌的 FSRS 状态预测每日待复习量
            </p>

            <div className="flex items-end gap-1 h-48">
              {forecast.map((day, idx) => {
                const height = (day.dueCount / maxForecast) * 100;
                const isToday = idx === 0;
                return (
                  <div key={idx} className="flex-1 flex flex-col items-center group relative">
                    <div
                      className={`w-full rounded-t transition-all duration-200 ${
                        isToday ? 'bg-primary' : 'bg-white/40 hover:bg-primary'
                      }`}
                      style={{ height: `${Math.max(height, 1)}%` }}
                    />
                    <div className="absolute bottom-0 left-1/2 -translate-x-1/2 translate-y-full mt-2 px-2 py-1 bg-bg-primary rounded text-xs whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10 border border-border">
                      {day.date.slice(5)}: {day.dueCount} 词
                    </div>
                  </div>
                );
              })}
            </div>
            <div className="flex justify-between text-xs text-text-secondary mt-2 px-1">
              <span>今天</span>
              <span>+30天</span>
            </div>
          </div>

          {/* Peak forecast */}
          {forecast.length > 0 &&
            (() => {
              const peak = forecast.reduce((a, b) => (a.dueCount > b.dueCount ? a : b));
              const totalDue = forecast.reduce((s, f) => s + f.dueCount, 0);
              return (
                <div className="grid grid-cols-2 gap-4">
                  <div className="bg-bg-secondary rounded-lg p-4 border border-border">
                    <div className="text-sm text-text-secondary">30天总复习量</div>
                    <div className="text-2xl font-bold">{totalDue}</div>
                    <div className="text-xs text-text-secondary">
                      平均每天 {Math.round(totalDue / 30)} 词
                    </div>
                  </div>
                  <div className="bg-bg-secondary rounded-lg p-4 border border-border">
                    <div className="text-sm text-text-secondary">峰值日期</div>
                    <div className="text-2xl font-bold">{peak.dueCount}</div>
                    <div className="text-xs text-text-secondary">{peak.date}</div>
                  </div>
                </div>
              );
            })()}
        </div>
      )}

      {tab === 'timing' && (
        <div className="space-y-6">
          <div className="bg-bg-secondary rounded-lg p-6 border border-border">
            <h3 className="font-semibold mb-2">各时段学习效果</h3>
            <p className="text-sm text-text-secondary mb-6">
              基于历史复习数据统计各时段的正确率，找到你的最佳学习时间
            </p>

            {bestTime.length === 0 ? (
              <p className="text-text-secondary text-center py-8">暂无复习数据</p>
            ) : (
              <div className="space-y-3">
                {bestTime.map((slot) => {
                  const barWidth = slot.correctRate;
                  return (
                    <div key={slot.hour} className="flex items-center gap-3">
                      <div className="w-12 text-sm text-text-secondary font-mono">
                        {slot.hour}:00
                      </div>
                      <div className="w-16 text-xs text-text-secondary">{slot.label}</div>
                      <div className="flex-1 h-6 bg-bg-primary rounded-full overflow-hidden">
                        <div
                          className={`h-full rounded-full transition-all duration-500 ${
                            slot.correctRate >= 80
                              ? 'bg-green-500'
                              : slot.correctRate >= 60
                                ? 'bg-yellow-500'
                                : 'bg-red-500'
                          }`}
                          style={{ width: `${barWidth}%` }}
                        />
                      </div>
                      <div className="w-16 text-right text-sm font-mono">
                        {slot.correctRate.toFixed(0)}%
                      </div>
                      <div className="w-16 text-right text-xs text-text-secondary">
                        {slot.reviewCount}次
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          {bestHour && (
            <div className="bg-green-500/10 border border-green-500/30 rounded-lg p-4">
              <div className="flex items-center gap-2">
                <CheckCircle className="w-5 h-5 text-green-400" />
                <span className="font-semibold">建议</span>
              </div>
              <p className="text-sm text-text-secondary mt-2">
                你在{' '}
                <strong>
                  {bestHour.label}（{bestHour.hour}:00）
                </strong>
                的学习效果最好， 正确率 {bestHour.correctRate.toFixed(0)}
                %，建议将主要学习安排在这个时段。
              </p>
            </div>
          )}
        </div>
      )}

      {tab === 'difficulty' && (
        <div className="space-y-6">
          <div className="bg-bg-secondary rounded-lg p-6 border border-border">
            <h3 className="font-semibold mb-2">卡牌难度分布</h3>
            <p className="text-sm text-text-secondary mb-6">
              各难度区间的卡牌数量分布（1=最简单，10=最难）
            </p>

            <div className="flex items-end gap-2 h-48">
              {difficulty.map((bucket, idx) => {
                const height = (bucket.count / maxDifficulty) * 100;
                return (
                  <div key={idx} className="flex-1 flex flex-col items-center group relative">
                    <div
                      className={`w-full rounded-t transition-all duration-200 ${
                        idx < 3
                          ? 'bg-green-500 hover:bg-green-400'
                          : idx < 6
                            ? 'bg-yellow-500 hover:bg-yellow-400'
                            : 'bg-red-500 hover:bg-red-400'
                      }`}
                      style={{ height: `${Math.max(height, 1)}%` }}
                    />
                    <div className="absolute bottom-0 left-1/2 -translate-x-1/2 translate-y-full mt-2 px-2 py-1 bg-bg-primary rounded text-xs whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10 border border-border">
                      {bucket.rangeStart.toFixed(0)}-{bucket.rangeEnd.toFixed(0)}: {bucket.count} 张
                    </div>
                  </div>
                );
              })}
            </div>
            <div className="flex justify-between text-xs text-text-secondary mt-2 px-1">
              <span>1（简单）</span>
              <span>5（中等）</span>
              <span>10（最难）</span>
            </div>
          </div>

          {/* Summary */}
          <div className="grid grid-cols-3 gap-4">
            <div className="bg-bg-secondary rounded-lg p-4 border border-border text-center">
              <div className="text-2xl font-bold text-green-400">
                {difficulty.slice(0, 3).reduce((s, d) => s + d.count, 0)}
              </div>
              <div className="text-xs text-text-secondary">简单 (1-3)</div>
            </div>
            <div className="bg-bg-secondary rounded-lg p-4 border border-border text-center">
              <div className="text-2xl font-bold text-yellow-400">
                {difficulty.slice(3, 6).reduce((s, d) => s + d.count, 0)}
              </div>
              <div className="text-xs text-text-secondary">中等 (4-6)</div>
            </div>
            <div className="bg-bg-secondary rounded-lg p-4 border border-border text-center">
              <div className="text-2xl font-bold text-red-400">
                {difficulty.slice(6).reduce((s, d) => s + d.count, 0)}
              </div>
              <div className="text-xs text-text-secondary">困难 (7-10)</div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function MetricCard({
  label,
  value,
  color,
}: {
  label: string;
  value: string;
  color: 'green' | 'yellow' | 'red' | 'blue';
}) {
  const colorMap = {
    green: 'bg-green-500/20 text-green-400 border-green-500/30',
    yellow: 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30',
    red: 'bg-red-500/20 text-red-400 border-red-500/30',
    blue: 'bg-white/10 text-primary border-border',
  };

  return (
    <div className={`rounded-lg p-4 border ${colorMap[color]}`}>
      <div className="text-2xl font-bold">{value}</div>
      <div className="text-xs opacity-70 mt-1">{label}</div>
    </div>
  );
}
