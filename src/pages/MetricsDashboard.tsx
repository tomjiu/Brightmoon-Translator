import { useState, useEffect, useCallback, useMemo } from 'react';
import { safeInvoke } from '../services/invoke';
import { useI18n } from '../i18n';
import { isTauriRuntime } from '../services/tauriRuntime';
import {
  BarChart3,
  RefreshCw,
  Download,
  Trash2,
  Zap,
  AlertTriangle,
  HardDrive,
  TrendingUp,
  Clock,
  History,
} from 'lucide-react';
import PageLayout from '../components/PageLayout';
import Icon from '../components/Icon';

interface EngineStats {
  count: number;
  avg_ms: number;
  min_ms: number;
  max_ms: number;
  p50_ms: number;
  p95_ms: number;
  p99_ms: number;
  failures: number;
}

interface CacheStats {
  hits: number;
  misses: number;
  hit_rate: number;
}

interface OcrStats {
  count: number;
  avg_ms: number;
}

interface ChunkStats {
  count: number;
  avg_size: number;
}

interface MetricsSummary {
  engine_stats: Record<string, EngineStats>;
  cache_stats: CacheStats;
  ocr_stats: OcrStats | null;
  chunk_stats: ChunkStats | null;
  total_translations: number;
  total_errors: number;
  error_rate: number;
}

interface MetricsTimeline {
  timestamp: number;
  engine: string;
  latency_ms: number;
  success: boolean;
}

interface HourlyStats {
  hour_timestamp: number;
  engine: string;
  total: number;
  success_count: number;
  avg_latency_ms: number;
}

const ENGINE_COLORS: Record<string, string> = {
  LLM: '#8b5cf6',
  primary: '#3b82f6',
  Google: '#22c55e',
  Baidu: '#ef4444',
  Youdao: '#f59e0b',
  DeepL: '#06b6d4',
  DeepLX: '#0ea5e9',
  Microsoft: '#6366f1',
  Yandex: '#ec4899',
};

function getEngineColor(engine: string): string {
  for (const [key, color] of Object.entries(ENGINE_COLORS)) {
    if (engine.includes(key)) return color;
  }
  return '#6b7280';
}

function formatTime(timestamp: number): string {
  const date = new Date(timestamp);
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

export default function MetricsDashboard() {
  const { t } = useI18n();
  const isTauri = isTauriRuntime();
  const [summary, setSummary] = useState<MetricsSummary | null>(null);
  const [timeline, setTimeline] = useState<MetricsTimeline[]>([]);
  const [hourlyStats, setHourlyStats] = useState<HourlyStats[]>([]);
  const [loading, setLoading] = useState(true);
  const [hours, setHours] = useState(24);

  const loadMetrics = useCallback(async () => {
    if (!isTauri) {
      setLoading(false);
      return;
    }

    setLoading(true);
    const [summaryData] = await safeInvoke<MetricsSummary>('get_metrics_summary');
    const [timelineData] = await safeInvoke<MetricsTimeline[]>('get_metrics_timeline', {
      limit: 200,
    });
    const [hourlyData] = await safeInvoke<HourlyStats[]>('get_metrics_hourly_stats', {
      hours,
    });

    if (summaryData) setSummary(summaryData);
    if (timelineData) setTimeline(timelineData);
    if (hourlyData) setHourlyStats(hourlyData);
    setLoading(false);
  }, [hours, isTauri]);

  useEffect(() => {
    loadMetrics();
  }, [loadMetrics]);

  // Calculate today's translation count from hourly stats
  const todayCount = useMemo(() => {
    const todayStart = new Date();
    todayStart.setHours(0, 0, 0, 0);
    const todayTimestamp = todayStart.getTime();

    return hourlyStats
      .filter((stat) => stat.hour_timestamp >= todayTimestamp)
      .reduce((sum, stat) => sum + stat.total, 0);
  }, [hourlyStats]);

  const handleExportCsv = async () => {
    const [csv] = await safeInvoke<string>('export_metrics_csv');
    if (csv) {
      downloadFile(csv, 'metrics.csv', 'text/csv');
    }
  };

  const handleExportJson = async () => {
    const [data] = await safeInvoke<string>('export_metrics_json');
    if (data) {
      const json = JSON.stringify(data, null, 2);
      downloadFile(json, 'metrics.json', 'application/json');
    }
  };

  const handleClear = async () => {
    // eslint-disable-next-line no-alert
    if (confirm(t('metrics.clearConfirm'))) {
      await safeInvoke('clear_metrics');
      loadMetrics();
    }
  };

  if (loading && !summary) {
    return (
      <div className="flex items-center justify-center h-full">
        <RefreshCw className="animate-spin text-text-secondary" size={24} />
      </div>
    );
  }

  return (
    <PageLayout
      chrome="none"
      title={t('metrics.title')}
      icon={BarChart3}
      actions={
            <div className="flex items-center gap-2">
              <select
                value={hours}
                onChange={(e) => setHours(Number(e.target.value))}
                className="px-3 py-1.5 bg-bg-secondary border border-border rounded-lg text-sm text-text-primary"
              >
                <option value={6}>{t('metrics.last6h')}</option>
                <option value={24}>{t('metrics.last24h')}</option>
                <option value={72}>{t('metrics.last3d')}</option>
                <option value={168}>{t('metrics.last7d')}</option>
              </select>
              <button
                onClick={loadMetrics}
                className="p-2 rounded-lg hover:bg-bg-tertiary text-text-secondary transition-colors"
                title={t('metrics.refresh')}
              >
                <Icon icon={RefreshCw} size="md" />
              </button>
              <button
                onClick={handleExportCsv}
                className="p-2 rounded-lg hover:bg-bg-tertiary text-text-secondary transition-colors"
                title="CSV"
              >
                <Icon icon={Download} size="md" />
              </button>
              <button
                onClick={handleExportJson}
                className="p-2 rounded-lg hover:bg-bg-tertiary text-text-secondary transition-colors"
                title="JSON"
              >
                <Icon icon={Download} size="md" />
              </button>
              <button
                onClick={handleClear}
                className="p-2 rounded-lg hover:bg-error/10 text-error transition-colors"
                title={t('metrics.clear')}
              >
                <Icon icon={Trash2} size="md" />
              </button>
            </div>
          }
        >

        {/* Overview Cards */}
        {summary && (
          <div className="grid grid-cols-2 md:grid-cols-5 xl:grid-cols-7 gap-3 md:gap-4">
            <StatCard
              icon={<TrendingUp size={18} />}
              label={t('metrics.totalTranslations')}
              value={summary.total_translations.toLocaleString()}
              color="text-primary"
            />
            <StatCard
              icon={<Clock size={18} />}
              label={t('metrics.todayTranslations') || 'Today'}
              value={todayCount.toLocaleString()}
              color="text-accent"
            />
            <StatCard
              icon={<AlertTriangle size={18} />}
              label={t('metrics.errorRate')}
              value={`${(summary.error_rate * 100).toFixed(1)}%`}
              color={summary.error_rate > 0.05 ? 'text-error' : 'text-success'}
            />
            <StatCard
              icon={<HardDrive size={18} />}
              label={t('metrics.cacheHitRate')}
              value={`${(summary.cache_stats.hit_rate * 100).toFixed(1)}%`}
              color="text-accent"
            />
            <StatCard
              icon={<Zap size={18} />}
              label={t('metrics.totalErrors')}
              value={summary.total_errors.toLocaleString()}
              color="text-warning"
            />
          </div>
        )}

        {/* Cache Stats Detail */}
        {summary && (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="ui-card">
              <h3 className="ui-section-title mb-3">
                {t('metrics.cacheDetails')}
              </h3>
              <div className="flex items-center gap-4">
                <div className="flex-1">
                  <div className="flex justify-between text-sm mb-1">
                    <span className="text-success">
                      {t('metrics.hits')}: {summary.cache_stats.hits}
                    </span>
                    <span className="text-error">
                      {t('metrics.misses')}: {summary.cache_stats.misses}
                    </span>
                  </div>
                  <div className="h-3 bg-bg-tertiary rounded-full overflow-hidden">
                    <div
                      className="h-full bg-success rounded-full transition-all"
                      style={{
                        width: `${(summary.cache_stats.hit_rate * 100).toFixed(0)}%`,
                      }}
                    />
                  </div>
                </div>
                <div className="ui-stat text-text-primary">
                  {(summary.cache_stats.hit_rate * 100).toFixed(1)}%
                </div>
              </div>
            </div>

            {/* OCR Stats */}
            {summary.ocr_stats && (
              <div className="ui-card">
                <h3 className="ui-section-title mb-3">
                  {t('metrics.ocrStats')}
                </h3>
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <div className="ui-stat text-text-primary">{summary.ocr_stats.count}</div>
                    <div className="text-xs text-text-secondary">{t('metrics.ocrCount')}</div>
                  </div>
                  <div>
                    <div className="ui-stat text-text-primary">{summary.ocr_stats.avg_ms}ms</div>
                    <div className="text-xs text-text-secondary">{t('metrics.avgLatency')}</div>
                  </div>
                </div>
              </div>
            )}
          </div>
        )}

        {/* Engine Stats Table */}
        {summary && Object.keys(summary.engine_stats).length > 0 && (
          <div className="ui-card">
            <h3 className="ui-section-title mb-3">
              {t('metrics.engineStats')}
            </h3>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border">
                    <th className="text-left py-2 px-3 text-text-secondary font-medium">
                      {t('metrics.engine')}
                    </th>
                    <th className="text-right py-2 px-3 text-text-secondary font-medium">
                      {t('metrics.count')}
                    </th>
                    <th className="text-right py-2 px-3 text-text-secondary font-medium">
                      {t('metrics.avgLatency')}
                    </th>
                    <th className="text-right py-2 px-3 text-text-secondary font-medium">P50</th>
                    <th className="text-right py-2 px-3 text-text-secondary font-medium">P95</th>
                    <th className="text-right py-2 px-3 text-text-secondary font-medium">P99</th>
                    <th className="text-right py-2 px-3 text-text-secondary font-medium">
                      {t('metrics.min')}
                    </th>
                    <th className="text-right py-2 px-3 text-text-secondary font-medium">
                      {t('metrics.max')}
                    </th>
                    <th className="text-right py-2 px-3 text-text-secondary font-medium">
                      {t('metrics.failures')}
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {Object.entries(summary.engine_stats).map(([name, stats]) => (
                    <tr key={name} className="border-b border-border/50 hover:bg-bg-tertiary/50">
                      <td className="py-2 px-3">
                        <div className="flex items-center gap-2">
                          <div
                            className="w-2.5 h-2.5 rounded-full"
                            style={{ backgroundColor: getEngineColor(name) }}
                          />
                          <span className="text-text-primary font-medium">{name}</span>
                        </div>
                      </td>
                      <td className="text-right py-2 px-3 text-text-primary">{stats.count}</td>
                      <td className="text-right py-2 px-3 text-text-primary">{stats.avg_ms}ms</td>
                      <td className="text-right py-2 px-3 text-text-primary">{stats.p50_ms}ms</td>
                      <td className="text-right py-2 px-3 text-text-primary">{stats.p95_ms}ms</td>
                      <td className="text-right py-2 px-3 text-text-primary">{stats.p99_ms}ms</td>
                      <td className="text-right py-2 px-3 text-success">{stats.min_ms}ms</td>
                      <td className="text-right py-2 px-3 text-warning">{stats.max_ms}ms</td>
                      <td className="text-right py-2 px-3">
                        {stats.failures > 0 ? (
                          <span className="text-error">{stats.failures}</span>
                        ) : (
                          <span className="text-text-secondary">0</span>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}

        {/* Latency Chart (Bar visualization using hourly data) */}
        {hourlyStats.length > 0 && (
          <div className="ui-card">
            <h3 className="ui-section-title mb-3">
              {t('metrics.hourlyLatency')}
            </h3>
            <HourlyLatencyChart data={hourlyStats} />
          </div>
        )}

        {/* Engine Usage Pie Chart */}
        {summary && Object.keys(summary.engine_stats).length > 0 && (
          <div className="ui-card">
            <h3 className="ui-section-title mb-3">
              {t('metrics.engineUsage')}
            </h3>
            <EngineUsageChart engineStats={summary.engine_stats} />
          </div>
        )}

        {/* Recent Translation History List */}
        {timeline.length > 0 && (
          <div className="ui-card">
            <div className="flex items-center gap-2 mb-3">
              <History size={16} className="text-text-secondary" />
              <h3 className="ui-section-title">
                {t('metrics.recentHistory') || 'Recent Translations'}
              </h3>
            </div>
            <TranslationHistoryList data={timeline} />
          </div>
        )}

        {/* Empty State */}
        {summary && summary.total_translations === 0 && (
          <div className="text-center py-12 text-text-secondary">
            <BarChart3 size={48} className="mx-auto mb-4 opacity-30" />
            <p className="ui-section-title">{t('metrics.noData')}</p>
            <p className="ui-caption mt-1">{t('metrics.noDataHint')}</p>
          </div>
        )}
    </PageLayout>
  );
}

function StatCard({
  icon,
  label,
  value,
  color,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  color: string;
}) {
  return (
    <div className="ui-card">
      <div className="flex items-center gap-2 mb-2">
        <div className={color}>{icon}</div>
        <span className="text-xs text-text-secondary">{label}</span>
      </div>
      <div className="ui-stat">{value}</div>
    </div>
  );
}

function HourlyLatencyChart({ data }: { data: HourlyStats[] }) {
  const { t } = useI18n();
  // Group by hour, aggregate engines
  const hourMap = new Map<number, { total: number; success: number; avgLatency: number }>();
  for (const d of data) {
    const existing = hourMap.get(d.hour_timestamp);
    if (existing) {
      existing.total += d.total;
      existing.success += d.success_count;
      existing.avgLatency = (existing.avgLatency + d.avg_latency_ms) / 2;
    } else {
      hourMap.set(d.hour_timestamp, {
        total: d.total,
        success: d.success_count,
        avgLatency: d.avg_latency_ms,
      });
    }
  }

  const sorted = Array.from(hourMap.entries()).sort((a, b) => a[0] - b[0]);
  if (sorted.length === 0) return null;

  const maxLatency = Math.max(...sorted.map(([, v]) => v.avgLatency), 1);

  return (
    <div className="flex items-end gap-1 h-40">
      {sorted.map(([ts, stats]) => {
        const height = (stats.avgLatency / maxLatency) * 100;
        const date = new Date(ts);
        const label = `${date.getHours()}:00`;
        const errorRate = stats.total > 0 ? (stats.total - stats.success) / stats.total : 0;
        const color = errorRate > 0.1 ? 'bg-error' : errorRate > 0.05 ? 'bg-warning' : 'bg-primary';

        return (
          <div key={ts} className="flex-1 flex flex-col items-center gap-1 min-w-0">
            <div className="w-full flex flex-col items-center" style={{ height: '128px' }}>
              <div className="flex-1 w-full flex items-end">
                <div
                  className={`w-full ${color} rounded-t transition-all opacity-80 hover:opacity-100`}
                  style={{ height: `${Math.max(height, 2)}%` }}
                  title={`${label}\n${t('metrics.avgLatency')}: ${stats.avgLatency.toFixed(0)}ms\n${t('metrics.count')}: ${stats.total}`}
                />
              </div>
            </div>
            <span className="text-[10px] text-text-secondary truncate w-full text-center">
              {label}
            </span>
          </div>
        );
      })}
    </div>
  );
}

function EngineUsageChart({ engineStats }: { engineStats: Record<string, EngineStats> }) {
  const entries = Object.entries(engineStats).sort((a, b) => b[1].count - a[1].count);
  const total = entries.reduce((sum, [, s]) => sum + s.count, 0);
  if (total === 0) return null;

  return (
    <div className="space-y-3">
      {entries.map(([name, stats]) => {
        const pct = (stats.count / total) * 100;
        return (
          <div key={name} className="flex items-center gap-3">
            <div className="w-24 text-sm text-text-primary font-medium truncate">{name}</div>
            <div className="flex-1">
              <div className="h-5 bg-bg-tertiary rounded-full overflow-hidden">
                <div
                  className="h-full rounded-full transition-all"
                  style={{
                    width: `${pct}%`,
                    backgroundColor: getEngineColor(name),
                  }}
                />
              </div>
            </div>
            <div className="w-20 text-right text-sm text-text-secondary">
              {stats.count} ({pct.toFixed(1)}%)
            </div>
          </div>
        );
      })}
    </div>
  );
}

function TranslationHistoryList({ data }: { data: MetricsTimeline[] }) {
  const { t } = useI18n();
  // Show most recent first, limit to 50 items
  const reversed = [...data].reverse().slice(0, 50);

  if (reversed.length === 0) {
    return (
      <div className="text-center py-8 text-text-secondary">
        <History size={32} className="mx-auto mb-2 opacity-30" />
        <p className="text-sm">{t('metrics.noHistory') || 'No translation history'}</p>
      </div>
    );
  }

  return (
    <div className="max-h-80 overflow-y-auto">
      <table className="w-full text-sm">
        <thead className="sticky top-0 bg-bg-secondary">
          <tr className="border-b border-border">
            <th className="text-left py-2 px-3 text-text-secondary font-medium">
              {t('metrics.time') || 'Time'}
            </th>
            <th className="text-left py-2 px-3 text-text-secondary font-medium">
              {t('metrics.engine')}
            </th>
            <th className="text-right py-2 px-3 text-text-secondary font-medium">
              {t('metrics.latency') || 'Latency'}
            </th>
            <th className="text-center py-2 px-3 text-text-secondary font-medium">
              {t('metrics.status') || 'Status'}
            </th>
          </tr>
        </thead>
        <tbody>
          {reversed.map((entry, index) => (
            <tr
              key={`${entry.timestamp}-${index}`}
              className="border-b border-border/30 hover:bg-bg-tertiary/50 transition-colors"
            >
              <td className="py-2 px-3 text-text-secondary">{formatTime(entry.timestamp)}</td>
              <td className="py-2 px-3">
                <div className="flex items-center gap-2">
                  <div
                    className="w-2 h-2 rounded-full"
                    style={{ backgroundColor: getEngineColor(entry.engine) }}
                  />
                  <span className="text-text-primary">{entry.engine}</span>
                </div>
              </td>
              <td className="text-right py-2 px-3 text-text-primary">{entry.latency_ms}ms</td>
              <td className="text-center py-2 px-3">
                {entry.success ? (
                  <span className="inline-flex items-center px-2 py-0.5 rounded-full text-xs bg-success/20 text-success">
                    OK
                  </span>
                ) : (
                  <span className="inline-flex items-center px-2 py-0.5 rounded-full text-xs bg-error/20 text-error">
                    FAIL
                  </span>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function downloadFile(content: string, filename: string, mimeType: string) {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
