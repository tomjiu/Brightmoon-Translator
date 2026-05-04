import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { Puzzle, FolderOpen, RefreshCw, ExternalLink } from "lucide-react";

interface PluginManifest {
  name: string;
  version: string;
  description: string;
  author: string;
  type: "translation" | "ocr" | "tts";
  enabled: boolean;
  translation?: {
    endpoint: string;
    supportedLanguages: string[][];
    headers: Record<string, string>;
  };
}

interface PluginInfo {
  manifest: PluginManifest;
  path: string;
}

function Plugins() {
  const { t } = useI18n();
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [pluginsDir, setPluginsDir] = useState("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadPlugins();
    loadPluginsDir();
  }, []);

  const loadPlugins = async () => {
    setLoading(true);
    try {
      const result = await invoke<PluginInfo[]>("get_plugins");
      setPlugins(result);
    } catch (err) {
      console.error("Failed to load plugins:", err);
    }
    setLoading(false);
  };

  const loadPluginsDir = async () => {
    try {
      const dir = await invoke<string>("get_plugins_dir");
      setPluginsDir(dir);
    } catch (err) {
      console.error("Failed to get plugins dir:", err);
    }
  };

  const togglePlugin = async (pluginName: string, enabled: boolean) => {
    try {
      await invoke("set_plugin_enabled", { pluginName, enabled });
      await loadPlugins();
    } catch (err) {
      console.error("Failed to toggle plugin:", err);
    }
  };

  const openPluginsDir = async () => {
    try {
      await invoke("open_plugins_dir");
    } catch (err) {
      console.error("Failed to open plugins dir:", err);
    }
  };

  const getTypeLabel = (type: string) => {
    switch (type) {
      case "translation": return t("plugins.typeTranslation");
      case "ocr": return t("plugins.typeOcr");
      case "tts": return t("plugins.typeTts");
      default: return type;
    }
  };

  const getTypeColor = (type: string) => {
    switch (type) {
      case "translation": return "bg-primary/20 text-primary";
      case "ocr": return "bg-accent/20 text-accent";
      case "tts": return "bg-warning/20 text-warning";
      default: return "bg-bg-tertiary text-text-secondary";
    }
  };

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="max-w-2xl mx-auto">
        <div className="flex items-center justify-between mb-6">
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Puzzle size={24} className="text-primary" />
            {t("plugins.title")}
          </h1>
          <button
            className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-primary hover:text-white hover:border-primary transition-colors flex items-center gap-1.5"
            onClick={loadPlugins}
          >
            <RefreshCw size={14} />
            {t("plugins.refresh")}
          </button>
        </div>

        {/* Plugins Directory */}
        <div className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-text-primary font-medium">{t("plugins.directory")}</p>
              <p className="text-xs text-text-secondary mt-1 font-mono break-all">{pluginsDir}</p>
            </div>
            <button
              className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-accent hover:text-white hover:border-accent transition-colors flex items-center gap-1.5"
              onClick={openPluginsDir}
            >
              <FolderOpen size={14} />
              {t("plugins.openDir")}
            </button>
          </div>
        </div>

        {/* Plugin Format Help */}
        <div className="bg-bg-secondary border border-border rounded-xl p-5 mb-5">
          <h2 className="text-sm font-semibold text-primary mb-3">{t("plugins.howTo")}</h2>
          <div className="text-xs text-text-secondary space-y-2">
            <p>{t("plugins.howToHint")}</p>
            <div className="bg-bg-tertiary rounded-lg p-3 font-mono">
              <p className="text-text-primary mb-1">plugins/</p>
              <p className="text-text-primary ml-2 mb-1">my-plugin/</p>
              <p className="ml-4">manifest.json</p>
            </div>
            <p className="mt-2">{t("plugins.manifestExample")}:</p>
            <pre className="bg-bg-tertiary rounded-lg p-3 overflow-x-auto">
{`{
  "name": "My Translator",
  "version": "1.0.0",
  "description": "Custom translation plugin",
  "author": "Your Name",
  "type": "translation",
  "enabled": true,
  "translation": {
    "endpoint": "http://localhost:8080/translate",
    "supportedLanguages": [],
    "headers": {}
  }
}`}
            </pre>
            <p className="mt-2">{t("plugins.apiFormat")}:</p>
            <pre className="bg-bg-tertiary rounded-lg p-3 overflow-x-auto">
{`// Request (POST):
{ "text": "Hello", "from": "en", "to": "zh" }

// Response:
{ "translated": "你好" }`}
            </pre>
          </div>
        </div>

        {/* Plugin List */}
        <div className="space-y-3">
          {loading ? (
            <div className="text-center py-12 text-text-secondary">
              <RefreshCw size={24} className="animate-spin mx-auto mb-3" />
              {t("common.loading")}
            </div>
          ) : plugins.length === 0 ? (
            <div className="text-center py-12 text-text-secondary">
              <Puzzle size={48} className="mx-auto mb-3 opacity-30" />
              <p>{t("plugins.noPlugins")}</p>
              <p className="text-xs mt-2">{t("plugins.addHint")}</p>
            </div>
          ) : (
            plugins.map((plugin) => (
              <div
                key={plugin.manifest.name}
                className="bg-bg-secondary border border-border rounded-xl p-5"
              >
                <div className="flex items-start justify-between">
                  <div className="flex-1">
                    <div className="flex items-center gap-2 mb-2">
                      <h3 className="text-base font-semibold text-text-primary">
                        {plugin.manifest.name}
                      </h3>
                      <span className="text-xs text-text-secondary">
                        v{plugin.manifest.version}
                      </span>
                      <span className={`text-xs px-2 py-0.5 rounded-full ${getTypeColor(plugin.manifest.type)}`}>
                        {getTypeLabel(plugin.manifest.type)}
                      </span>
                    </div>
                    {plugin.manifest.description && (
                      <p className="text-sm text-text-secondary mb-2">
                        {plugin.manifest.description}
                      </p>
                    )}
                    <div className="flex items-center gap-4 text-xs text-text-secondary">
                      {plugin.manifest.author && (
                        <span>{t("plugins.author")}: {plugin.manifest.author}</span>
                      )}
                      {plugin.manifest.translation?.endpoint && (
                        <span className="font-mono flex items-center gap-1">
                          <ExternalLink size={10} />
                          {plugin.manifest.translation.endpoint}
                        </span>
                      )}
                    </div>
                  </div>
                  <button
                    className={`relative w-12 h-6 rounded-full transition-colors flex-shrink-0 ${
                      plugin.manifest.enabled ? "bg-primary" : "bg-bg-tertiary"
                    }`}
                    onClick={() => togglePlugin(plugin.manifest.name, !plugin.manifest.enabled)}
                  >
                    <div
                      className={`absolute top-0.5 w-5 h-5 rounded-full bg-white transition-transform ${
                        plugin.manifest.enabled ? "translate-x-6" : "translate-x-0.5"
                      }`}
                    />
                  </button>
                </div>
              </div>
            ))
          )}
        </div>

        {plugins.length > 0 && (
          <p className="text-xs text-text-secondary text-center mt-4">
            {t("plugins.restartHint")}
          </p>
        )}
      </div>
    </div>
  );
}

export default Plugins;
