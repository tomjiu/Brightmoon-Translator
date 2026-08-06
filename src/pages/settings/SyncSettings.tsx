import { useState, useCallback } from 'react';
import { invokeOrThrow } from '../../services/invoke';
import { useConfigStore } from '../../stores/configStore';
import { useI18n } from '../../i18n';
import type { SyncConfig, SyncStatus } from '../../types';
import {
  Cloud,
  RefreshCw,
  Check,
  Save,
  AlertCircle,
  CheckCircle,
  Loader2,
  Eye,
  EyeOff,
  FolderOpen,
  Clock,
  FileText,
  BookOpen,
  Database,
  Settings as SettingsIcon,
} from 'lucide-react';

function SyncSettings() {
  const { config, saved, saveConfig, updateConfig } = useConfigStore();
  const { t } = useI18n();
  const sync = config.sync;

  const [testing, setTesting] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [syncResult, setSyncResult] = useState<SyncStatus | null>(null);
  const [showPassword, setShowPassword] = useState(false);

  const updateSync = useCallback(
    (updater: (prev: SyncConfig) => SyncConfig) => {
      updateConfig((prev) => ({
        ...prev,
        sync: updater(
          prev.sync || {
            enabled: false,
            serverUrl: '',
            username: '',
            password: '',
            remoteDir: 'moontranslator',
            intervalMins: 30,
            syncConfig: true,
            syncGlossary: true,
            syncHistory: true,
            syncWordbook: true,
            lastSyncAt: 0,
            lastSyncStatus: '',
          },
        ),
      }));
    },
    [updateConfig],
  );

  const handleTestConnection = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      // Save config first so backend uses latest settings
      await void saveConfig();
      const result = await invokeOrThrow<string>('test_webdav_connection');
      setTestResult({ success: true, message: result });
    } catch (err) {
      setTestResult({
        success: false,
        message: err instanceof Error ? err.message : String(err),
      });
    } finally {
      setTesting(false);
    }
  };

  const handleSyncNow = async () => {
    setSyncing(true);
    setSyncResult(null);
    try {
      // Save config first
      await void saveConfig();
      const result = await invokeOrThrow<SyncStatus>('sync_now');
      setSyncResult(result);
      // Reload config to get updated lastSyncAt
      // Config is automatically updated on backend
    } catch (err) {
      setSyncResult({
        success: false,
        message: err instanceof Error ? err.message : String(err),
        syncedAt: 0,
        uploaded: [],
        downloaded: [],
      });
    } finally {
      setSyncing(false);
    }
  };

  const formatTimestamp = (ts: number) => {
    if (!ts) return t('settings.sync.never');
    return new Date(ts).toLocaleString();
  };

  return (
    <section className="ui-card p-5 mb-5">
      <h2 className="text-base font-semibold text-primary mb-4 flex items-center gap-2">
        <Cloud size={18} />
        {t('settings.sync.title')}
      </h2>
      <p className="text-xs text-text-secondary mb-4">{t('settings.sync.description')}</p>

      <div className="space-y-4">
        {/* Enable Sync Toggle */}
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm text-text-primary font-medium">{t('settings.sync.enable')}</p>
            <p className="text-xs text-text-secondary mt-1">{t('settings.sync.enableHint')}</p>
          </div>
          <button
            className={`relative w-12 h-6 rounded-full transition-colors ${
              sync?.enabled ? 'bg-primary' : 'bg-bg-tertiary'
            }`}
            onClick={() => updateSync((prev) => ({ ...prev, enabled: !prev.enabled }))}
          >
            <div
              className={`absolute top-0.5 w-5 h-5 rounded-full shadow transition-transform ${
                sync?.enabled ? 'translate-x-6 bg-primary-fg' : 'translate-x-0.5 bg-text-secondary'
              }`}
            />
          </button>
        </div>

        {/* Server Settings */}
        {sync?.enabled && (
          <>
            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                {t('settings.sync.serverUrl')}
              </label>
              <input
                value={sync.serverUrl}
                onChange={(e) => updateSync((prev) => ({ ...prev, serverUrl: e.target.value }))}
                placeholder="https://dav.jianguoyun.com/dav/"
                className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
              />
              <p className="text-xs text-text-secondary mt-1">{t('settings.sync.serverUrlHint')}</p>
            </div>

            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                {t('settings.sync.username')}
              </label>
              <input
                value={sync.username}
                onChange={(e) => updateSync((prev) => ({ ...prev, username: e.target.value }))}
                placeholder="user@example.com"
                className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
              />
            </div>

            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                {t('settings.sync.password')}
              </label>
              <div className="relative">
                <input
                  type={showPassword ? 'text' : 'password'}
                  value={sync.password}
                  onChange={(e) => updateSync((prev) => ({ ...prev, password: e.target.value }))}
                  placeholder={t('settings.sync.passwordPlaceholder')}
                  className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 pr-10 text-sm focus:border-primary outline-none"
                />
                <button
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  className="absolute right-2 top-1/2 -translate-y-1/2 p-1.5 text-text-secondary hover:text-text-primary"
                >
                  {showPassword ? <EyeOff size={14} /> : <Eye size={14} />}
                </button>
              </div>
            </div>

            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                {t('settings.sync.remoteDir')}
              </label>
              <div className="flex items-center gap-2">
                <FolderOpen size={14} className="text-text-secondary" />
                <input
                  value={sync.remoteDir}
                  onChange={(e) => updateSync((prev) => ({ ...prev, remoteDir: e.target.value }))}
                  placeholder="moontranslator"
                  className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
                />
              </div>
            </div>

            <div>
              <label className="block text-xs text-text-secondary mb-1.5">
                {t('settings.sync.autoSyncInterval')}
              </label>
              <div className="flex items-center gap-3">
                <Clock size={14} className="text-text-secondary" />
                <input
                  type="number"
                  value={sync.intervalMins}
                  onChange={(e) =>
                    updateSync((prev) => ({
                      ...prev,
                      intervalMins: parseInt(e.target.value) || 30,
                    }))
                  }
                  min={0}
                  max={1440}
                  className="w-24 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
                />
                <span className="text-xs text-text-secondary">
                  {sync.intervalMins === 0
                    ? t('settings.sync.manualOnly')
                    : t('settings.sync.everyNmin', { mins: sync.intervalMins })}
                </span>
              </div>
            </div>

            {/* Sync Items */}
            <div className="border-t border-border pt-4">
              <p className="text-xs text-text-secondary mb-3 font-medium">
                {t('settings.sync.itemsToSync')}
              </p>
              <div className="space-y-2">
                <label className="flex items-center gap-3 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={sync.syncConfig}
                    onChange={(e) =>
                      updateSync((prev) => ({ ...prev, syncConfig: e.target.checked }))
                    }
                    className="w-4 h-4 rounded border-border accent-primary"
                  />
                  <SettingsIcon size={14} className="text-text-secondary" />
                  <span className="text-sm text-text-primary">
                    {t('settings.sync.syncConfigItem')}
                  </span>
                </label>
                <label className="flex items-center gap-3 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={sync.syncGlossary}
                    onChange={(e) =>
                      updateSync((prev) => ({ ...prev, syncGlossary: e.target.checked }))
                    }
                    className="w-4 h-4 rounded border-border accent-primary"
                  />
                  <BookOpen size={14} className="text-text-secondary" />
                  <span className="text-sm text-text-primary">
                    {t('settings.sync.syncGlossaryItem')}
                  </span>
                </label>
                <label className="flex items-center gap-3 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={sync.syncHistory}
                    onChange={(e) =>
                      updateSync((prev) => ({ ...prev, syncHistory: e.target.checked }))
                    }
                    className="w-4 h-4 rounded border-border accent-primary"
                  />
                  <Database size={14} className="text-text-secondary" />
                  <span className="text-sm text-text-primary">
                    {t('settings.sync.syncHistoryItem')}
                  </span>
                </label>
                <label className="flex items-center gap-3 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={sync.syncWordbook}
                    onChange={(e) =>
                      updateSync((prev) => ({ ...prev, syncWordbook: e.target.checked }))
                    }
                    className="w-4 h-4 rounded border-border accent-primary"
                  />
                  <FileText size={14} className="text-text-secondary" />
                  <span className="text-sm text-text-primary">
                    {t('settings.sync.syncWordbookItem')}
                  </span>
                </label>
              </div>
            </div>

            {/* Connection Test */}
            <div className="border-t border-border pt-4">
              <div className="flex gap-2">
                <button
                  onClick={handleTestConnection}
                  disabled={testing || !sync.serverUrl}
                  className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-4 py-2 text-sm hover:bg-bg-tertiary/80 transition-colors flex items-center gap-2 disabled:opacity-50"
                >
                  {testing ? (
                    <Loader2 size={14} className="animate-spin" />
                  ) : (
                    <RefreshCw size={14} />
                  )}
                  {t('settings.sync.testConnection')}
                </button>
              </div>

              {testResult && (
                <div
                  className={`mt-3 p-3 rounded-lg flex items-start gap-2 text-xs ${
                    testResult.success
                      ? 'bg-green-500/10 border border-green-500/20 text-green-400'
                      : 'bg-red-500/10 border border-red-500/20 text-red-400'
                  }`}
                >
                  {testResult.success ? <CheckCircle size={14} /> : <AlertCircle size={14} />}
                  <span>{testResult.message}</span>
                </div>
              )}
            </div>

            {/* Manual Sync */}
            <div className="border-t border-border pt-4">
              <div className="flex items-center justify-between mb-3">
                <div>
                  <p className="text-sm text-text-primary font-medium">
                    {t('settings.sync.manualSync')}
                  </p>
                  <p className="text-xs text-text-secondary mt-1">
                    {t('settings.sync.lastSynced', { time: formatTimestamp(sync.lastSyncAt) })}
                  </p>
                </div>
                <button
                  onClick={handleSyncNow}
                  disabled={syncing || !sync.serverUrl}
                  className="bg-primary text-bg-primary font-semibold rounded-lg px-4 py-2 text-sm hover:bg-primary-hover transition-colors flex items-center gap-2 disabled:opacity-50"
                >
                  {syncing ? <Loader2 size={14} className="animate-spin" /> : <Cloud size={14} />}
                  {syncing ? t('settings.sync.syncing') : t('settings.sync.syncNow')}
                </button>
              </div>

              {sync.lastSyncStatus && (
                <p className="text-xs text-text-secondary mb-2">{sync.lastSyncStatus}</p>
              )}

              {syncResult && (
                <div
                  className={`p-3 rounded-lg text-xs ${
                    syncResult.success
                      ? 'bg-green-500/10 border border-green-500/20 text-green-400'
                      : 'bg-red-500/10 border border-red-500/20 text-red-400'
                  }`}
                >
                  <p className="font-medium mb-1">{syncResult.message}</p>
                  {syncResult.uploaded.length > 0 && (
                    <p className="text-text-secondary">
                      {t('settings.sync.uploaded', { files: syncResult.uploaded.join(', ') })}
                    </p>
                  )}
                  {syncResult.downloaded.length > 0 && (
                    <p className="text-text-secondary">
                      {t('settings.sync.downloaded', { files: syncResult.downloaded.join(', ') })}
                    </p>
                  )}
                </div>
              )}
            </div>
          </>
        )}
      </div>

      {/* Save Button */}
      <div className="flex justify-center mt-5">
        <button
          className="bg-primary text-bg-primary font-semibold rounded-lg px-8 py-2.5 text-sm hover:bg-primary-hover transition-colors flex items-center gap-2"
          onClick={saveConfig}
        >
          {saved ? (
            <>
              <Check size={16} />
              {t('settings.saved')}
            </>
          ) : (
            <>
              <Save size={16} />
              {t('settings.save')}
            </>
          )}
        </button>
      </div>
    </section>
  );
}

export default SyncSettings;
