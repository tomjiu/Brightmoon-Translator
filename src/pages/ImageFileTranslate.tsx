import { useEffect, useState } from 'react';
import { invokeOrThrow } from '../services/invoke';
import { useI18n } from '../i18n';
import { isTauriRuntime } from '../services/tauriRuntime';
import { takePendingDocPath } from '../services/docHandoff';
import { Image, Languages, Loader2 } from 'lucide-react';
import PageHeader from '../components/PageHeader';

interface ImageTranslationResult {
  outputPath?: string;
  linesTranslated?: number;
  totalLines?: number;
  originalWidth?: number;
  originalHeight?: number;
}

interface ImagePreview {
  lines?: Array<{ text: string; confidence?: number }>;
  width?: number;
  height?: number;
}

function ImageFileTranslate() {
  const [filePath, setFilePath] = useState<string | null>(null);
  const [fileName, setFileName] = useState('');
  const [preview, setPreview] = useState<ImagePreview | null>(null);
  const [resultPath, setResultPath] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [fromLang, setFromLang] = useState('auto');
  const [toLang, setToLang] = useState('zh');
  const { t } = useI18n();
  const isTauri = isTauriRuntime();

  useEffect(() => {
    if (!isTauri) return;
    const path = takePendingDocPath();
    if (!path) return;
    setFilePath(path);
    setFileName(path.split(/[/\\]/).pop() || 'image');
    setPreview(null);
    setResultPath(null);
  }, [isTauri]);

  const openFile = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        filters: [{ name: 'Image', extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp'] }],
      });
      if (!selected) return;
      const path = selected;
      setFilePath(path);
      setFileName(path.split(/[/\\]/).pop() || 'image');
      setPreview(null);
      setResultPath(null);
    } catch (err) {
      console.error(err);
    }
  };

  const runPreview = async () => {
    if (!filePath) return;
    setBusy(true);
    try {
      const res = await invokeOrThrow<ImagePreview>('preview_image_translation', {
        inputPath: filePath,
        lang: fromLang === 'auto' ? 'en' : fromLang,
      });
      setPreview(res);
    } catch (err) {
      console.error(err);
    } finally {
      setBusy(false);
    }
  };

  const runTranslate = async () => {
    if (!filePath) return;
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const defaultName = fileName.replace(/\.[^.]+$/, '_translated.png');
      const outputPath = await save({
        defaultPath: defaultName,
        filters: [{ name: 'PNG', extensions: ['png'] }],
      });
      if (!outputPath) return;
      setBusy(true);
      const res = await invokeOrThrow<ImageTranslationResult>('translate_image', {
        inputPath: filePath,
        outputPath,
        fromLang,
        toLang,
      });
      setResultPath(res.outputPath || outputPath);
    } catch (err) {
      console.error(err);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="h-full flex flex-col p-6">
      <PageHeader
        title={t('documents.image') || '图片文件翻译'}
        icon={Image}
        actions={
          <div className="flex items-center gap-2 flex-wrap">
            <select
              value={fromLang}
              onChange={(e) => setFromLang(e.target.value)}
              className="bg-bg-secondary border border-border rounded-lg px-3 py-2 text-sm"
            >
              <option value="auto">Auto</option>
              <option value="en">English</option>
              <option value="zh">中文</option>
              <option value="ja">日本語</option>
            </select>
            <span>→</span>
            <select
              value={toLang}
              onChange={(e) => setToLang(e.target.value)}
              className="bg-bg-secondary border border-border rounded-lg px-3 py-2 text-sm"
            >
              <option value="zh">中文</option>
              <option value="en">English</option>
              <option value="ja">日本語</option>
            </select>
            <button
              type="button"
              onClick={openFile}
              className="px-3 py-2 rounded-lg border border-border bg-bg-secondary text-sm"
            >
              {t('common.open') || '打开'}
            </button>
            <button
              type="button"
              onClick={runPreview}
              disabled={!filePath || busy}
              className="px-3 py-2 rounded-lg border border-border bg-bg-secondary text-sm disabled:opacity-50"
            >
              OCR 预览
            </button>
            <button
              type="button"
              onClick={runTranslate}
              disabled={!filePath || busy}
              className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-primary text-primary-fg text-sm disabled:opacity-50"
            >
              {busy ? <Loader2 size={16} className="animate-spin" /> : <Languages size={16} />}
              翻译并保存
            </button>
          </div>
        }
      />
      {fileName && <p className="text-sm text-text-secondary mb-2">{fileName}</p>}
      {resultPath && (
        <p className="text-sm text-primary mb-2 truncate" title={resultPath}>
          已写出: {resultPath}
        </p>
      )}
      <div className="flex-1 overflow-auto min-h-0 rounded-lg border border-border bg-bg-secondary p-4">
        {!filePath && (
          <p className="text-sm text-text-secondary">选择图片文件 → OCR 预览或翻译叠字导出 PNG。</p>
        )}
        {preview?.lines?.map((line, i) => (
          <div key={i} className="text-sm py-1 border-b border-border/40">
            {line.text}
          </div>
        ))}
      </div>
    </div>
  );
}

export default ImageFileTranslate;
