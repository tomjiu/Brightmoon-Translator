import { useState, useMemo, useEffect, useCallback } from "react";
import { useI18n } from "../i18n";
import { invokeOrThrow } from "../services/invoke";
import { isTauriRuntime } from "../services/tauriRuntime";
import {
  Search,
  Download,
  Trash2,
  RefreshCw,
  Star,
  Users,
  X,
  Check,
  ChevronRight,
  ShoppingBag,
  Loader2,
} from "lucide-react";

// ── Types ───────────────────────────────────────────────────────────────────────

type PluginCategory = "translation" | "text-processing" | "ui-enhancement" | "other";
type PluginStatus = "available" | "installed" | "update-available";

/** Shape returned by the backend `plugin_list_marketplace` command. */
interface MarketplaceEntryBackend {
  id: string;
  name: string;
  description: string;
  fullDescription: string;
  author: string;
  category: string;
  icon: string;
  rating: number;
  downloads: number;
  version: string;
  latestVersion: string;
  permissions: string[];
  changelog: Array<{ version: string; date: string; changes: string }>;
  downloadUrl: string;
  installed: boolean;
}

/** Frontend view-model derived from the backend entry. */
interface MarketplacePlugin {
  id: string;
  name: string;
  description: string;
  fullDescription: string;
  author: string;
  category: PluginCategory;
  icon: string;
  rating: number;
  downloads: number;
  version: string;
  latestVersion: string;
  status: PluginStatus;
  permissions: string[];
  changelog: Array<{ version: string; date: string; changes: string }>;
}

// ── Mapping helpers ─────────────────────────────────────────────────────────────

function toPluginStatus(entry: MarketplaceEntryBackend): PluginStatus {
  if (!entry.installed) return "available";
  if (entry.version !== entry.latestVersion) return "update-available";
  return "installed";
}

function toMarketplacePlugin(entry: MarketplaceEntryBackend): MarketplacePlugin {
  return {
    id: entry.id,
    name: entry.name,
    description: entry.description,
    fullDescription: entry.fullDescription,
    author: entry.author,
    category: entry.category as PluginCategory,
    icon: entry.icon,
    rating: entry.rating,
    downloads: entry.downloads,
    version: entry.version,
    latestVersion: entry.latestVersion,
    status: toPluginStatus(entry),
    permissions: entry.permissions,
    changelog: entry.changelog,
  };
}

// ── Helper ──────────────────────────────────────────────────────────────────────

const CATEGORY_KEYS: Record<PluginCategory, string> = {
  translation: "marketplace.catTranslation",
  "text-processing": "marketplace.catTextProcessing",
  "ui-enhancement": "marketplace.catUiEnhancement",
  other: "marketplace.catOther",
};

const CATEGORY_ICONS: Record<PluginCategory, string> = {
  translation: "\u{1F310}",
  "text-processing": "\u{1F4DD}",
  "ui-enhancement": "\u{1F3A8}",
  other: "\u{1F4E6}",
};

const CATEGORIES: Array<PluginCategory | "all"> = [
  "all",
  "translation",
  "text-processing",
  "ui-enhancement",
  "other",
];

// ── Component ───────────────────────────────────────────────────────────────────

function PluginMarketplace() {
  const { t } = useI18n();
  const isTauri = isTauriRuntime();
  const [search, setSearch] = useState("");
  const [activeCategory, setActiveCategory] = useState<PluginCategory | "all">("all");
  const [selectedPlugin, setSelectedPlugin] = useState<MarketplacePlugin | null>(null);
  const [plugins, setPlugins] = useState<MarketplacePlugin[]>([]);
  const [loading, setLoading] = useState(true);
  const [actionLoading, setActionLoading] = useState<string | null>(null);

  // ── Load marketplace plugins from backend on mount ──────────────────────────

  const loadPlugins = useCallback(async () => {
    if (!isTauri) {
      setPlugins([]);
      setLoading(false);
      return;
    }

    setLoading(true);
    try {
      const entries = await invokeOrThrow<MarketplaceEntryBackend[]>(
        "plugin_list_marketplace"
      );
      setPlugins(entries.map(toMarketplacePlugin));
    } catch {
      // Error toast is shown by invokeOrThrow
    } finally {
      setLoading(false);
    }
  }, [isTauri]);

  useEffect(() => {
    void loadPlugins();
  }, [loadPlugins]);

  // ── Filter logic ───────────────────────────────────────────────────────────

  const filtered = useMemo(() => {
    return plugins.filter((p) => {
      const matchesSearch =
        !search ||
        p.name.toLowerCase().includes(search.toLowerCase()) ||
        p.description.toLowerCase().includes(search.toLowerCase());
      const matchesCategory = activeCategory === "all" || p.category === activeCategory;
      return matchesSearch && matchesCategory;
    });
  }, [plugins, search, activeCategory]);

  const installedPlugins = plugins.filter((p) => p.status !== "available");

  // ── Action handlers (install / uninstall / update) ─────────────────────────

  const handleAction = async (pluginId: string, action: "install" | "uninstall" | "update") => {
    setActionLoading(pluginId);
    try {
      switch (action) {
        case "install": {
          const updated = await invokeOrThrow<MarketplaceEntryBackend>(
            "plugin_install_marketplace",
            { id: pluginId }
          );
          const mapped = toMarketplacePlugin(updated);
          setPlugins((prev) => prev.map((p) => (p.id === pluginId ? mapped : p)));
          if (selectedPlugin?.id === pluginId) setSelectedPlugin(mapped);
          break;
        }
        case "uninstall": {
          await invokeOrThrow<null>("plugin_uninstall_marketplace", { id: pluginId });
          setPlugins((prev) =>
            prev.map((p) =>
              p.id === pluginId ? { ...p, status: "available" as const, version: p.latestVersion } : p
            )
          );
          if (selectedPlugin?.id === pluginId) {
            setSelectedPlugin((prev) =>
              prev ? { ...prev, status: "available" as const } : null
            );
          }
          break;
        }
        case "update": {
          const updated = await invokeOrThrow<MarketplaceEntryBackend>(
            "plugin_update_marketplace",
            { id: pluginId }
          );
          const mapped = toMarketplacePlugin(updated);
          setPlugins((prev) => prev.map((p) => (p.id === pluginId ? mapped : p)));
          if (selectedPlugin?.id === pluginId) setSelectedPlugin(mapped);
          break;
        }
      }
    } catch {
      // Error toast is shown by invokeOrThrow
    } finally {
      setActionLoading(null);
    }
  };

  // ── Render helpers ─────────────────────────────────────────────────────────

  const renderStars = (rating: number) => (
    <span className="flex items-center gap-0.5">
      {[1, 2, 3, 4, 5].map((i) => (
        <Star
          key={i}
          size={12}
          className={
            i <= Math.round(rating)
              ? "text-warning fill-warning"
              : "text-text-secondary opacity-30"
          }
        />
      ))}
      <span className="ml-1 text-xs text-text-secondary">{rating.toFixed(1)}</span>
    </span>
  );

  const renderActionButton = (plugin: MarketplacePlugin) => {
    const isLoading = actionLoading === plugin.id;
    const disabledClass = isLoading ? "opacity-60 pointer-events-none" : "";

    switch (plugin.status) {
      case "available":
        return (
          <button
            onClick={(e) => {
              e.stopPropagation();
              void handleAction(plugin.id, "install");
            }}
            className={`bg-primary text-white text-xs px-3 py-1.5 rounded-lg hover:bg-primary-hover transition-colors flex items-center gap-1 ${disabledClass}`}
          >
            {isLoading ? <Loader2 size={12} className="animate-spin" /> : <Download size={12} />}
            {t("marketplace.install")}
          </button>
        );
      case "update-available":
        return (
          <button
            onClick={(e) => {
              e.stopPropagation();
              void handleAction(plugin.id, "update");
            }}
            className={`bg-accent text-white text-xs px-3 py-1.5 rounded-lg hover:opacity-80 transition-colors flex items-center gap-1 ${disabledClass}`}
          >
            {isLoading ? <Loader2 size={12} className="animate-spin" /> : <RefreshCw size={12} />}
            {t("marketplace.update")}
          </button>
        );
      case "installed":
        return (
          <button
            onClick={(e) => {
              e.stopPropagation();
              void handleAction(plugin.id, "uninstall");
            }}
            className={`bg-bg-tertiary text-text-secondary text-xs px-3 py-1.5 rounded-lg hover:bg-error hover:text-white transition-colors flex items-center gap-1 ${disabledClass}`}
          >
            {isLoading ? <Loader2 size={12} className="animate-spin" /> : <Trash2 size={12} />}
            {t("marketplace.uninstall")}
          </button>
        );
    }
  };

  // ── Loading state ──────────────────────────────────────────────────────────

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <Loader2 size={32} className="animate-spin text-primary" />
      </div>
    );
  }

  // ── Main render ────────────────────────────────────────────────────────────

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="max-w-3xl mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <ShoppingBag size={24} className="text-primary" />
            {t("marketplace.title")}
          </h1>
        </div>

        {/* Search */}
        <div className="relative mb-4">
          <Search
            size={16}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary"
          />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t("marketplace.searchPlaceholder")}
            className="w-full bg-bg-secondary border border-border rounded-xl pl-10 pr-4 py-2.5 text-sm text-text-primary placeholder:text-text-secondary focus:outline-none focus:border-primary transition-colors"
          />
          {search && (
            <button
              onClick={() => setSearch("")}
              className="absolute right-3 top-1/2 -translate-y-1/2 text-text-secondary hover:text-text-primary"
            >
              <X size={14} />
            </button>
          )}
        </div>

        {/* Category Tabs */}
        <div className="flex gap-2 mb-6 overflow-x-auto">
          {CATEGORIES.map((cat) => (
            <button
              key={cat}
              onClick={() => setActiveCategory(cat)}
              className={`px-3 py-1.5 rounded-lg text-sm whitespace-nowrap transition-colors ${
                activeCategory === cat
                  ? "bg-primary text-white"
                  : "bg-bg-tertiary text-text-secondary hover:text-text-primary"
              }`}
            >
              {cat === "all"
                ? t("marketplace.catAll")
                : `${CATEGORY_ICONS[cat]} ${t(CATEGORY_KEYS[cat])}`}
            </button>
          ))}
        </div>

        {/* Plugin Grid */}
        {filtered.length === 0 ? (
          <div className="text-center py-12 text-text-secondary">
            <ShoppingBag size={48} className="mx-auto mb-3 opacity-30" />
            <p>{t("marketplace.noResults")}</p>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-8">
            {filtered.map((plugin) => (
              <div
                key={plugin.id}
                onClick={() => setSelectedPlugin(plugin)}
                className="bg-bg-secondary border border-border rounded-xl p-4 cursor-pointer hover:border-primary/50 transition-colors group"
              >
                <div className="flex items-start gap-3">
                  <span className="text-2xl shrink-0">{plugin.icon}</span>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <h3 className="text-sm font-semibold text-text-primary truncate">
                        {plugin.name}
                      </h3>
                      {plugin.status === "installed" && (
                        <Check size={14} className="text-success shrink-0" />
                      )}
                      {plugin.status === "update-available" && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-accent/20 text-accent shrink-0">
                          {t("marketplace.updateAvailable")}
                        </span>
                      )}
                    </div>
                    <p className="text-xs text-text-secondary line-clamp-2 mb-2">
                      {plugin.description}
                    </p>
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-3 text-xs text-text-secondary">
                        {renderStars(plugin.rating)}
                        <span className="flex items-center gap-0.5">
                          <Users size={10} />
                          {plugin.downloads >= 1000
                            ? `${(plugin.downloads / 1000).toFixed(1)}k`
                            : plugin.downloads}
                        </span>
                      </div>
                      {renderActionButton(plugin)}
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Installed Plugins Section */}
        {installedPlugins.length > 0 && (
          <div className="mb-6">
            <h2 className="text-lg font-semibold mb-3 flex items-center gap-2">
              <Check size={18} className="text-success" />
              {t("marketplace.installedTitle")} ({installedPlugins.length})
            </h2>
            <div className="bg-bg-secondary border border-border rounded-xl divide-y divide-border">
              {installedPlugins.map((plugin) => (
                <div
                  key={plugin.id}
                  onClick={() => setSelectedPlugin(plugin)}
                  className="flex items-center gap-3 px-4 py-3 cursor-pointer hover:bg-bg-tertiary/50 transition-colors"
                >
                  <span className="text-lg">{plugin.icon}</span>
                  <div className="flex-1 min-w-0">
                    <span className="text-sm font-medium text-text-primary">
                      {plugin.name}
                    </span>
                    <span className="text-xs text-text-secondary ml-2">
                      v{plugin.version}
                    </span>
                  </div>
                  {plugin.status === "update-available" && (
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        void handleAction(plugin.id, "update");
                      }}
                      className={`bg-accent text-white text-xs px-2.5 py-1 rounded-lg hover:opacity-80 transition-colors flex items-center gap-1 ${
                        actionLoading === plugin.id ? "opacity-60 pointer-events-none" : ""
                      }`}
                    >
                      {actionLoading === plugin.id ? (
                        <Loader2 size={10} className="animate-spin" />
                      ) : (
                        <RefreshCw size={10} />
                      )}
                      {t("marketplace.update")}
                    </button>
                  )}
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      void handleAction(plugin.id, "uninstall");
                    }}
                    className={`text-text-secondary hover:text-error transition-colors ${
                      actionLoading === plugin.id ? "opacity-60 pointer-events-none" : ""
                    }`}
                    title={t("marketplace.uninstall")}
                  >
                    {actionLoading === plugin.id ? (
                      <Loader2 size={14} className="animate-spin" />
                    ) : (
                      <Trash2 size={14} />
                    )}
                  </button>
                  <ChevronRight size={14} className="text-text-secondary" />
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Detail Modal */}
      {selectedPlugin && (
        <div
          className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4"
          onClick={() => setSelectedPlugin(null)}
        >
          <div
            className="bg-bg-secondary border border-border rounded-2xl w-full max-w-lg max-h-[80vh] overflow-y-auto"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Modal Header */}
            <div className="sticky top-0 bg-bg-secondary border-b border-border px-6 py-4 flex items-center justify-between">
              <div className="flex items-center gap-3">
                <span className="text-3xl">{selectedPlugin.icon}</span>
                <div>
                  <h2 className="text-lg font-bold text-text-primary">
                    {selectedPlugin.name}
                  </h2>
                  <p className="text-xs text-text-secondary">
                    {t("marketplace.author")}: {selectedPlugin.author} &middot; v
                    {selectedPlugin.version}
                  </p>
                </div>
              </div>
              <button
                onClick={() => setSelectedPlugin(null)}
                className="text-text-secondary hover:text-text-primary transition-colors"
              >
                <X size={20} />
              </button>
            </div>

            {/* Modal Body */}
            <div className="px-6 py-4 space-y-5">
              {/* Action */}
              <div className="flex items-center gap-3">
                {renderActionButton(selectedPlugin)}
                <div className="flex items-center gap-2 text-sm text-text-secondary">
                  {renderStars(selectedPlugin.rating)}
                  <span className="flex items-center gap-1">
                    <Users size={12} />
                    {selectedPlugin.downloads.toLocaleString()}
                  </span>
                </div>
              </div>

              {/* Description */}
              <div>
                <h3 className="text-sm font-semibold text-text-primary mb-2">
                  {t("marketplace.description")}
                </h3>
                <p className="text-sm text-text-secondary leading-relaxed">
                  {selectedPlugin.fullDescription}
                </p>
              </div>

              {/* Permissions */}
              <div>
                <h3 className="text-sm font-semibold text-text-primary mb-2">
                  {t("marketplace.permissions")}
                </h3>
                <div className="flex flex-wrap gap-2">
                  {selectedPlugin.permissions.map((perm) => (
                    <span
                      key={perm}
                      className="text-xs bg-warning/10 text-warning px-2 py-1 rounded-lg"
                    >
                      {perm}
                    </span>
                  ))}
                </div>
              </div>

              {/* Changelog */}
              <div>
                <h3 className="text-sm font-semibold text-text-primary mb-2">
                  {t("marketplace.versionHistory")}
                </h3>
                <div className="space-y-3">
                  {selectedPlugin.changelog.map((entry) => (
                    <div key={entry.version} className="flex gap-3">
                      <div className="shrink-0 pt-0.5">
                        <span className="text-xs bg-bg-tertiary text-text-primary px-2 py-0.5 rounded font-mono">
                          v{entry.version}
                        </span>
                      </div>
                      <div>
                        <p className="text-xs text-text-secondary mb-0.5">
                          {entry.date}
                        </p>
                        <p className="text-sm text-text-primary">{entry.changes}</p>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default PluginMarketplace;
