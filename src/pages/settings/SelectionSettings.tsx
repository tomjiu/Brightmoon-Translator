import { useConfigStore } from '../../stores/configStore';
import type { SelectionTriggerMode, SelectionUxConfig } from '../../types';
import Card from '../../components/Card';
import Switch from '../../components/Switch';
import Badge from '../../components/Badge';

const DEFAULT_UX: SelectionUxConfig = {
  triggerMode: 'pop_button',
  hoverDictionary: false,
  hoverDwellMs: 400,
  ocrForcePickup: false,
  autoMinChars: 1,
  minDragPx: 10,
  excludeProcesses: [],
};

const TRIGGERS: Array<{
  id: SelectionTriggerMode;
  label: string;
  description: string;
  recommended?: boolean;
}> = [
  {
    id: 'pop_button',
    label: '浮钮再译',
    description: '选中后出现「译」按钮，点一下才翻译（推荐；双击选词也支持）',
    recommended: true,
  },
  {
    id: 'auto_on_select',
    label: '选中即显示',
    description: '拖选/双击后松开直接出译文。浏览器/VS Code 优先剪贴板；终端不模拟 Ctrl+C',
  },
  {
    id: 'hotkey_only',
    label: '仅快捷键',
    description: '只有按 Ctrl+Shift+Y（可改）才翻译，鼠标选中不自动触发',
  },
];

export default function SelectionSettings() {
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);

  const ux: SelectionUxConfig = {
    ...DEFAULT_UX,
    ...(config.selectionUx ?? {}),
  };

  const patchUx = (partial: Partial<SelectionUxConfig>) => {
    updateConfig((prev) => ({
      ...prev,
      selectionUx: {
        ...DEFAULT_UX,
        ...(prev.selectionUx ?? {}),
        ...partial,
      },
    }));
    void saveConfig();
  };

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">划词翻译</h1>
        <p className="ui-page-desc">
          选中文字 → 浮钮/自动翻译；悬停词典；OCR 补取。默认「浮钮再译」
        </p>
      </div>

      <Card title="选中文本 → 翻译浮层" description="任意可复制/可 UIA 选中的应用">
        <div className="grid gap-2">
          {TRIGGERS.map((t) => (
            <label
              key={t.id}
              className={`flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-colors ${
                ux.triggerMode === t.id
                  ? 'border-primary bg-primary/5'
                  : 'border-border hover:border-border-strong'
              }`}
            >
              <input
                type="radio"
                name="selectionTrigger"
                value={t.id}
                checked={ux.triggerMode === t.id}
                onChange={() => patchUx({ triggerMode: t.id })}
                className="mt-1"
              />
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium text-text-primary">{t.label}</span>
                  {t.recommended && <Badge variant="info">默认</Badge>}
                </div>
                <p className="text-xs text-text-secondary mt-0.5 leading-relaxed">
                  {t.description}
                </p>
              </div>
            </label>
          ))}
        </div>
        {(ux.triggerMode === 'auto_on_select' || ux.triggerMode === 'pop_button') && (
          <div className="mt-4 space-y-4">
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">
                最少选中字数
              </label>
              <input
                type="number"
                min={1}
                max={50}
                value={ux.autoMinChars}
                onChange={(e) => {
                  const n = parseInt(e.target.value, 10);
                  patchUx({
                    autoMinChars: Number.isFinite(n) ? Math.min(50, Math.max(1, n)) : 1,
                  });
                }}
                className="w-24 px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">
                最小拖拽距离 (像素)
              </label>
              <input
                type="number"
                min={1}
                max={80}
                value={ux.minDragPx ?? 10}
                onChange={(e) => {
                  const n = parseInt(e.target.value, 10);
                  patchUx({
                    minDragPx: Number.isFinite(n) ? Math.min(80, Math.max(1, n)) : 10,
                  });
                }}
                className="w-24 px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
              />
              <p className="ui-caption mt-1.5">
                仅作提示；双击选中单词（0 像素拖拽）仍会取词。无选区的纯单击不会触发
              </p>
            </div>
          </div>
        )}
        <div className="mt-4">
          <label className="block text-sm font-medium text-text-primary mb-2">
            排除进程（逗号分隔，无 .exe）
          </label>
          <input
            type="text"
            value={(ux.excludeProcesses ?? []).join(', ')}
            onChange={(e) => {
              const list = e.target.value
                .split(/[,，\s]+/)
                .map((s) => s.trim())
                .filter(Boolean);
              patchUx({ excludeProcesses: list });
            }}
            placeholder="例如 potplayer, game"
            className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
          />
          <p className="ui-caption mt-1.5">
            这些进程不自动划词/悬停。取词策略：Electron→剪贴板优先；终端→仅 UIA（永不 Ctrl+C）
          </p>
        </div>
        <p className="ui-caption mt-3 leading-relaxed">
          快捷键仍可在「快捷键」页修改。浏览器内选中/悬停由扩展负责，与本页独立。详见
          docs/REFERENCE_SELECTION_UX.md
        </p>
      </Card>

      <Card title="悬停词典" description="与「浮钮再译」可同时开：停留出词，拖选出钮">
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <p className="text-sm font-medium text-text-primary">启用悬停取词</p>
            <p className="text-xs text-text-secondary mt-0.5 leading-relaxed">
              光标稳定后查词（无需 Alt）。终端内自动关闭（避免 PowerShell
              误识）。打字会立刻关卡。OCR 补取用横向窄条。词典未命中不弹空窗。
            </p>
          </div>
          <Switch checked={ux.hoverDictionary} onChange={(v) => patchUx({ hoverDictionary: v })} />
        </div>
        {ux.hoverDictionary && (
          <div className="mt-4">
            <label className="block text-sm font-medium text-text-primary mb-2">
              停留时长 (毫秒)
            </label>
            <input
              type="number"
              min={150}
              max={2000}
              step={50}
              value={ux.hoverDwellMs}
              onChange={(e) => {
                const n = parseInt(e.target.value, 10);
                patchUx({
                  hoverDwellMs: Number.isFinite(n) ? Math.min(2000, Math.max(150, n)) : 400,
                });
              }}
              className="w-28 px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
            />
          </div>
        )}
      </Card>

      <Card title="OCR 强力取词" description="不是「截图翻译」，是划词失败时的补救">
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <p className="text-sm font-medium text-text-primary">选区为空时 OCR 补取光标附近文字</p>
            <p className="text-xs text-text-secondary mt-0.5 leading-relaxed">
              仅在这些情况生效：① 拖选/双击后 UIA+剪贴板都取不到字；②
              按划词热键时没有选区。补取用光标旁横向窄条（约 180×28）WinRT
              OCR，减少标题栏误识。图片/游戏里「选不中」的字才有感；终端默认不做悬停
              OCR。整页/框选仍用工具栏「OCR 截图翻译」。
            </p>
          </div>
          <Switch checked={ux.ocrForcePickup} onChange={(v) => patchUx({ ocrForcePickup: v })} />
        </div>
      </Card>

      <Card title="浮层外观" description="划词结果窗（配置项原有，集中到此）">
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">详细程度</label>
            <select
              value={config.overlayLevel ?? 2}
              onChange={(e) => {
                updateConfig((prev) => ({
                  ...prev,
                  overlayLevel: parseInt(e.target.value, 10) || 2,
                }));
                void saveConfig();
              }}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
            >
              <option value={1}>简洁（译文 + 自动消失）</option>
              <option value={2}>标准（复制 / 关闭）</option>
              <option value={3}>完整（更多控件）</option>
            </select>
          </div>
          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">
              自动关闭 (毫秒，0=不自动)
            </label>
            <input
              type="number"
              min={0}
              max={30000}
              step={500}
              value={config.overlayAutoDismissMs ?? 3000}
              onChange={(e) => {
                const n = parseInt(e.target.value, 10);
                updateConfig((prev) => ({
                  ...prev,
                  overlayAutoDismissMs: Number.isFinite(n) ? Math.max(0, n) : 3000,
                }));
              }}
              onBlur={() => void saveConfig()}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
            />
          </div>
        </div>
      </Card>
    </div>
  );
}
