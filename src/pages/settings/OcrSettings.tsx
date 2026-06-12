import { useConfigStore } from '../../stores/configStore';
import Card from '../../components/Card';
import Badge from '../../components/Badge';
import { AlertCircle, CheckCircle } from 'lucide-react';

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
      description: '有道提供的 OCR 服务',
      icon: '📘',
      status: 'unavailable',
      badges: [{ label: '不可用', variant: 'error' as const }],
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
    <div className="space-y-6">
      {/* 页面标题 */}
      <div>
        <h1 className="text-2xl font-bold text-text-primary">OCR 设置</h1>
        <p className="text-sm text-text-secondary mt-1">配置屏幕截图 OCR 识别引擎</p>
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
      </Card>

      {/* Youdao OCR 警告 */}
      <div className="p-4 bg-yellow-500/10 border border-yellow-500/30 rounded-lg">
        <div className="flex items-start gap-3">
          <AlertCircle size={20} className="text-yellow-600 dark:text-yellow-400 shrink-0 mt-0.5" />
          <div className="text-sm">
            <p className="font-medium text-yellow-600 dark:text-yellow-400 mb-1">
              ⚠️ 有道 OCR 当前不可用
            </p>
            <p className="text-text-secondary">
              有道 OCR API 返回 404 错误。推荐使用 <strong>Windows 原生 OCR</strong>
              （快速、准确、免费）
            </p>
          </div>
        </div>
      </div>

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
              刷新间隔 (毫秒)
            </label>
            <input
              type="number"
              min="500"
              max="10000"
              step="100"
              value={config.ocrInterval || 2000}
              onChange={(e) => {
                updateConfig((prev) => ({
                  ...prev,
                  ocrInterval: parseInt(e.target.value, 10),
                }));
              }}
              onBlur={() => void saveConfig()}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none"
            />
            <p className="text-xs text-text-secondary mt-1">
              OCR 区域自动刷新的时间间隔，默认 2000ms
            </p>
          </div>
        </div>
      </Card>
    </div>
  );
}
