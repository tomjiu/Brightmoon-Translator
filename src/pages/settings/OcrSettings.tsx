import { useConfigStore } from '../../stores/configStore';
import Card from '../../components/Card';
import Badge from '../../components/Badge';
import { CheckCircle } from 'lucide-react';
import {
  OCR_WATCH_INTERVAL_DEFAULT_MS,
  OCR_WATCH_INTERVAL_MIN_MS,
} from '../../services/ocrConstants';

export default function OcrSettings() {
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);

  const ocrEngines = [
    {
      id: 'winrt',
      name: 'Windows 原生 OCR',
      description: '使用 Windows 内置的 OCR 引擎，快速、准确、免费',
      icon: '🪟',
      status: 'available',
      recommended: true,
      badges: [
        { label: '推荐', variant: 'success' as const },
        { label: '快速', variant: 'info' as const },
        { label: '免费', variant: 'success' as const },
      ],
    },
    {
      id: 'auto',
      name: '自动选择',
      description: '并行尝试多个 OCR 引擎，可能较慢',
      icon: '🤖',
      status: 'available',
      badges: [{ label: '智能', variant: 'info' as const }],
    },
    {
      id: 'youdao',
      name: '有道 OCR',
      description: '有道提供的免费 OCR 服务（已逆向工程，无需 API Key）',
      icon: '📘',
      status: 'available',
      badges: [
        { label: '免费', variant: 'success' as const },
        { label: '无需配置', variant: 'info' as const },
      ],
    },
    {
      id: 'tesseract',
      name: 'Tesseract.js',
      description: '离线 OCR 引擎，速度较慢但完全本地化',
      icon: '📝',
      status: 'available',
      badges: [
        { label: '离线', variant: 'info' as const },
        { label: '慢速', variant: 'warning' as const },
      ],
    },
  ];

  const currentEngine = config.ocrEngine || 'winrt';
  const currentEngineInfo = ocrEngines.find((e) => e.id === currentEngine);

  return (
    <div className="space-y-5">
      {/* 页面标题 */}
      <div>
        <h1 className="text-xl font-semibold text-text-primary">OCR 设置</h1>
        <p className="text-xs text-text-secondary mt-1">配置屏幕截图 OCR 识别引擎</p>
      </div>

      {/* 当前使用的引擎 */}
      <Card>
        <div className="flex items-center gap-3 p-4 bg-primary/5 border border-primary/30 rounded-lg">
          <div className="text-3xl">{currentEngineInfo?.icon}</div>
          <div className="flex-1">
            <p className="text-sm text-text-secondary">当前使用</p>
            <p className="text-lg font-semibold text-primary">{currentEngineInfo?.name}</p>
          </div>
          <CheckCircle size={24} className="text-primary" />
        </div>
        {(currentEngine === 'winrt' || currentEngine === 'tesseract') && (
          <div className="mt-3 p-3 bg-green-500/10 border border-green-500/30 rounded-lg">
            <p className="text-sm text-green-700 dark:text-green-400">
              ✅ {currentEngine === 'winrt' ? 'Windows 原生 OCR' : 'Tesseract.js'}{' '}
              已开箱即用，无需配置，完全免费！
            </p>
          </div>
        )}
      </Card>

      {/* OCR 引擎选择 */}
      <Card title="选择 OCR 引擎" description="选择一个 OCR 引擎用于屏幕截图翻译">
        <div className="space-y-3">
          {ocrEngines.map((engine) => (
            <label
              key={engine.id}
              className={`flex items-start gap-4 p-4 rounded-lg border-2 cursor-pointer transition-all ${
                currentEngine === engine.id
                  ? 'border-primary bg-primary/5'
                  : engine.status === 'unavailable'
                    ? 'border-border opacity-50 cursor-not-allowed'
                    : 'border-border hover:border-border/60'
              }`}
            >
              <input
                type="radio"
                name="ocrEngine"
                value={engine.id}
                checked={currentEngine === engine.id}
                disabled={engine.status === 'unavailable'}
                onChange={(e) => {
                  updateConfig((prev) => ({
                    ...prev,
                    ocrEngine: e.target.value as 'auto' | 'winrt' | 'youdao' | 'tesseract',
                  }));
                  void saveConfig();
                }}
                className="mt-1"
              />

              <div className="text-3xl shrink-0">{engine.icon}</div>

              <div className="flex-1">
                <div className="flex items-center gap-2 mb-1">
                  <span className="font-medium text-text-primary">{engine.name}</span>
                  {engine.recommended && currentEngine !== engine.id && (
                    <Badge variant="success">推荐</Badge>
                  )}
                </div>
                <p className="text-sm text-text-secondary mb-2">{engine.description}</p>
                <div className="flex flex-wrap gap-2">
                  {engine.badges.map((badge, idx) => (
                    <Badge key={idx} variant={badge.variant}>
                      {badge.label}
                    </Badge>
                  ))}
                </div>
              </div>
            </label>
          ))}
        </div>
      </Card>

      {/* OCR 参数配置 */}
      <Card title="OCR 参数" description="配置 OCR 行为参数">
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
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none"
            />
            <p className="text-xs text-text-secondary mt-1">
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
