import { useEffect, useState } from 'react';
import {
  Database,
  HardDrive,
  Layers,
  FileJson,
  Loader2,
  CheckCircle,
  AlertTriangle,
  FolderOpen,
  Github,
  DownloadCloud,
} from 'lucide-react';
import { open, save } from '@tauri-apps/plugin-dialog';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  getDictStats,
  exportCompressedDict,
  exportDictShards,
  type DictStats,
} from '../services/dictOptimize';
import { exportForGithub, exportAiCacheForGithub } from '../services/githubExport';
import {
  downloadEcDict,
  readEcDictInfoSilently,
  type EcDictDownloadInfo,
  type EcDictProgress,
} from '../services/dictDownload';

function formatSize(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${bytes} B`;
}

export default function DictOptimization() {
  const [stats, setStats] = useState<DictStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [exporting, setExporting] = useState(false);
  const [result, setResult] = useState<{ type: 'success' | 'error'; message: string } | null>(null);
  const [maxRank, setMaxRank] = useState(5000);
  const [dlInfo, setDlInfo] = useState<EcDictDownloadInfo | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [dlProgress, setDlProgress] = useState<EcDictProgress | null>(null);

  useEffect(() => {
    loadStats();
    readEcDictInfoSilently().then((d) => setDlInfo(d));
  }, []);

  const loadStats = async () => {
    try {
      setLoading(true);
      const data = await getDictStats();
      setStats(data);
    } catch (error) {
      console.error('加载词典统计失败:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleDownloadDict = async () => {
    if (downloading) return;
    setDownloading(true);
    setDlProgress(null);
    let unlisten: UnlistenFn | null = null;
    try {
      unlisten = await listen<EcDictProgress>('ecdict-download-progress', (e) => {
        setDlProgress(e.payload);
      });
      const path = await downloadEcDict();
      unlisten();
      unlisten = null;
      setDlInfo({
        present: true,
        length: dlProgress?.total ?? 0,
        path,
      });
      showSuccess('词典下载完成，重启后生效');
    } catch (error) {
      showError(`下载失败: ${error}`);
    } finally {
      if (unlisten) unlisten();
      setDownloading(false);
    }
  };

  const showSuccess = (message: string) => {
    setResult({ type: 'success', message });
    setTimeout(() => setResult(null), 5000);
  };

  const showError = (message: string) => {
    setResult({ type: 'error', message });
    setTimeout(() => setResult(null), 8000);
  };

  const handleExportCompressed = async () => {
    setExporting(true);
    try {
      const path = await save({
        defaultPath: `ecdict_compressed_${maxRank}.db`,
        filters: [{ name: 'SQLite', extensions: ['db'] }],
      });
      if (!path) return;

      const result = await exportCompressedDict(path, maxRank);
      showSuccess(`压缩导出完成！共 ${result.exportedWords} 个单词`);
    } catch (error) {
      showError(`导出失败: ${error}`);
    } finally {
      setExporting(false);
    }
  };

  const handleExportShards = async () => {
    setExporting(true);
    try {
      const dir = await open({
        directory: true,
        multiple: false,
      });
      if (!dir) return;

      const manifest = await exportDictShards(dir);
      showSuccess(
        `分片导出完成！${manifest.shards.length} 个分片，共 ${manifest.totalWords} 个单词`,
      );
    } catch (error) {
      showError(`导出失败: ${error}`);
    } finally {
      setExporting(false);
    }
  };

  const handleExportGithub = async () => {
    setExporting(true);
    try {
      const dir = await open({
        directory: true,
        multiple: false,
      });
      if (!dir) return;

      const result = await exportForGithub(dir, 50000);
      showSuccess(
        `GitHub 数据导出完成！${result.totalWords} 个单词，${result.shardsCreated} 个分片`,
      );
    } catch (error) {
      showError(`导出失败: ${error}`);
    } finally {
      setExporting(false);
    }
  };

  const handleExportAiCache = async () => {
    setExporting(true);
    try {
      const dir = await open({
        directory: true,
        multiple: false,
      });
      if (!dir) return;

      const count = await exportAiCacheForGithub(dir, 1000);
      showSuccess(`AI 缓存导出完成！${count} 个单词的 AI 内容`);
    } catch (error) {
      showError(`导出失败: ${error}`);
    } finally {
      setExporting(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <Loader2 className="w-8 h-8 animate-spin text-primary" />
        <span className="ml-3 text-text-secondary">加载词典统计...</span>
      </div>
    );
  }

  if (!stats) {
    return (
      <div className="flex items-center justify-center h-full">
        <AlertTriangle className="w-8 h-8 text-yellow-400" />
        <span className="ml-3">无法加载词典数据</span>
      </div>
    );
  }

  const freqDistribution = [
    { label: '高频词 (≤5000)', count: stats.highFreqWords, color: 'bg-green-500' },
    { label: '中频词 (5001-15000)', count: stats.midFreqWords, color: 'bg-yellow-500' },
    { label: '低频词 (>15000)', count: stats.lowFreqWords, color: 'bg-orange-500' },
    { label: '无频率数据', count: stats.noFreqWords, color: 'bg-gray-500' },
  ];

  const maxCount = Math.max(...freqDistribution.map((d) => d.count), 1);

  return (
    <div className="h-full overflow-y-auto p-8">
      <div className="max-w-3xl mx-auto space-y-8">
        {/* Header */}
        <div>
          <h1 className="ui-page-title flex items-center gap-2.5">
            <Database className="w-5 h-5 shrink-0" />
            词典优化
          </h1>
          <p className="ui-page-desc">压缩和分片词典数据，减小体积、便于备份与迁移</p>
        </div>

        {/* Result Toast */}
        {result && (
          <div
            className={`flex items-center gap-3 p-4 rounded-lg border animate-fadeIn ${
              result.type === 'success'
                ? 'bg-green-500/10 border-green-500/30 text-green-400'
                : 'bg-red-500/10 border-red-500/30 text-red-400'
            }`}
          >
            {result.type === 'success' ? (
              <CheckCircle className="w-5 h-5 flex-shrink-0" />
            ) : (
              <AlertTriangle className="w-5 h-5 flex-shrink-0" />
            )}
            <span className="text-sm">{result.message}</span>
          </div>
        )}

        {/* Dictionary Cloud Download */}
        <div className="bg-bg-secondary rounded-xl border border-border p-6">
          <h2 className="ui-section-title mb-2 flex items-center gap-2">
            <DownloadCloud className="w-4 h-4" />
            词典数据下载（云端）
          </h2>
          <p className="text-sm text-text-secondary mb-4">
            ecdict.db（约 812MB，60 万+ 词条中英词典）因体积过大不随安装包分发，可从
            GitHub Release 云下载到本机；下载完成后重启应用生效。
          </p>

          <div className="flex items-center gap-4">
            <button
              onClick={handleDownloadDict}
              disabled={downloading || (dlInfo?.present ?? false)}
              className="flex items-center gap-2 px-6 py-3 bg-primary hover:bg-primary-hover disabled:bg-bg-tertiary disabled:text-text-secondary rounded-lg transition-colors"
            >
              {downloading ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <DownloadCloud className="w-4 h-4" />
              )}
              {dlInfo?.present ? '词典数据已就绪' : downloading ? '下载中...' : '下载词典数据'}
            </button>
            {dlInfo?.present && (
              <span className="flex items-center gap-1 text-sm text-green-400">
                <CheckCircle className="w-4 h-4" />
                已下载 {formatSize(dlInfo.length)}
              </span>
            )}
          </div>

          {dlProgress && (
            <div className="mt-4 space-y-1">
              <div className="flex justify-between text-xs text-text-secondary">
                <span>
                  {formatSize(dlProgress.received)} /{' '}
                  {dlProgress.total > 0 ? formatSize(dlProgress.total) : '...'}
                </span>
                <span>{Math.round(dlProgress.percent)}%</span>
              </div>
              <div className="h-2 bg-bg-primary rounded-full overflow-hidden">
                <div
                  className="h-full bg-primary rounded-full transition-all duration-300"
                  style={{ width: `${Math.min(dlProgress.percent, 100)}%` }}
                />
              </div>
            </div>
          )}

          {!downloading && dlInfo?.present && dlInfo.path && (
            <div className="mt-3 p-3 bg-bg-primary rounded-lg border border-border">
              <p className="text-xs text-text-secondary break-all">
                已下载至 <code className="px-1 py-0.5 bg-bg-tertiary rounded">{dlInfo.path}</code>
                。重启应用后本地词典生效，词典页可离线查询。
              </p>
            </div>
          )}
        </div>

        {/* Current Stats */}
        <div className="bg-bg-secondary rounded-xl border border-border p-6">
          <h2 className="ui-section-title mb-4 flex items-center gap-2">
            <HardDrive className="w-4 h-4" />
            当前词典状态
          </h2>

          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
            <div className="bg-bg-primary rounded-lg p-4 border border-border">
              <div className="ui-stat">{stats.totalWords.toLocaleString()}</div>
              <div className="text-xs text-text-secondary">总单词数</div>
            </div>
            <div className="bg-bg-primary rounded-lg p-4 border border-border">
              <div className="ui-stat">{stats.totalSizeMb.toFixed(1)} MB</div>
              <div className="text-xs text-text-secondary">数据库大小</div>
            </div>
            <div className="bg-bg-primary rounded-lg p-4 border border-border">
              <div className="ui-stat text-green-400">{stats.highFreqWords.toLocaleString()}</div>
              <div className="text-xs text-text-secondary">高频词</div>
            </div>
            <div className="bg-bg-primary rounded-lg p-4 border border-border">
              <div className="ui-stat text-yellow-400">
                {((stats.highFreqWords / stats.totalWords) * 100).toFixed(1)}%
              </div>
              <div className="text-xs text-text-secondary">高频词占比</div>
            </div>
          </div>

          {/* Frequency Distribution */}
          <div className="space-y-3">
            <h3 className="text-sm font-medium text-text-secondary">词频分布</h3>
            {freqDistribution.map((item) => (
              <div key={item.label} className="flex items-center gap-3">
                <div className="w-32 text-xs text-text-secondary">{item.label}</div>
                <div className="flex-1 h-6 bg-bg-primary rounded-full overflow-hidden">
                  <div
                    className={`h-full ${item.color} rounded-full transition-all duration-500`}
                    style={{ width: `${(item.count / maxCount) * 100}%` }}
                  />
                </div>
                <div className="w-20 text-right text-sm font-mono">
                  {item.count.toLocaleString()}
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Export Compressed */}
        <div className="bg-bg-secondary rounded-xl border border-border p-6">
          <h2 className="ui-section-title mb-2 flex items-center gap-2">
            <FileJson className="w-5 h-5" />
            压缩导出
          </h2>
          <p className="text-sm text-text-secondary mb-4">
            只保留高频词，精简释义字段，大幅减小数据库体积
          </p>

          <div className="flex items-center gap-4 mb-4">
            <label className="text-sm text-text-secondary">保留词频范围：</label>
            <select
              value={maxRank}
              onChange={(e) => setMaxRank(parseInt(e.target.value))}
              className="px-3 py-2 bg-bg-primary rounded-lg border border-border focus:outline-none focus:ring-2 focus:ring-primary"
            >
              <option value={3000}>前 3000 词（核心词汇）</option>
              <option value={5000}>前 5000 词（四级水平）</option>
              <option value={8000}>前 8000 词（六级水平）</option>
              <option value={15000}>前 15000 词（考研水平）</option>
            </select>
          </div>

          <div className="flex items-center justify-between">
            <div className="text-sm text-text-secondary">
              预计保留约{' '}
              {Math.min(
                stats.highFreqWords + (maxRank > 5000 ? stats.midFreqWords : 0),
                stats.totalWords,
              ).toLocaleString()}{' '}
              个单词
            </div>
            <button
              onClick={handleExportCompressed}
              disabled={exporting}
              className="flex items-center gap-2 px-6 py-3 bg-primary hover:bg-primary-hover disabled:bg-bg-tertiary disabled:text-text-secondary rounded-lg transition-colors"
            >
              {exporting ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <FileJson className="w-4 h-4" />
              )}
              导出压缩版
            </button>
          </div>
        </div>

        {/* Export Shards */}
        <div className="bg-bg-secondary rounded-xl border border-border p-6">
          <h2 className="ui-section-title mb-2 flex items-center gap-2">
            <Layers className="w-5 h-5" />
            分片导出
          </h2>
          <p className="text-sm text-text-secondary mb-4">
            按首字母拆分为 26 个分片文件，适合移动端按需下载
          </p>

          <div className="flex items-center justify-between">
            <div className="text-sm text-text-secondary">
              将生成 26 个 .db 文件 + manifest.json 清单
            </div>
            <button
              onClick={handleExportShards}
              disabled={exporting}
              className="flex items-center gap-2 px-6 py-3 bg-primary hover:bg-primary-hover disabled:bg-bg-tertiary disabled:text-text-secondary rounded-lg transition-colors"
            >
              {exporting ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <FolderOpen className="w-4 h-4" />
              )}
              选择目录并导出
            </button>
          </div>
        </div>

        {/* GitHub Export */}
        <div className="bg-bg-secondary rounded-xl border border-border p-6">
          <h2 className="ui-section-title mb-2 flex items-center gap-2">
            <Github className="w-5 h-5" />
            GitHub 数据源导出
          </h2>
          <p className="text-sm text-text-secondary mb-4">
            导出为 GitHub 仓库格式（JSON + GZ 压缩），支持移动端和云端部署
          </p>

          <div className="flex items-center gap-4">
            <button
              onClick={handleExportGithub}
              disabled={exporting}
              className="flex items-center gap-2 px-6 py-3 bg-primary hover:bg-primary-hover disabled:bg-bg-tertiary disabled:text-text-secondary rounded-lg transition-colors"
            >
              {exporting ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Github className="w-4 h-4" />
              )}
              导出词典数据
            </button>

            <button
              onClick={handleExportAiCache}
              disabled={exporting}
              className="flex items-center gap-2 px-6 py-3 bg-bg-primary hover:bg-bg-tertiary border border-border disabled:opacity-50 rounded-lg transition-colors"
            >
              {exporting ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <FileJson className="w-4 h-4" />
              )}
              导出 AI 缓存
            </button>
          </div>

          <div className="mt-4 p-3 bg-bg-primary rounded-lg border border-border">
            <p className="text-xs text-text-secondary">
              💡 导出后将文件上传到 GitHub 仓库{' '}
              <code className="px-1 py-0.5 bg-bg-tertiary rounded">moontranslator-data</code>， 配置
              Cloudflare Workers 即可使用云端词典
            </p>
          </div>
        </div>

        {/* Info */}
        <div className="bg-bg-secondary rounded-xl border border-border p-6">
          <h3 className="font-semibold mb-3">📋 使用说明</h3>
          <div className="space-y-2 text-sm text-text-secondary">
            <p>
              <strong className="text-text-primary">压缩导出：</strong>
              适合移动端打包，只包含常用词汇，体积小（约 20-50MB）
            </p>
            <p>
              <strong className="text-text-primary">分片导出：</strong>
              适合云端部署（GitHub Release），用户可按需下载分片
            </p>
            <p>
              <strong className="text-text-primary">词频范围：</strong>
              高频词（≤5000）覆盖日常交流和考试核心词汇，中频词（5001-15000）覆盖进阶学习
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
