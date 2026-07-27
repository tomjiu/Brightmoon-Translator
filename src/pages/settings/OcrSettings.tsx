import { CheckCircle, Monitor, Sparkles, BookOpen, Cpu, type LucideIcon } from 'lucide-react';
import { useConfigStore } from '../../stores/configStore';
import Card from '../../components/Card';
import Badge from '../../components/Badge';
import {
  OCR_WATCH_INTERVAL_DEFAULT_MS,
  OCR_WATCH_INTERVAL_MIN_MS,
} from '../../services/ocrConstants';

type OcrEngineId = 'auto' | 'winrt' | 'youdao' | 'tesseract' | 'rapid' | 'paddle';

interface OcrEngineOption {
  id: OcrEngineId;
  name: string;
  description: string;
  icon: LucideIcon;
  status: 'available' | 'unavailable';
  recommended?: boolean;
  badges: Array<{ label: string; variant: 'success' | 'warning' | 'error' | 'info' }>;
}

export default function OcrSettings() {
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);

  const ocrEngines: OcrEngineOption[] = [
    {
      id: 'winrt',
      name: 'Windows 原生 OCR',
      description: '系统内置引擎，速度快、免费、无需密钥',
      icon: Monitor,
      status: 'available',
      recommended: true,
      badges: [
        { label: '推荐', variant: 'success' },
        { label: '快速', variant: 'info' },
        { label: '免费', variant: 'success' },
      ],
    },
    {
      id: 'auto',
      name: '自动选择',
      description: '并行尝试多个引擎，兼容性更好但可能更慢',
      icon: Sparkles,
      status: 'available',
      badges: [{ label: '智能', variant: 'info' }],
    },
    {
      id: 'youdao',
      name: '有道 OCR',
      description: '有道免费 OCR 通道，无需单独配置 API Key',
      icon: BookOpen,
      status: 'available',
      badges: [
        { label: '免费', variant: 'success' },
        { label: '无需配置', variant: 'info' },
      ],
    },
    {
      id: 'tesseract',
      name: 'Tesseract',
      description: '本地离线识别，速度较慢但完全本地化',
      icon: Cpu,
      status: 'available',
      badges: [
        { label: '离线', variant: 'info' },
        { label: '慢速', variant: 'warning' },
      ],
    },
    {
      id: 'rapid',
      name: 'RapidOCR（离线）',
      description: '需自备 RapidOcrOnnx + 模型目录（见 docs/OCR_OFFLINE.md）',
      icon: Cpu,
      status: 'available',
      badges: [
        { label: '离线', variant: 'info' },
        { label: '外置模型', variant: 'warning' },
      ],
    },
    {
      id: 'paddle',
      name: 'PaddleOCR-json（离线）',
      description: '需自备 PaddleOCR-json.exe + models（Windows）',
      icon: Cpu,
      status: 'available',
      badges: [
        { label: '离线', variant: 'info' },
        { label: '外置模型', variant: 'warning' },
      ],
    },
  ];

  const currentEngine = (config.ocrEngine || 'winrt') as OcrEngineId;
  const currentEngineInfo = ocrEngines.find((e) => e.id === currentEngine);
  const CurrentIcon = currentEngineInfo?.icon ?? Monitor;

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">OCR 设置</h1>
        <p className="ui-page-desc">截图识别引擎与区域监视参数</p>
      </div>

      <Card>
        <div className="flex items-center gap-3 p-3.5 rounded-xl border border-border bg-bg-tertiary">
          <div className="w-11 h-11 rounded-xl bg-bg-secondary border border-border flex items-center justify-center text-text-primary shrink-0">
            <CurrentIcon size={22} strokeWidth={1.75} />
          </div>
          <div className="flex-1 min-w-0">
            <p className="ui-caption">当前使用</p>
            <p className="ui-section-title mt-0.5">{currentEngineInfo?.name}</p>
          </div>
          <CheckCircle size={20} className="text-text-secondary shrink-0" />
        </div>
        {(currentEngine === 'winrt' || currentEngine === 'tesseract') && (
          <p className="ui-caption mt-3 leading-relaxed">
            {currentEngine === 'winrt' ? 'Windows 原生 OCR' : 'Tesseract'} 开箱即用，无需额外配置。
          </p>
        )}
      </Card>

      <Card title="选择 OCR 引擎" description="用于截图翻译的识别后端">
        <div className="space-y-2">
          {ocrEngines.map((engine) => {
            const Icon = engine.icon;
            const active = currentEngine === engine.id;
            return (
              <label
                key={engine.id}
                className={`flex items-start gap-3 p-3.5 rounded-xl border cursor-pointer transition-colors duration-150 ${
                  active
                    ? 'border-primary bg-primary/5'
                    : engine.status === 'unavailable'
                      ? 'border-border opacity-50 cursor-not-allowed'
                      : 'border-border hover:border-border-strong'
                }`}
              >
                <input
                  type="radio"
                  name="ocrEngine"
                  value={engine.id}
                  checked={active}
                  disabled={engine.status === 'unavailable'}
                  onChange={(e) => {
                    updateConfig((prev) => ({
                      ...prev,
                      ocrEngine: e.target.value as OcrEngineId,
                    }));
                    void saveConfig();
                  }}
                  className="mt-2.5"
                />

                <div className="w-9 h-9 rounded-lg bg-bg-tertiary border border-border flex items-center justify-center shrink-0 text-text-secondary">
                  <Icon size={18} strokeWidth={1.75} />
                </div>

                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-0.5">
                    <span className="text-sm font-medium tracking-tight text-text-primary">
                      {engine.name}
                    </span>
                    {engine.recommended && !active && <Badge variant="success">推荐</Badge>}
                  </div>
                  <p className="ui-caption mb-2 leading-relaxed">{engine.description}</p>
                  <div className="flex flex-wrap gap-1.5">
                    {engine.badges.map((badge, idx) => (
                      <Badge key={idx} variant={badge.variant}>
                        {badge.label}
                      </Badge>
                    ))}
                  </div>
                </div>
              </label>
            );
          })}
        </div>
      </Card>

      {(currentEngine === 'rapid' || currentEngine === 'paddle') && (
        <Card title="离线 OCR 目录" description="不内置模型；指向 pot 插件或 Paddle 解压目录">
          <div className="space-y-3">
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">
                插件/模型目录 pluginDir
              </label>
              <input
                type="text"
                value={config.offlineOcr?.pluginDir || ''}
                onChange={(e) => {
                  updateConfig((prev) => ({
                    ...prev,
                    offlineOcr: {
                      backend: currentEngine === 'paddle' ? 'paddle' : 'rapid',
                      pluginDir: e.target.value,
                    },
                  }));
                }}
                onBlur={() => void saveConfig()}
                placeholder="例如 D:\ocr\rapid 或 pot 插件解压路径"
                className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
              />
              <p className="ui-caption mt-1.5">
                Rapid 需含 RapidOcrOnnx.exe + models；Paddle 需 PaddleOCR-json.exe。详见
                docs/OCR_OFFLINE.md
              </p>
            </div>
          </div>
        </Card>
      )}

      <Card title="OCR 参数" description="区域监视行为">
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">
              区域监视间隔 (毫秒)
            </label>
            <input
              type="number"
              min={OCR_WATCH_INTERVAL_MIN_MS}
              max={10000}
              step={100}
              value={config.ocrInterval ?? OCR_WATCH_INTERVAL_DEFAULT_MS}
              onChange={(e) => {
                const n = parseInt(e.target.value, 10);
                updateConfig((prev) => ({
                  ...prev,
                  ocrInterval: Number.isFinite(n)
                    ? Math.min(10000, Math.max(OCR_WATCH_INTERVAL_MIN_MS, n))
                    : OCR_WATCH_INTERVAL_DEFAULT_MS,
                }));
              }}
              onBlur={() => void saveConfig()}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg"
            />
            <p className="ui-caption mt-1.5 leading-relaxed">
              结果框开启「区域监视」后的检测间隔（默认 {OCR_WATCH_INTERVAL_DEFAULT_MS}
              ms，最小 {OCR_WATCH_INTERVAL_MIN_MS}
              ms）。内容未变时会自动拉长间隔以省电。
            </p>
          </div>
        </div>
      </Card>
    </div>
  );
}
