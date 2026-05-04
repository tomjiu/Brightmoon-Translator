import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useConfigStore } from "../stores/configStore";
import { useI18n } from "../i18n";
import { Save, Check, Trash2, Database, Power, Clipboard, Eye, EyeOff, Globe, Keyboard, Plus, X, Download, Upload, Languages, Wand2, MousePointer } from "lucide-react";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";

function Settings() {
  const {
    config,
    saved,
    cacheSize,
    loadConfig,
    saveConfig,
    updateConfig,
    updateLlm,
    loadCacheSize,
    clearCache,
  } = useConfigStore();

  const { locale, setLocale, t } = useI18n();
  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [newApiKey, setNewApiKey] = useState("");
  const [newTemplateName, setNewTemplateName] = useState("");
  const [newTemplatePrompt, setNewTemplatePrompt] = useState("");

  // Post-processing state
  interface ReplacementRule {
    id: string;
    pattern: string;
    replacement: string;
    enabled: boolean;
    isRegex: boolean;
  }
  interface PostProcessConfig {
    rules: ReplacementRule[];
    trimWhitespace: boolean;
    fixPunctuation: boolean;
    fixNewlines: boolean;
  }
  const [postConfig, setPostConfig] = useState<PostProcessConfig>({
    rules: [],
    trimWhitespace: true,
    fixPunctuation: true,
    fixNewlines: true,
  });
  const [newRulePattern, setNewRulePattern] = useState("");
  const [newRuleReplacement, setNewRuleReplacement] = useState("");
  const [newRuleIsRegex, setNewRuleIsRegex] = useState(false);
  const [testInput, setTestInput] = useState("");
  const [testOutput, setTestOutput] = useState("");

  useEffect(() => {
    loadConfig();
    loadCacheSize();
    checkAutostart();
    loadPostProcessConfig();
  }, [loadConfig, loadCacheSize]);

  const loadPostProcessConfig = async () => {
    try {
      const config = await invoke<PostProcessConfig>("get_post_process_config");
      setPostConfig(config);
    } catch (err) {
      console.error("Failed to load post-process config:", err);
    }
  };

  const savePostProcessConfig = async (newConfig: PostProcessConfig) => {
    setPostConfig(newConfig);
    try {
      await invoke("update_post_process_config", { config: newConfig });
    } catch (err) {
      console.error("Failed to save post-process config:", err);
    }
  };

  const addReplacementRule = async () => {
    if (!newRulePattern.trim()) return;
    try {
      await invoke("add_replacement_rule", {
        pattern: newRulePattern.trim(),
        replacement: newRuleReplacement,
        isRegex: newRuleIsRegex,
      });
      setNewRulePattern("");
      setNewRuleReplacement("");
      setNewRuleIsRegex(false);
      await loadPostProcessConfig();
    } catch (err) {
      console.error("Failed to add rule:", err);
    }
  };

  const removeReplacementRule = async (id: string) => {
    try {
      await invoke("remove_replacement_rule", { id });
      await loadPostProcessConfig();
    } catch (err) {
      console.error("Failed to remove rule:", err);
    }
  };

  const toggleRuleEnabled = async (rule: ReplacementRule) => {
    try {
      await invoke("update_replacement_rule", {
        id: rule.id,
        pattern: rule.pattern,
        replacement: rule.replacement,
        enabled: !rule.enabled,
        isRegex: rule.isRegex,
      });
      await loadPostProcessConfig();
    } catch (err) {
      console.error("Failed to toggle rule:", err);
    }
  };

  const runPostProcessTest = async () => {
    if (!testInput.trim()) return;
    try {
      const result = await invoke<string>("test_post_process", { text: testInput });
      setTestOutput(result);
    } catch (err) {
      console.error("Failed to test post-process:", err);
    }
  };

  const checkAutostart = async () => {
    try {
      const enabled = await isEnabled();
      setAutostartEnabled(enabled);
    } catch (err) {
      console.error("Failed to check autostart:", err);
    }
  };

  const toggleAutostart = async () => {
    try {
      if (autostartEnabled) {
        await disable();
      } else {
        await enable();
      }
      setAutostartEnabled(!autostartEnabled);
    } catch (err) {
      console.error("Failed to toggle autostart:", err);
    }
  };

  const addPromptTemplate = () => {
    if (!newTemplateName.trim() || !newTemplatePrompt.trim()) return;
    updateConfig((prev) => ({
      ...prev,
      promptTemplates: [
        ...prev.promptTemplates,
        { name: newTemplateName.trim(), prompt: newTemplatePrompt.trim() },
      ],
    }));
    setNewTemplateName("");
    setNewTemplatePrompt("");
  };

  const removePromptTemplate = (index: number) => {
    updateConfig((prev) => ({
      ...prev,
      promptTemplates: prev.promptTemplates.filter((_, i) => i !== index),
    }));
  };

  const applyTemplate = (prompt: string) => {
    updateConfig((prev) => ({
      ...prev,
      customPrompt: prompt,
    }));
  };

  const exportConfig = async () => {
    try {
      const json = await invoke<string>("export_config_json");
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `moontranslator-config_${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error("Failed to export config:", err);
    }
  };

  const importConfig = async () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;
      try {
        const text = await file.text();
        await invoke("import_config_json", { json: text });
        // Reload config after import
        await loadConfig();
        alert("配置导入成功！");
      } catch (err) {
        console.error("Failed to import config:", err);
        alert("配置导入失败：" + err);
      }
    };
    input.click();
  };

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="max-w-xl mx-auto">
        <h1 className="text-2xl font-bold mb-6">{t("settings.title")}</h1>

        {/* Language Selector */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4 flex items-center gap-2">
            <Languages size={18} />
            语言 / Language
          </h2>
          <div className="flex gap-3">
            <button
              className={`px-4 py-2 rounded-lg text-sm border transition-colors ${
                locale === "zh"
                  ? "bg-primary text-white border-primary"
                  : "bg-bg-tertiary text-text-secondary border-border hover:border-primary"
              }`}
              onClick={() => setLocale("zh")}
            >
              中文
            </button>
            <button
              className={`px-4 py-2 rounded-lg text-sm border transition-colors ${
                locale === "en"
                  ? "bg-primary text-white border-primary"
                  : "bg-bg-tertiary text-text-secondary border-border hover:border-primary"
              }`}
              onClick={() => setLocale("en")}
            >
              English
            </button>
          </div>
        </section>

        {/* LLM Section */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4">
            LLM 翻译引擎
          </h2>

          <div className="space-y-4">
            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                服务商
              </label>
              <select
                value={config.llm.provider}
                onChange={(e) => {
                  const provider = e.target.value as
                    | "openai"
                    | "deepseek"
                    | "custom";
                  const baseUrls: Record<string, string> = {
                    openai: "https://api.openai.com/v1",
                    deepseek: "https://api.deepseek.com/v1",
                    custom: config.llm.baseUrl,
                  };
                  updateConfig((prev) => ({
                    ...prev,
                    llm: {
                      ...prev.llm,
                      provider,
                      baseUrl: baseUrls[provider] || prev.llm.baseUrl,
                    },
                  }));
                }}
                className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary"
              >
                <option value="deepseek">DeepSeek</option>
                <option value="openai">OpenAI</option>
                <option value="custom">自定义</option>
              </select>
            </div>

            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                API Key (主密钥)
              </label>
              <input
                type="password"
                value={config.llm.apiKey}
                onChange={(e) => updateLlm("apiKey", e.target.value)}
                placeholder="sk-..."
                className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
              />
            </div>

            {/* Additional API Keys */}
            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                备用 API Keys (轮询/故障转移)
              </label>
              <p className="text-xs text-text-secondary mb-2">
                多个密钥自动轮询，单个失败时自动切换下一个
              </p>
              <div className="space-y-2">
                {config.llm.apiKeys.map((key, index) => (
                  <div key={index} className="flex gap-2">
                    <input
                      type="password"
                      value={key}
                      onChange={(e) => {
                        const newKeys = [...config.llm.apiKeys];
                        newKeys[index] = e.target.value;
                        updateConfig((prev) => ({
                          ...prev,
                          llm: { ...prev.llm, apiKeys: newKeys },
                        }));
                      }}
                      placeholder="备用 API Key"
                      className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
                    />
                    <button
                      onClick={() => {
                        const newKeys = config.llm.apiKeys.filter((_, i) => i !== index);
                        updateConfig((prev) => ({
                          ...prev,
                          llm: { ...prev.llm, apiKeys: newKeys },
                        }));
                      }}
                      className="bg-bg-tertiary text-error border border-border rounded-lg px-3 py-2 hover:bg-error hover:text-white hover:border-error transition-colors"
                    >
                      <X size={14} />
                    </button>
                  </div>
                ))}
                <div className="flex gap-2">
                  <input
                    type="password"
                    value={newApiKey}
                    onChange={(e) => setNewApiKey(e.target.value)}
                    placeholder="输入新的备用 API Key"
                    className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && newApiKey.trim()) {
                        updateConfig((prev) => ({
                          ...prev,
                          llm: { ...prev.llm, apiKeys: [...prev.llm.apiKeys, newApiKey.trim()] },
                        }));
                        setNewApiKey("");
                      }
                    }}
                  />
                  <button
                    onClick={() => {
                      if (newApiKey.trim()) {
                        updateConfig((prev) => ({
                          ...prev,
                          llm: { ...prev.llm, apiKeys: [...prev.llm.apiKeys, newApiKey.trim()] },
                        }));
                        setNewApiKey("");
                      }
                    }}
                    className="bg-primary text-white rounded-lg px-3 py-2 hover:bg-primary-hover transition-colors flex items-center gap-1"
                  >
                    <Plus size={14} />
                    添加
                  </button>
                </div>
              </div>
              {config.llm.apiKeys.length > 0 && (
                <p className="text-xs text-primary mt-2">
                  已配置 {config.llm.apiKeys.length + 1} 个 API Key (含主密钥)
                </p>
              )}
            </div>

            {config.llm.provider === "custom" && (
              <div>
                <label className="block text-xs text-text-secondary mb-1.5">
                  Base URL
                </label>
                <input
                  value={config.llm.baseUrl}
                  onChange={(e) => updateLlm("baseUrl", e.target.value)}
                  placeholder="https://api.example.com/v1"
                  className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
                />
              </div>
            )}

            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                模型
              </label>
              <input
                value={config.llm.model}
                onChange={(e) => updateLlm("model", e.target.value)}
                placeholder="deepseek-chat"
                className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
              />
            </div>
          </div>
        </section>

        {/* Traditional Engines Section */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4">
            传统翻译引擎
          </h2>

          <div className="space-y-4">
            {/* Google */}
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={config.engines.google.enabled}
                onChange={(e) =>
                  updateConfig((prev) => ({
                    ...prev,
                    engines: {
                      ...prev.engines,
                      google: { enabled: e.target.checked },
                    },
                  }))
                }
                className="accent-primary w-4 h-4"
              />
              <span className="text-sm">Google 翻译</span>
            </label>

            {/* Baidu */}
            <div>
              <label className="flex items-center gap-2 cursor-pointer mb-2">
                <input
                  type="checkbox"
                  checked={config.engines.baidu.enabled}
                  onChange={(e) =>
                    updateConfig((prev) => ({
                      ...prev,
                      engines: {
                        ...prev.engines,
                        baidu: {
                          ...prev.engines.baidu,
                          enabled: e.target.checked,
                        },
                      },
                    }))
                  }
                  className="accent-primary w-4 h-4"
                />
                <span className="text-sm">百度翻译</span>
              </label>
              {config.engines.baidu.enabled && (
                <div className="ml-6 space-y-2">
                  <input
                    value={config.engines.baidu.appId}
                    onChange={(e) =>
                      updateConfig((prev) => ({
                        ...prev,
                        engines: {
                          ...prev.engines,
                          baidu: {
                            ...prev.engines.baidu,
                            appId: e.target.value,
                          },
                        },
                      }))
                    }
                    placeholder="APP ID"
                    className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
                  />
                  <input
                    type="password"
                    value={config.engines.baidu.secret}
                    onChange={(e) =>
                      updateConfig((prev) => ({
                        ...prev,
                        engines: {
                          ...prev.engines,
                          baidu: {
                            ...prev.engines.baidu,
                            secret: e.target.value,
                          },
                        },
                      }))
                    }
                    placeholder="密钥"
                    className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
                  />
                </div>
              )}
            </div>

            {/* Youdao */}
            <div>
              <label className="flex items-center gap-2 cursor-pointer mb-2">
                <input
                  type="checkbox"
                  checked={config.engines.youdao.enabled}
                  onChange={(e) =>
                    updateConfig((prev) => ({
                      ...prev,
                      engines: {
                        ...prev.engines,
                        youdao: {
                          ...prev.engines.youdao,
                          enabled: e.target.checked,
                        },
                      },
                    }))
                  }
                  className="accent-primary w-4 h-4"
                />
                <span className="text-sm">有道翻译 (免费，自动获取密钥)</span>
              </label>
              {config.engines.youdao.enabled && (
                <div className="ml-6">
                  <p className="text-xs text-text-secondary mb-2">
                    通过 CDN 自动获取密钥，无需手动配置 API Key
                  </p>
                  <label className="flex items-center gap-2 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={config.engines.youdao.useAi}
                      onChange={(e) =>
                        updateConfig((prev) => ({
                          ...prev,
                          engines: {
                            ...prev.engines,
                            youdao: {
                              ...prev.engines.youdao,
                              useAi: e.target.checked,
                            },
                          },
                        }))
                      }
                      className="accent-primary w-4 h-4"
                    />
                    <span className="text-xs text-text-secondary">
                      使用 AI 翻译 (每日限量3次，质量更高)
                    </span>
                  </label>
                </div>
              )}
            </div>

            {/* DeepL */}
            <div>
              <label className="flex items-center gap-2 cursor-pointer mb-2">
                <input
                  type="checkbox"
                  checked={config.engines.deepl.enabled}
                  onChange={(e) =>
                    updateConfig((prev) => ({
                      ...prev,
                      engines: {
                        ...prev.engines,
                        deepl: {
                          ...prev.engines.deepl,
                          enabled: e.target.checked,
                        },
                      },
                    }))
                  }
                  className="accent-primary w-4 h-4"
                />
                <span className="text-sm">DeepL 翻译</span>
              </label>
              {config.engines.deepl.enabled && (
                <div className="ml-6 space-y-2">
                  <input
                    type="password"
                    value={config.engines.deepl.apiKey}
                    onChange={(e) =>
                      updateConfig((prev) => ({
                        ...prev,
                        engines: {
                          ...prev.engines,
                          deepl: {
                            ...prev.engines.deepl,
                            apiKey: e.target.value,
                          },
                        },
                      }))
                    }
                    placeholder="DeepL API Key"
                    className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
                  />
                  <label className="flex items-center gap-2 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={config.engines.deepl.pro}
                      onChange={(e) =>
                        updateConfig((prev) => ({
                          ...prev,
                          engines: {
                            ...prev.engines,
                            deepl: {
                              ...prev.engines.deepl,
                              pro: e.target.checked,
                            },
                          },
                        }))
                      }
                      className="accent-primary w-4 h-4"
                    />
                    <span className="text-xs text-text-secondary">
                      使用 DeepL Pro API
                    </span>
                  </label>
                </div>
              )}
            </div>

            {/* DeepLX */}
            <div>
              <label className="flex items-center gap-2 cursor-pointer mb-2">
                <input
                  type="checkbox"
                  checked={config.engines.deeplx.enabled}
                  onChange={(e) =>
                    updateConfig((prev) => ({
                      ...prev,
                      engines: {
                        ...prev.engines,
                        deeplx: {
                          ...prev.engines.deeplx,
                          enabled: e.target.checked,
                        },
                      },
                    }))
                  }
                  className="accent-primary w-4 h-4"
                />
                <span className="text-sm">DeepLX (免费内置)</span>
              </label>
              {config.engines.deeplx.enabled && (
                <div className="ml-6 space-y-2">
                  <p className="text-xs text-text-secondary">
                    内置 DeepL 免费接口，无需额外服务
                  </p>
                  <div>
                    <label className="text-xs text-text-secondary mb-1 block">API Key (可选，用于 Pro 模式)</label>
                    <input
                      type="password"
                      value={config.engines.deeplx.apiKey || ""}
                      onChange={(e) =>
                        updateConfig((prev) => ({
                          ...prev,
                          engines: {
                            ...prev.engines,
                            deeplx: {
                              ...prev.engines.deeplx,
                              apiKey: e.target.value,
                            },
                          },
                        }))
                      }
                      placeholder="留空使用免费模式"
                      className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
                    />
                  </div>
                  {config.engines.deeplx.apiKey && (
                    <label className="flex items-center gap-2 cursor-pointer">
                      <input
                        type="checkbox"
                        checked={config.engines.deeplx.pro}
                        onChange={(e) =>
                          updateConfig((prev) => ({
                            ...prev,
                            engines: {
                              ...prev.engines,
                              deeplx: {
                                ...prev.engines.deeplx,
                                pro: e.target.checked,
                              },
                            },
                          }))
                        }
                        className="accent-primary w-4 h-4"
                      />
                      <span className="text-xs">使用 DeepL Pro API</span>
                    </label>
                  )}
                </div>
              )}
            </div>

            {/* Microsoft */}
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={config.engines.microsoft.enabled}
                onChange={(e) =>
                  updateConfig((prev) => ({
                    ...prev,
                    engines: {
                      ...prev.engines,
                      microsoft: { enabled: e.target.checked },
                    },
                  }))
                }
                className="accent-primary w-4 h-4"
              />
              <span className="text-sm">Microsoft 翻译 (免费)</span>
            </label>

            {/* Yandex */}
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="checkbox"
                checked={config.engines.yandex.enabled}
                onChange={(e) =>
                  updateConfig((prev) => ({
                    ...prev,
                    engines: {
                      ...prev.engines,
                      yandex: { enabled: e.target.checked },
                    },
                  }))
                }
                className="accent-primary w-4 h-4"
              />
              <span className="text-sm">Yandex 翻译 (免费)</span>
            </label>
          </div>
        </section>

        {/* System Section */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4 flex items-center gap-2">
            <Power size={18} />
            系统设置
          </h2>
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-text-primary font-medium">
                开机自启动
              </p>
              <p className="text-xs text-text-secondary mt-1">
                系统启动时自动运行 Moon Translator
              </p>
            </div>
            <button
              className={`relative w-12 h-6 rounded-full transition-colors ${
                autostartEnabled ? "bg-primary" : "bg-bg-tertiary"
              }`}
              onClick={toggleAutostart}
            >
              <div
                className={`absolute top-0.5 w-5 h-5 rounded-full bg-white transition-transform ${
                  autostartEnabled ? "translate-x-6" : "translate-x-0.5"
                }`}
              />
            </button>
          </div>
        </section>

        {/* Cache Section */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4 flex items-center gap-2">
            <Database size={18} />
            翻译缓存
          </h2>
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-text-secondary">
                缓存已翻译内容，避免重复请求
              </p>
              <p className="text-sm text-text-primary mt-1">
                当前缓存: <span className="font-semibold">{cacheSize}</span> 条
              </p>
            </div>
            <button
              className="bg-bg-tertiary text-error border border-border rounded-lg px-4 py-2 text-sm hover:bg-error hover:text-white hover:border-error transition-colors flex items-center gap-1.5"
              onClick={clearCache}
              disabled={cacheSize === 0}
            >
              <Trash2 size={14} />
              清空缓存
            </button>
          </div>
        </section>

        {/* Custom Prompt Section */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4">
            {t("settings.prompt.title")}
          </h2>
          <p className="text-xs text-text-secondary mb-3">
            {t("settings.prompt.hint")}
          </p>
          <textarea
            value={config.customPrompt}
            onChange={(e) =>
              updateConfig((prev) => ({
                ...prev,
                customPrompt: e.target.value,
              }))
            }
            placeholder={t("settings.prompt.placeholder")}
            rows={5}
            className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none resize-y"
          />

          {/* Prompt Templates */}
          <div className="mt-4">
            <h3 className="text-sm font-medium text-text-primary mb-2">保存的提示词模板</h3>
            <div className="space-y-2 mb-3">
              {config.promptTemplates.map((template, index) => (
                <div
                  key={index}
                  className="flex items-center justify-between bg-bg-tertiary rounded-lg p-2 group"
                >
                  <button
                    className="flex-1 text-left text-sm text-text-primary hover:text-primary transition-colors"
                    onClick={() => applyTemplate(template.prompt)}
                    title={template.prompt}
                  >
                    {template.name}
                  </button>
                  <button
                    className="opacity-0 group-hover:opacity-100 transition-opacity p-1 rounded-md hover:bg-error/20 text-text-secondary hover:text-error"
                    onClick={() => removePromptTemplate(index)}
                  >
                    <X size={12} />
                  </button>
                </div>
              ))}
            </div>
            <div className="flex gap-2">
              <input
                type="text"
                value={newTemplateName}
                onChange={(e) => setNewTemplateName(e.target.value)}
                placeholder="模板名称"
                className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-1.5 text-sm focus:border-primary outline-none"
              />
              <button
                className="bg-primary text-white rounded-lg px-3 py-1.5 text-sm hover:bg-primary/80 transition-colors flex items-center gap-1"
                onClick={addPromptTemplate}
                disabled={!newTemplateName.trim() || !config.customPrompt.trim()}
              >
                <Plus size={14} />
                保存当前
              </button>
            </div>
          </div>
        </section>

        {/* Translation Options Section */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4 flex items-center gap-2">
            <Clipboard size={18} />
            {t("settings.options.title")}
          </h2>
          <div className="space-y-4">
            {/* Auto Copy */}
            <div>
              <div className="flex items-center justify-between mb-2">
                <div>
                  <p className="text-sm text-text-primary font-medium">
                    {t("settings.options.autoCopy")}
                  </p>
                  <p className="text-xs text-text-secondary mt-1">
                    {t("settings.options.autoCopyHint")}
                  </p>
                </div>
                <button
                  className={`relative w-12 h-6 rounded-full transition-colors ${
                    config.autoCopyResult ? "bg-primary" : "bg-bg-tertiary"
                  }`}
                  onClick={() =>
                    updateConfig((prev) => ({
                      ...prev,
                      autoCopyResult: !prev.autoCopyResult,
                    }))
                  }
                >
                  <div
                    className={`absolute top-0.5 w-5 h-5 rounded-full bg-white transition-transform ${
                      config.autoCopyResult
                        ? "translate-x-6"
                        : "translate-x-0.5"
                    }`}
                  />
                </button>
              </div>
              {config.autoCopyResult && (
                <div className="flex gap-2 ml-0 mt-2">
                  {[
                    { value: "translated", label: "译文" },
                    { value: "source", label: "原文" },
                    { value: "both", label: "原文+译文" },
                  ].map((mode) => (
                    <button
                      key={mode.value}
                      className={`px-3 py-1 rounded-lg text-xs transition-colors ${
                        config.autoCopyMode === mode.value
                          ? "bg-primary text-white"
                          : "bg-bg-tertiary text-text-secondary hover:bg-bg-tertiary/80"
                      }`}
                      onClick={() =>
                        updateConfig((prev) => ({
                          ...prev,
                          autoCopyMode: mode.value as any,
                        }))
                      }
                    >
                      {mode.label}
                    </button>
                  ))}
                </div>
              )}
            </div>

            {/* Clipboard Monitor */}
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-text-primary font-medium">
                  剪贴板监听翻译
                </p>
                <p className="text-xs text-text-secondary mt-1">
                  监听剪贴板变化，自动翻译复制的内容
                </p>
              </div>
              <button
                className={`relative w-12 h-6 rounded-full transition-colors ${
                  config.clipboardMonitor ? "bg-primary" : "bg-bg-tertiary"
                }`}
                onClick={() =>
                  updateConfig((prev) => ({
                    ...prev,
                    clipboardMonitor: !prev.clipboardMonitor,
                  }))
                }
              >
                <div
                  className={`absolute top-0.5 w-5 h-5 rounded-full bg-white transition-transform ${
                    config.clipboardMonitor
                      ? "translate-x-6"
                      : "translate-x-0.5"
                  }`}
                />
              </button>
            </div>

            {/* Translation Mask */}
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-text-primary font-medium flex items-center gap-1.5">
                  {config.translationMask ? <EyeOff size={14} /> : <Eye size={14} />}
                  翻译遮罩 (学习模式)
                </p>
                <p className="text-xs text-text-secondary mt-1">
                  遮挡原文，先看译文尝试理解，点击显示原文
                </p>
              </div>
              <button
                className={`relative w-12 h-6 rounded-full transition-colors ${
                  config.translationMask ? "bg-primary" : "bg-bg-tertiary"
                }`}
                onClick={() =>
                  updateConfig((prev) => ({
                    ...prev,
                    translationMask: !prev.translationMask,
                  }))
                }
              >
                <div
                  className={`absolute top-0.5 w-5 h-5 rounded-full bg-white transition-transform ${
                    config.translationMask
                      ? "translate-x-6"
                      : "translate-x-0.5"
                  }`}
                />
              </button>
            </div>
          </div>
        </section>

        {/* Translation Blacklist Section */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4 flex items-center gap-2">
            <EyeOff size={18} />
            {t("settings.blacklist.title")}
          </h2>
          <p className="text-xs text-text-secondary mb-4">
            {t("settings.blacklist.hint")}
          </p>

          <div className="space-y-3">
            {/* Blacklist items */}
            <div className="flex flex-wrap gap-2">
              {(config.translationBlacklist || []).map((word, index) => (
                <span
                  key={index}
                  className="inline-flex items-center gap-1 px-3 py-1 bg-bg-tertiary text-text-primary rounded-full text-sm"
                >
                  {word}
                  <button
                    className="text-text-secondary hover:text-red-500 transition-colors"
                    onClick={() =>
                      updateConfig((prev) => ({
                        ...prev,
                        translationBlacklist: prev.translationBlacklist.filter((_, i) => i !== index),
                      }))
                    }
                  >
                    <X size={12} />
                  </button>
                </span>
              ))}
              {(config.translationBlacklist || []).length === 0 && (
                <span className="text-xs text-text-secondary">{t("settings.blacklist.empty")}</span>
              )}
            </div>

            {/* Add new word */}
            <div className="flex gap-2">
              <input
                type="text"
                className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary focus:outline-none"
                placeholder={t("settings.blacklist.placeholder")}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    const input = e.target as HTMLInputElement;
                    const word = input.value.trim();
                    if (word && !(config.translationBlacklist || []).includes(word)) {
                      updateConfig((prev) => ({
                        ...prev,
                        translationBlacklist: [...(prev.translationBlacklist || []), word],
                      }));
                      input.value = "";
                    }
                  }
                }}
              />
              <button
                className="bg-primary text-white rounded-lg px-4 py-2 text-sm hover:bg-primary/80 transition-colors"
                onClick={(e) => {
                  const input = (e.target as HTMLElement).previousElementSibling as HTMLInputElement;
                  const word = input.value.trim();
                  if (word && !(config.translationBlacklist || []).includes(word)) {
                    updateConfig((prev) => ({
                      ...prev,
                      translationBlacklist: [...(prev.translationBlacklist || []), word],
                    }));
                    input.value = "";
                  }
                }}
              >
                {t("settings.blacklist.add")}
              </button>
            </div>
          </div>
        </section>

        {/* Post-Processing Section */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4 flex items-center gap-2">
            <Wand2 size={18} />
            {t("settings.postProcess.title")}
          </h2>
          <p className="text-xs text-text-secondary mb-4">
            {t("settings.postProcess.hint")}
          </p>

          <div className="space-y-4">
            {/* Trim Whitespace */}
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-text-primary font-medium">{t("settings.postProcess.trimWhitespace")}</p>
                <p className="text-xs text-text-secondary mt-1">{t("settings.postProcess.trimWhitespaceHint")}</p>
              </div>
              <button
                className={`relative w-12 h-6 rounded-full transition-colors ${postConfig.trimWhitespace ? "bg-primary" : "bg-bg-tertiary"}`}
                onClick={() => savePostProcessConfig({ ...postConfig, trimWhitespace: !postConfig.trimWhitespace })}
              >
                <div className={`absolute top-0.5 w-5 h-5 rounded-full bg-white transition-transform ${postConfig.trimWhitespace ? "translate-x-6" : "translate-x-0.5"}`} />
              </button>
            </div>

            {/* Fix Punctuation */}
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-text-primary font-medium">{t("settings.postProcess.fixPunctuation")}</p>
                <p className="text-xs text-text-secondary mt-1">{t("settings.postProcess.fixPunctuationHint")}</p>
              </div>
              <button
                className={`relative w-12 h-6 rounded-full transition-colors ${postConfig.fixPunctuation ? "bg-primary" : "bg-bg-tertiary"}`}
                onClick={() => savePostProcessConfig({ ...postConfig, fixPunctuation: !postConfig.fixPunctuation })}
              >
                <div className={`absolute top-0.5 w-5 h-5 rounded-full bg-white transition-transform ${postConfig.fixPunctuation ? "translate-x-6" : "translate-x-0.5"}`} />
              </button>
            </div>

            {/* Fix Newlines */}
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-text-primary font-medium">{t("settings.postProcess.fixNewlines")}</p>
                <p className="text-xs text-text-secondary mt-1">{t("settings.postProcess.fixNewlinesHint")}</p>
              </div>
              <button
                className={`relative w-12 h-6 rounded-full transition-colors ${postConfig.fixNewlines ? "bg-primary" : "bg-bg-tertiary"}`}
                onClick={() => savePostProcessConfig({ ...postConfig, fixNewlines: !postConfig.fixNewlines })}
              >
                <div className={`absolute top-0.5 w-5 h-5 rounded-full bg-white transition-transform ${postConfig.fixNewlines ? "translate-x-6" : "translate-x-0.5"}`} />
              </button>
            </div>

            {/* Replacement Rules */}
            <div className="pt-2 border-t border-border">
              <h3 className="text-sm font-medium text-text-primary mb-3">{t("settings.postProcess.replacementRules")}</h3>

              {postConfig.rules.length === 0 ? (
                <p className="text-xs text-text-secondary mb-3">{t("settings.postProcess.noRules")}</p>
              ) : (
                <div className="space-y-2 mb-3">
                  {postConfig.rules.map((rule) => (
                    <div key={rule.id} className="flex items-center gap-2 bg-bg-tertiary rounded-lg p-2 group">
                      <button
                        className={`w-4 h-4 rounded border flex-shrink-0 flex items-center justify-center ${rule.enabled ? "bg-primary border-primary" : "border-border"}`}
                        onClick={() => toggleRuleEnabled(rule)}
                      >
                        {rule.enabled && <Check size={10} className="text-white" />}
                      </button>
                      <div className="flex-1 min-w-0">
                        <span className={`text-xs font-mono ${!rule.enabled ? "text-text-secondary line-through" : "text-text-primary"}`}>
                          {rule.pattern}
                        </span>
                        <span className="text-xs text-text-secondary mx-1">→</span>
                        <span className={`text-xs font-mono ${!rule.enabled ? "text-text-secondary line-through" : "text-accent"}`}>
                          {rule.replacement}
                        </span>
                        {rule.isRegex && (
                          <span className="ml-1 text-xs bg-primary/20 text-primary px-1 rounded">.*</span>
                        )}
                      </div>
                      <button
                        className="opacity-0 group-hover:opacity-100 transition-opacity p-1 rounded-md hover:bg-error/20 text-text-secondary hover:text-error"
                        onClick={() => removeReplacementRule(rule.id)}
                      >
                        <X size={12} />
                      </button>
                    </div>
                  ))}
                </div>
              )}

              {/* Add new rule */}
              <div className="flex gap-2">
                <input
                  type="text"
                  value={newRulePattern}
                  onChange={(e) => setNewRulePattern(e.target.value)}
                  placeholder={t("settings.postProcess.pattern")}
                  className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-1.5 text-sm focus:border-primary outline-none"
                />
                <input
                  type="text"
                  value={newRuleReplacement}
                  onChange={(e) => setNewRuleReplacement(e.target.value)}
                  placeholder={t("settings.postProcess.replacement")}
                  className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-1.5 text-sm focus:border-primary outline-none"
                />
                <button
                  className={`px-2 py-1.5 rounded-lg text-xs transition-colors ${newRuleIsRegex ? "bg-primary text-white" : "bg-bg-tertiary text-text-secondary border border-border"}`}
                  onClick={() => setNewRuleIsRegex(!newRuleIsRegex)}
                  title={t("settings.postProcess.isRegex")}
                >
                  .*
                </button>
                <button
                  className="bg-primary text-white rounded-lg px-3 py-1.5 text-sm hover:bg-primary/80 transition-colors flex items-center gap-1"
                  onClick={addReplacementRule}
                  disabled={!newRulePattern.trim()}
                >
                  <Plus size={14} />
                  {t("settings.postProcess.addRule")}
                </button>
              </div>
            </div>

            {/* Test */}
            <div className="pt-2 border-t border-border">
              <h3 className="text-sm font-medium text-text-primary mb-3">{t("settings.postProcess.test")}</h3>
              <div className="flex gap-2 mb-2">
                <input
                  type="text"
                  value={testInput}
                  onChange={(e) => setTestInput(e.target.value)}
                  placeholder={t("settings.postProcess.testInput")}
                  className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-1.5 text-sm focus:border-primary outline-none"
                  onKeyDown={(e) => e.key === "Enter" && runPostProcessTest()}
                />
                <button
                  className="bg-accent text-white rounded-lg px-3 py-1.5 text-sm hover:bg-accent/80 transition-colors"
                  onClick={runPostProcessTest}
                  disabled={!testInput.trim()}
                >
                  {t("settings.postProcess.test")}
                </button>
              </div>
              {testOutput && (
                <div className="bg-bg-tertiary rounded-lg p-3 text-sm text-text-primary font-mono">
                  {testOutput}
                </div>
              )}
            </div>
          </div>
        </section>

        {/* Window Follow Section */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4 flex items-center gap-2">
            <MousePointer size={18} />
            {t("settings.windowFollow.title")}
          </h2>
          <p className="text-xs text-text-secondary mb-4">
            {t("settings.windowFollow.hint")}
          </p>
          <div className="flex gap-3">
            {(["none", "cursor"] as const).map((mode) => (
              <button
                key={mode}
                className={`px-4 py-2 rounded-lg text-sm border transition-colors ${
                  config.windowFollowMode === mode
                    ? "bg-primary text-white border-primary"
                    : "bg-bg-tertiary text-text-secondary border-border hover:border-primary"
                }`}
                onClick={() =>
                  updateConfig((prev) => ({
                    ...prev,
                    windowFollowMode: mode,
                  }))
                }
              >
                {t(`settings.windowFollow.${mode}`)}
              </button>
            ))}
          </div>
        </section>

        {/* Hotkey Settings Section */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4 flex items-center gap-2">
            <Keyboard size={18} />
            快捷键设置
          </h2>
          <p className="text-xs text-text-secondary mb-4">
            自定义全局快捷键。修改后需要重启应用生效。
          </p>
          <div className="space-y-4">
            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                OCR 截图翻译
              </label>
              <input
                value={config.hotkeys.ocrTranslate}
                onChange={(e) =>
                  updateConfig((prev) => ({
                    ...prev,
                    hotkeys: { ...prev.hotkeys, ocrTranslate: e.target.value },
                  }))
                }
                placeholder="Ctrl+Shift+T"
                className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
              />
            </div>
            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                显示主窗口
              </label>
              <input
                value={config.hotkeys.showWindow}
                onChange={(e) =>
                  updateConfig((prev) => ({
                    ...prev,
                    hotkeys: { ...prev.hotkeys, showWindow: e.target.value },
                  }))
                }
                placeholder="Ctrl+T"
                className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
              />
            </div>
            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                翻译选中文本
              </label>
              <input
                value={config.hotkeys.translateSelection}
                onChange={(e) =>
                  updateConfig((prev) => ({
                    ...prev,
                    hotkeys: { ...prev.hotkeys, translateSelection: e.target.value },
                  }))
                }
                placeholder="Ctrl+Shift+Y"
                className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
              />
            </div>
            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                替换翻译
              </label>
              <input
                value={config.hotkeys.replaceTranslate || "Ctrl+Shift+R"}
                onChange={(e) =>
                  updateConfig((prev) => ({
                    ...prev,
                    hotkeys: { ...prev.hotkeys, replaceTranslate: e.target.value },
                  }))
                }
                placeholder="Ctrl+Shift+R"
                className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
              />
            </div>
          </div>
        </section>

        {/* Proxy Settings Section */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4 flex items-center gap-2">
            <Globe size={18} />
            代理设置
          </h2>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-text-primary font-medium">
                  启用代理
                </p>
                <p className="text-xs text-text-secondary mt-1">
                  通过代理服务器发送翻译请求
                </p>
              </div>
              <button
                className={`relative w-12 h-6 rounded-full transition-colors ${
                  config.proxy.enabled ? "bg-primary" : "bg-bg-tertiary"
                }`}
                onClick={() =>
                  updateConfig((prev) => ({
                    ...prev,
                    proxy: { ...prev.proxy, enabled: !prev.proxy.enabled },
                  }))
                }
              >
                <div
                  className={`absolute top-0.5 w-5 h-5 rounded-full bg-white transition-transform ${
                    config.proxy.enabled ? "translate-x-6" : "translate-x-0.5"
                  }`}
                />
              </button>
            </div>

            {config.proxy.enabled && (
              <>
                <div>
                  <label className="block text-xs text-text-secondary mb-1.5">
                    代理类型
                  </label>
                  <select
                    value={config.proxy.proxyType}
                    onChange={(e) =>
                      updateConfig((prev) => ({
                        ...prev,
                        proxy: { ...prev.proxy, proxyType: e.target.value },
                      }))
                    }
                    className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary"
                  >
                    <option value="http">HTTP</option>
                    <option value="https">HTTPS</option>
                    <option value="socks5">SOCKS5</option>
                  </select>
                </div>
                <div className="flex gap-3">
                  <div className="flex-1">
                    <label className="block text-xs text-text-secondary mb-1.5">
                      主机
                    </label>
                    <input
                      value={config.proxy.host}
                      onChange={(e) =>
                        updateConfig((prev) => ({
                          ...prev,
                          proxy: { ...prev.proxy, host: e.target.value },
                        }))
                      }
                      placeholder="127.0.0.1"
                      className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
                    />
                  </div>
                  <div className="w-24">
                    <label className="block text-xs text-text-secondary mb-1.5">
                      端口
                    </label>
                    <input
                      type="number"
                      value={config.proxy.port}
                      onChange={(e) =>
                        updateConfig((prev) => ({
                          ...prev,
                          proxy: {
                            ...prev.proxy,
                            port: parseInt(e.target.value) || 7890,
                          },
                        }))
                      }
                      className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
                    />
                  </div>
                </div>
                <div className="flex gap-3">
                  <div className="flex-1">
                    <label className="block text-xs text-text-secondary mb-1.5">
                      用户名 (可选)
                    </label>
                    <input
                      value={config.proxy.username}
                      onChange={(e) =>
                        updateConfig((prev) => ({
                          ...prev,
                          proxy: { ...prev.proxy, username: e.target.value },
                        }))
                      }
                      className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
                    />
                  </div>
                  <div className="flex-1">
                    <label className="block text-xs text-text-secondary mb-1.5">
                      密码 (可选)
                    </label>
                    <input
                      type="password"
                      value={config.proxy.password}
                      onChange={(e) =>
                        updateConfig((prev) => ({
                          ...prev,
                          proxy: { ...prev.proxy, password: e.target.value },
                        }))
                      }
                      className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
                    />
                  </div>
                </div>
              </>
            )}
          </div>
        </section>

        {/* API Server Section */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4 flex items-center gap-2">
            <Globe size={18} />
            API 服务器
          </h2>
          <p className="text-xs text-text-secondary mb-4">
            开启本地HTTP API服务器，允许外部工具调用翻译功能。
            重启应用后生效。
          </p>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-text-primary font-medium">
                  启用 API 服务器
                </p>
                <p className="text-xs text-text-secondary mt-1">
                  在本地端口提供 REST API 接口
                </p>
              </div>
              <button
                className={`relative w-12 h-6 rounded-full transition-colors ${
                  config.apiServerEnabled ? "bg-primary" : "bg-bg-tertiary"
                }`}
                onClick={() =>
                  updateConfig((prev) => ({
                    ...prev,
                    apiServerEnabled: !prev.apiServerEnabled,
                  }))
                }
              >
                <div
                  className={`absolute top-0.5 w-5 h-5 rounded-full bg-white transition-transform ${
                    config.apiServerEnabled
                      ? "translate-x-6"
                      : "translate-x-0.5"
                  }`}
                />
              </button>
            </div>

            {config.apiServerEnabled && (
              <div>
                <label className="block text-xs text-text-secondary mb-1.5">
                  端口号
                </label>
                <input
                  type="number"
                  value={config.apiServerPort}
                  onChange={(e) =>
                    updateConfig((prev) => ({
                      ...prev,
                      apiServerPort: parseInt(e.target.value) || 60828,
                    }))
                  }
                  min={1024}
                  max={65535}
                  className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
                />
                <p className="text-xs text-text-secondary mt-2">
                  API 地址: http://127.0.0.1:{config.apiServerPort}
                </p>
                <div className="mt-3 p-3 bg-bg-tertiary rounded-lg text-xs text-text-secondary font-mono">
                  <p className="mb-1">可用接口:</p>
                  <p>POST /translate - 多引擎翻译</p>
                  <p>POST /translate/primary - 主引擎翻译</p>
                  <p>GET /config - 获取配置</p>
                  <p>POST /config - 更新配置</p>
                  <p>GET /history - 翻译历史</p>
                  <p>GET /engines - 引擎列表</p>
                  <p>GET /health - 健康检查</p>
                </div>
              </div>
            )}
          </div>
        </section>

        {/* Import/Export Section */}
        <section className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-base font-semibold text-primary mb-4 flex items-center gap-2">
            <Download size={18} />
            配置备份
          </h2>
          <p className="text-xs text-text-secondary mb-4">
            导出当前配置为JSON文件，或从文件导入配置。
          </p>
          <div className="flex gap-3">
            <button
              className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-4 py-2 text-sm hover:bg-primary hover:text-white hover:border-primary transition-colors flex items-center gap-1.5"
              onClick={exportConfig}
            >
              <Download size={14} />
              导出配置
            </button>
            <button
              className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-4 py-2 text-sm hover:bg-accent hover:text-white hover:border-accent transition-colors flex items-center gap-1.5"
              onClick={importConfig}
            >
              <Upload size={14} />
              导入配置
            </button>
          </div>
        </section>

        {/* Save Button */}
        <div className="flex justify-center">
          <button
            className="bg-primary text-bg-primary font-semibold rounded-lg px-8 py-2.5 text-sm hover:bg-primary-hover transition-colors flex items-center gap-2"
            onClick={saveConfig}
          >
            {saved ? (
              <>
                <Check size={16} />
                已保存
              </>
            ) : (
              <>
                <Save size={16} />
                保存设置
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}

export default Settings;
