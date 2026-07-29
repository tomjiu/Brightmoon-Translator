import { useEffect, useState } from 'react';
import { useConfigStore } from '../../stores/configStore';
import type { SelectionTriggerMode, SelectionUxConfig } from '../../types';
import Card from '../../components/Card';
import Switch from '../../components/Switch';
import Badge from '../../components/Badge';
import { safeInvoke } from '../../services/invoke';
import { isTauriRuntime } from '../../services/tauriRuntime';

const DEFAULT_UX: SelectionUxConfig = {
  triggerMode: 'pop_button',
  hoverDictionary: false,
  hoverDwellMs: 400,
  hoverUnit: 'word',
  hoverDictSource: 'auto',
  ocrForcePickup: false,
  ocrModifierKey: '',
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
  const [ecdictLoaded, setEcdictLoaded] = useState<boolean | null>(null);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    void safeInvoke<{ loaded: boolean }>('ecdict_status', undefined, { silent: true }).then(
      ([status]) => {
        if (status) setEcdictLoaded(status.loaded);
      },
    );
  }, []);

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
          选中文字 → 浮钮/自动；悬停=仅词典。单字→词典卡，句/段→翻译卡；junk
          不机翻。默认「浮钮再译」
        </p>
      </div>

      <Card
        title="选中文本 → 词典/翻译浮层"
        description="词→词典卡（ECDICT→有道）；句→机翻；浮钮只确认 show 时 pending"
      >
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
              光标稳定后查词（无需 Alt）。仅词典卡，未命中不弹机翻。终端内自动关闭。打字立刻关卡。
              与浮钮共用 ECDICT→有道。
            </p>
          </div>
          <Switch checked={ux.hoverDictionary} onChange={(v) => patchUx({ hoverDictionary: v })} />
        </div>
        {ux.hoverDictionary && (
          <div className="mt-4 space-y-3">
            <div>
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
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">
                取词单位（MTT 词/句）
              </label>
              <select
                value={ux.hoverUnit || 'word'}
                onChange={(e) => patchUx({ hoverUnit: e.target.value })}
                className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
              >
                <option value="word">词（仅真词典命中）</option>
                <option value="sentence">句（机翻）</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">词典来源</label>
              <select
                value={ux.hoverDictSource || 'auto'}
                onChange={(e) => patchUx({ hoverDictSource: e.target.value })}
                className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
              >
                <option value="auto">自动（本地 ECDICT → 有道）</option>
                <option value="ecdict">仅本地 ECDICT</option>
                <option value="youdao">仅有道在线</option>
              </select>
              {ecdictLoaded === false && (
                <p className="text-xs text-warning mt-1">
                  本地 ECDICT 未加载，选「仅本地」或「自动」时悬停释义可能为空。请将 ecdict.db 放到
                  dictionaries/ 或安装包 resources 目录。
                </p>
              )}
              {ecdictLoaded === true && (
                <p className="text-xs text-text-secondary mt-1">本地 ECDICT 已加载。</p>
              )}
              <p className="text-xs text-text-secondary mt-1">
                悬停是<strong>非侵入</strong> UIA（不注入进程）。只读 TextPattern/编辑框内容，
                <strong>不用</strong>窗口标题 Name（避免 PowerShell/Google）。未命中词典不弹机翻。
                终端默认关闭悬停；浏览器页内文字依赖控件是否暴露 TextPattern。
              </p>
            </div>
          </div>
        )}
      </Card>

      <Card title="OCR 强力取词" description="不是「截图翻译」，是划词失败时的补救">
        <div className="space-y-4">
          <div className="flex items-center justify-between gap-4">
            <div className="min-w-0">
              <p className="text-sm font-medium text-text-primary">
                选区为空时 OCR 补取光标附近文字
              </p>
              <p className="text-xs text-text-secondary mt-0.5 leading-relaxed">
                仅在这些情况生效：① 拖选/双击后 UIA+剪贴板都取不到字；②
                按划词热键时没有选区。补取用光标旁横向窄条（约 180×28）WinRT
                OCR。整页/框选仍用工具栏「OCR 截图翻译」。
              </p>
            </div>
            <Switch checked={ux.ocrForcePickup} onChange={(v) => patchUx({ ocrForcePickup: v })} />
          </div>
          {ux.ocrForcePickup && (
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">
                修饰键（MTT 风格，减少误触）
              </label>
              <select
                value={ux.ocrModifierKey || ''}
                onChange={(e) => patchUx({ ocrModifierKey: e.target.value })}
                className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
              >
                <option value="">无（只要开关开就 OCR）</option>
                <option value="shift">按住 Shift</option>
                <option value="ctrl">按住 Ctrl</option>
                <option value="alt">按住 Alt</option>
              </select>
              <p className="text-xs text-text-secondary mt-1">
                悬停图文 / 空选区补取时需同时按住；推荐 Shift 对齐 Mouse Tooltip Translator。
              </p>
            </div>
          )}
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
