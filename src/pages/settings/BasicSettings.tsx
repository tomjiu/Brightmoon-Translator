import { useConfigStore } from '../../stores/configStore';
import { useTranslateStore } from '../../stores/translateStore';
import Card from '../../components/Card';
import { LANGUAGES } from '../../types';

export default function BasicSettings() {
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);
  const syncClipboardMonitorFromConfig = useTranslateStore((s) => s.syncClipboardMonitorFromConfig);

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">基础设置</h1>
        <p className="ui-page-desc">默认语言与翻译行为</p>
      </div>

      <Card title="默认语言" description="设置翻译的默认源语言和目标语言">
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">源语言</label>
            <select
              value={config.defaultFrom}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, defaultFrom: e.target.value }));
                void saveConfig();
              }}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none"
            >
              <option value="auto">自动检测</option>
              {LANGUAGES.map((lang) => (
                <option key={lang.code} value={lang.code}>
                  {lang.name}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">目标语言</label>
            <select
              value={config.defaultTo}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, defaultTo: e.target.value }));
                void saveConfig();
              }}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none"
            >
              {LANGUAGES.map((lang) => (
                <option key={lang.code} value={lang.code}>
                  {lang.name}
                </option>
              ))}
            </select>
          </div>
        </div>
      </Card>

      <Card title="翻译选项" description="配置翻译行为">
        <div className="space-y-3">
          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={config.clipboardMonitor || false}
              onChange={(e) => {
                const enabled = e.target.checked;
                updateConfig((prev) => ({ ...prev, clipboardMonitor: enabled }));
                void saveConfig();
                // Wire preference to real event-driven backend listener
                void syncClipboardMonitorFromConfig(enabled);
              }}
              className="rounded"
            />
            <div>
              <p className="text-sm font-medium text-text-primary">剪贴板监听</p>
              <p className="text-xs text-text-secondary">
                监听剪贴板变化并自动翻译（Windows 事件驱动；与主界面剪贴板按钮同步）
              </p>
            </div>
          </label>

          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={config.autoCopyResult || false}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, autoCopyResult: e.target.checked }));
                void saveConfig();
              }}
              className="rounded"
            />
            <div>
              <p className="text-sm font-medium text-text-primary">自动复制结果</p>
              <p className="text-xs text-text-secondary">翻译完成后自动复制到剪贴板</p>
            </div>
          </label>

          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={config.useClipboardOutput}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, useClipboardOutput: e.target.checked }));
                void saveConfig();
              }}
              className="rounded"
            />
            <div>
              <p className="text-sm font-medium text-text-primary">替换用剪贴板粘贴</p>
              <p className="text-xs text-text-secondary">
                开启：Ctrl+V 粘贴（默认，兼容性好）。关闭：Unicode 模拟键入（不改剪贴板）
              </p>
            </div>
          </label>

          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={config.ttsAutoPlay || false}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, ttsAutoPlay: e.target.checked }));
                void saveConfig();
              }}
              className="rounded"
            />
            <div>
              <p className="text-sm font-medium text-text-primary">翻译后自动朗读</p>
              <p className="text-xs text-text-secondary">结果出来后用下方 TTS 后端播放</p>
            </div>
          </label>
        </div>
      </Card>

      <Card
        title="语音合成 (TTS)"
        description="Edge 默认免 Key；Fish s2.1-pro-free 需 Key 但模型免费；OpenAI 兼容要 Key；有道适合单词"
      >
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">后端</label>
            <select
              value={config.ttsProvider || 'edge'}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, ttsProvider: e.target.value }));
                void saveConfig();
              }}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg outline-none"
            >
              <option value="edge">Edge TTS（默认）</option>
              <option value="fish">Fish Audio（s2.1-pro-free）</option>
              <option value="openai">OpenAI / 兼容 TTS</option>
              <option value="youdao">有道 dictvoice</option>
            </select>
          </div>

          {(config.ttsProvider || 'edge') === 'edge' && (
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">
                音色（空=按语言）
              </label>
              <input
                value={config.ttsVoice || ''}
                onChange={(e) => {
                  updateConfig((prev) => ({ ...prev, ttsVoice: e.target.value }));
                  void saveConfig();
                }}
                placeholder="zh-CN-XiaoxiaoNeural"
                className="w-full px-3 py-2 bg-bg-tertiary border border-border rounded-lg text-sm"
              />
            </div>
          )}

          {(config.ttsProvider || 'edge') === 'fish' && (
            <div className="space-y-3">
              <p className="text-xs text-text-secondary">
                模型 s2.1-pro-free 按 Fish 文档为免费开发档（公平使用、无 SLA）。仍需 API
                Key；也可设环境变量 FISH_API_KEY。
              </p>
              <div>
                <label className="block text-sm font-medium text-text-primary mb-2">API Key</label>
                <input
                  type="password"
                  value={config.fishTts?.apiKey || ''}
                  onChange={(e) => {
                    updateConfig((prev) => ({
                      ...prev,
                      fishTts: {
                        apiKey: e.target.value,
                        model: prev.fishTts?.model || 's2.1-pro-free',
                        referenceId:
                          prev.fishTts?.referenceId || '12b8a0bf8e0042c3b11e519d11db8b68',
                        format: prev.fishTts?.format || 'mp3',
                        speed: prev.fishTts?.speed ?? 1,
                      },
                    }));
                    void saveConfig();
                  }}
                  className="w-full px-3 py-2 bg-bg-tertiary border border-border rounded-lg text-sm"
                  placeholder="Fish API Key"
                />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="block text-xs text-text-secondary mb-1">Model</label>
                  <select
                    value={config.fishTts?.model || 's2.1-pro-free'}
                    onChange={(e) => {
                      updateConfig((prev) => ({
                        ...prev,
                        fishTts: {
                          apiKey: prev.fishTts?.apiKey || '',
                          model: e.target.value,
                          referenceId:
                            prev.fishTts?.referenceId || '12b8a0bf8e0042c3b11e519d11db8b68',
                          format: prev.fishTts?.format || 'mp3',
                          speed: prev.fishTts?.speed ?? 1,
                        },
                      }));
                      void saveConfig();
                    }}
                    className="w-full px-2 py-1.5 bg-bg-tertiary border border-border rounded text-sm"
                  >
                    <option value="s2.1-pro-free">s2.1-pro-free（免费）</option>
                    <option value="s2.1-pro">s2.1-pro</option>
                    <option value="s2-pro">s2-pro</option>
                    <option value="s1">s1</option>
                  </select>
                </div>
                <div>
                  <label className="block text-xs text-text-secondary mb-1">语速</label>
                  <input
                    type="number"
                    min={0.5}
                    max={2}
                    step={0.1}
                    value={config.fishTts?.speed ?? 1}
                    onChange={(e) => {
                      const speed = Number(e.target.value) || 1;
                      updateConfig((prev) => ({
                        ...prev,
                        fishTts: {
                          apiKey: prev.fishTts?.apiKey || '',
                          model: prev.fishTts?.model || 's2.1-pro-free',
                          referenceId:
                            prev.fishTts?.referenceId || '12b8a0bf8e0042c3b11e519d11db8b68',
                          format: prev.fishTts?.format || 'mp3',
                          speed,
                        },
                      }));
                      void saveConfig();
                    }}
                    className="w-full px-2 py-1.5 bg-bg-tertiary border border-border rounded text-sm"
                  />
                </div>
              </div>
              <div>
                <label className="block text-sm font-medium text-text-primary mb-2">
                  音色 reference_id（Fish 声音库 / 克隆模型 ID）
                </label>
                <input
                  value={config.fishTts?.referenceId || ''}
                  onChange={(e) => {
                    updateConfig((prev) => ({
                      ...prev,
                      fishTts: {
                        apiKey: prev.fishTts?.apiKey || '',
                        model: prev.fishTts?.model || 's2.1-pro-free',
                        referenceId: e.target.value,
                        format: prev.fishTts?.format || 'mp3',
                        speed: prev.fishTts?.speed ?? 1,
                      },
                      ttsVoice: e.target.value,
                    }));
                    void saveConfig();
                  }}
                  className="w-full px-3 py-2 bg-bg-tertiary border border-border rounded-lg text-sm font-mono"
                  placeholder="12b8a0bf8e0042c3b11e519d11db8b68"
                />
              </div>
            </div>
          )}

          {(config.ttsProvider || 'edge') === 'openai' && (
            <div className="space-y-3">
              <div>
                <label className="block text-sm font-medium text-text-primary mb-2">API Key</label>
                <input
                  type="password"
                  value={config.openaiTts?.apiKey || ''}
                  onChange={(e) => {
                    updateConfig((prev) => ({
                      ...prev,
                      openaiTts: {
                        apiKey: e.target.value,
                        baseUrl: prev.openaiTts?.baseUrl || 'https://api.openai.com/v1',
                        model: prev.openaiTts?.model || 'tts-1',
                        voice: prev.openaiTts?.voice || 'alloy',
                        speed: prev.openaiTts?.speed ?? 1,
                      },
                    }));
                    void saveConfig();
                  }}
                  className="w-full px-3 py-2 bg-bg-tertiary border border-border rounded-lg text-sm"
                  placeholder="sk-..."
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-text-primary mb-2">Base URL</label>
                <input
                  value={config.openaiTts?.baseUrl || 'https://api.openai.com/v1'}
                  onChange={(e) => {
                    updateConfig((prev) => ({
                      ...prev,
                      openaiTts: {
                        apiKey: prev.openaiTts?.apiKey || '',
                        baseUrl: e.target.value,
                        model: prev.openaiTts?.model || 'tts-1',
                        voice: prev.openaiTts?.voice || 'alloy',
                        speed: prev.openaiTts?.speed ?? 1,
                      },
                    }));
                    void saveConfig();
                  }}
                  className="w-full px-3 py-2 bg-bg-tertiary border border-border rounded-lg text-sm"
                />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="block text-xs text-text-secondary mb-1">Model</label>
                  <input
                    value={config.openaiTts?.model || 'tts-1'}
                    onChange={(e) => {
                      updateConfig((prev) => ({
                        ...prev,
                        openaiTts: {
                          apiKey: prev.openaiTts?.apiKey || '',
                          baseUrl: prev.openaiTts?.baseUrl || 'https://api.openai.com/v1',
                          model: e.target.value,
                          voice: prev.openaiTts?.voice || 'alloy',
                          speed: prev.openaiTts?.speed ?? 1,
                        },
                      }));
                      void saveConfig();
                    }}
                    className="w-full px-2 py-1.5 bg-bg-tertiary border border-border rounded text-sm"
                  />
                </div>
                <div>
                  <label className="block text-xs text-text-secondary mb-1">Voice</label>
                  <input
                    value={config.openaiTts?.voice || 'alloy'}
                    onChange={(e) => {
                      updateConfig((prev) => ({
                        ...prev,
                        openaiTts: {
                          apiKey: prev.openaiTts?.apiKey || '',
                          baseUrl: prev.openaiTts?.baseUrl || 'https://api.openai.com/v1',
                          model: prev.openaiTts?.model || 'tts-1',
                          voice: e.target.value,
                          speed: prev.openaiTts?.speed ?? 1,
                        },
                      }));
                      void saveConfig();
                    }}
                    className="w-full px-2 py-1.5 bg-bg-tertiary border border-border rounded text-sm"
                    placeholder="alloy / echo / ..."
                  />
                </div>
              </div>
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}
