import { useState } from 'react';
import {
  Download,
  Upload,
  FileJson,
  FileSpreadsheet,
  Database,
  FolderOpen,
  CheckCircle,
  AlertCircle,
  Loader2,
  FileText,
} from 'lucide-react';
import { open, save } from '@tauri-apps/plugin-dialog';
import {
  exportLearningDataJson,
  exportAnkiTsv,
  ankiNotesToTsv,
  importLearningDataJson,
  importWordlistCsv,
  autoBackupWithCleanup,
  writeFileContent,
  type ImportResult,
} from '../services/dataIO';

export default function DataIO() {
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<{ type: 'success' | 'error'; message: string } | null>(null);

  const showSuccess = (message: string) => {
    setResult({ type: 'success', message });
    setTimeout(() => setResult(null), 5000);
  };

  const showError = (message: string) => {
    setResult({ type: 'error', message });
    setTimeout(() => setResult(null), 8000);
  };

  // ====== 导出功能 ======

  const handleExportJson = async () => {
    setLoading(true);
    try {
      const path = await save({
        defaultPath: `学习数据_${new Date().toISOString().split('T')[0]}.json`,
        filters: [{ name: 'JSON', extensions: ['json'] }],
      });
      if (!path) return;

      const data = await exportLearningDataJson();
      const json = JSON.stringify(data, null, 2);
      await writeFileContent(path, json);

      showSuccess(`成功导出 ${data.totalCards} 张卡牌到 JSON`);
    } catch (error) {
      showError(`导出失败: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  const handleExportAnki = async () => {
    setLoading(true);
    try {
      const path = await save({
        defaultPath: `Anki导入_${new Date().toISOString().split('T')[0]}.txt`,
        filters: [{ name: 'TSV', extensions: ['txt', 'tsv'] }],
      });
      if (!path) return;

      const notes = await exportAnkiTsv();
      const tsv = ankiNotesToTsv(notes);
      await writeFileContent(path, tsv);

      showSuccess(`成功导出 ${notes.length} 条 Anki 笔记`);
    } catch (error) {
      showError(`导出失败: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  const handleExportCsv = async () => {
    setLoading(true);
    try {
      const data = await exportLearningDataJson();
      const header = 'word,stability,difficulty,reps,last_review\n';
      const rows = data.cards
        .map((card) => {
          const fsrs = card.fsrsState as Record<string, unknown>;
          return [
            card.word,
            (fsrs.stability as number).toFixed(2) || '0',
            (fsrs.difficulty as number).toFixed(2) || '0',
            (fsrs.reps as number) || 0,
            card.lastReview ? new Date(card.lastReview * 1000).toISOString().split('T')[0] : '',
          ].join(',');
        })
        .join('\n');

      const csv = header + rows;
      const path = await save({
        defaultPath: `学习数据_${new Date().toISOString().split('T')[0]}.csv`,
        filters: [{ name: 'CSV', extensions: ['csv'] }],
      });
      if (!path) return;

      await writeFileContent(path, csv);

      showSuccess(`成功导出 ${data.totalCards} 条 CSV 记录`);
    } catch (error) {
      showError(`导出失败: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  // ====== 导入功能 ======

  const handleImportJson = async () => {
    setLoading(true);
    try {
      const path = await open({
        filters: [{ name: 'JSON', extensions: ['json'] }],
        multiple: false,
      });
      if (!path) return;

      const result = await importLearningDataJson(path);
      showImportResult(result, 'JSON');
    } catch (error) {
      showError(`导入失败: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  const handleImportCsv = async () => {
    setLoading(true);
    try {
      const path = await open({
        filters: [{ name: 'CSV/TSV', extensions: ['csv', 'tsv', 'txt'] }],
        multiple: false,
      });
      if (!path) return;

      const result = await importWordlistCsv(path);
      showImportResult(result, 'CSV');
    } catch (error) {
      showError(`导入失败: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  const showImportResult = (result: ImportResult, format: string) => {
    const parts = [
      result.imported > 0 ? `新增 ${result.imported} 词` : '没有新词导入',
      result.skipped > 0 ? `跳过 ${result.skipped} 个重复词` : '',
      result.invalid > 0 ? `忽略 ${result.invalid} 个无效行` : '',
    ]
      .filter(Boolean)
      .join('，');
    if (result.imported > 0) {
      showSuccess(`${format} 导入完成！${parts}`);
    } else {
      showError(parts);
    }
  };

  // ====== 自动备份 ======

  const handleAutoBackup = async () => {
    setLoading(true);
    try {
      const path = await open({
        directory: true,
        multiple: false,
      });
      if (!path) return;

      const backupPath = await autoBackupWithCleanup(path, 30);
      showSuccess(`备份成功！文件: ${backupPath}`);
    } catch (error) {
      showError(`备份失败: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="h-full overflow-y-auto">
      <div className="w-full p-4 md:p-5 lg:p-6 space-y-5">
        {/* Header */}
        <div>
          <h1 className="ui-page-title flex items-center gap-2.5">
            <Database className="w-5 h-5 shrink-0" />
            数据管理
          </h1>
          <p className="ui-page-desc">导入导出学习数据，支持 JSON、CSV、Anki 等格式</p>
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
              <AlertCircle className="w-5 h-5 flex-shrink-0" />
            )}
            <span className="text-sm">{result.message}</span>
          </div>
        )}

        {/* Loading Overlay */}
        {loading && (
          <div className="flex items-center justify-center gap-3 p-4 bg-bg-secondary rounded-lg">
            <Loader2 className="w-5 h-5 animate-spin text-primary" />
            <span className="text-text-secondary">处理中...</span>
          </div>
        )}

        {/* Export Section */}
        <div className="ui-card ui-card-hover p-5">
          <h2 className="ui-section-title flex items-center gap-2 mb-4">
            <Download className="w-5 h-5" />
            导出数据
          </h2>

          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <button
              onClick={handleExportJson}
              disabled={loading}
              className="flex flex-col items-center gap-3 p-6 bg-bg-secondary rounded-xl border border-border hover:border-primary/50 hover:scale-[1.02] transition-all group"
            >
              <FileJson className="w-10 h-10 text-primary group-hover:text-primary" />
              <div className="text-center">
                <div className="ui-section-title">JSON 全量导出</div>
                <div className="text-xs text-text-secondary mt-1">
                  完整卡牌 + FSRS 状态 + AI 内容
                </div>
              </div>
            </button>

            <button
              onClick={handleExportAnki}
              disabled={loading}
              className="flex flex-col items-center gap-3 p-6 bg-bg-secondary rounded-xl border border-border hover:border-primary/50 hover:scale-[1.02] transition-all group"
            >
              <FileText className="w-10 h-10 text-green-400 group-hover:text-green-300" />
              <div className="text-center">
                <div className="ui-section-title">Anki 导入格式</div>
                <div className="text-xs text-text-secondary mt-1">TSV 格式，支持 HTML 释义</div>
              </div>
            </button>

            <button
              onClick={handleExportCsv}
              disabled={loading}
              className="flex flex-col items-center gap-3 p-6 bg-bg-secondary rounded-xl border border-border hover:border-primary/50 hover:scale-[1.02] transition-all group"
            >
              <FileSpreadsheet className="w-10 h-10 text-yellow-400 group-hover:text-yellow-300" />
              <div className="text-center">
                <div className="ui-section-title">CSV 通用格式</div>
                <div className="text-xs text-text-secondary mt-1">可在 Excel 中查看和编辑</div>
              </div>
            </button>
          </div>
        </div>

        {/* Import Section */}
        <div className="ui-card ui-card-hover p-5">
          <h2 className="ui-section-title flex items-center gap-2 mb-4">
            <Upload className="w-5 h-5" />
            导入数据
          </h2>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <button
              onClick={handleImportJson}
              disabled={loading}
              className="flex flex-col items-center gap-3 p-6 bg-bg-secondary rounded-xl border border-border hover:border-primary/50 hover:scale-[1.02] transition-all group"
            >
              <FileJson className="w-10 h-10 text-primary group-hover:text-primary" />
              <div className="text-center">
                <div className="ui-section-title">导入 JSON 备份</div>
                <div className="text-xs text-text-secondary mt-1">恢复之前导出的完整数据</div>
              </div>
            </button>

            <button
              onClick={handleImportCsv}
              disabled={loading}
              className="flex flex-col items-center gap-3 p-6 bg-bg-secondary rounded-xl border border-border hover:border-primary/50 hover:scale-[1.02] transition-all group"
            >
              <FileSpreadsheet className="w-10 h-10 text-yellow-400 group-hover:text-yellow-300" />
              <div className="text-center">
                <div className="ui-section-title">导入单词列表</div>
                <div className="text-xs text-text-secondary mt-1">
                  支持 CSV / TSV / Quizlet / 扇贝 导出格式
                </div>
              </div>
            </button>
          </div>
        </div>

        {/* Auto Backup */}
        <div className="ui-card ui-card-hover p-5">
          <h2 className="ui-section-title flex items-center gap-2 mb-4">
            <FolderOpen className="w-5 h-5" />
            自动备份
          </h2>

          <div>
            <div className="flex items-center justify-between">
              <div>
                <h3 className="ui-section-title">手动备份</h3>
                <p className="ui-caption mt-1">
                  选择一个文件夹，导出带时间戳的完整备份文件
                </p>
              </div>
              <button
                onClick={handleAutoBackup}
                disabled={loading}
                className="px-6 py-3 bg-primary hover:bg-primary-hover disabled:bg-bg-tertiary disabled:text-text-secondary rounded-lg transition-colors font-medium"
              >
                选择目录并备份
              </button>
            </div>
          </div>
        </div>

        {/* Format Guide */}
        <div className="ui-card ui-card-hover p-5">
          <h3 className="ui-section-title mb-3">📋 格式说明</h3>
          <div className="space-y-2 text-sm text-text-secondary">
            <p>
              <strong className="text-text-primary">JSON 全量：</strong>
              包含所有卡牌、FSRS 状态、学习历史、每日活动数据，适合完整备份和恢复
            </p>
            <p>
              <strong className="text-text-primary">Anki TSV：</strong>
              标准 Anki 导入格式，包含正反面内容和标签，可直接在 Anki 中 File → Import 使用
            </p>
            <p>
              <strong className="text-text-primary">CSV：</strong>
              通用表格格式，word/stability/difficulty/reps/last_review 五列
            </p>
            <p>
              <strong className="text-text-primary">单词列表导入：</strong>
              支持 CSV 或 TSV 格式，第一列为单词，第二列为释义（可选），兼容 Quizlet/扇贝导出
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
