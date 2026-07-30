// HookProfileSettings - Hook profile management (Phase 3.1)
import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Plus, Trash2, Check, Monitor, Clock, Edit3, X } from 'lucide-react';
import Card from '../../components/Card';
import { useI18n } from '../../i18n';

interface HookConfig {
  enabledSources: string[];
  showOverlay: boolean;
  autoCopy: boolean;
  enabled: boolean;
  uiaIntervalMs: number;
  ocrIntervalMs: number;
}

interface HookProfile {
  id: string;
  name: string;
  processName?: string;
  windowTitlePattern?: string;
  hookConfig: HookConfig;
  sourceLang?: string;
  targetLang?: string;
  notes: string;
  createdAt: number;
  lastUsed?: number;
}

interface HookProfileUpdate {
  name?: string;
  processName?: string;
  windowTitlePattern?: string;
  hookConfig?: HookConfig;
  sourceLang?: string;
  targetLang?: string;
  notes?: string;
}

interface HookProfileFormData {
  name: string;
  processName: string;
  windowTitle: string;
  sourceLang: string;
  targetLang: string;
  notes: string;
}

const EMPTY_FORM: HookProfileFormData = {
  name: '',
  processName: '',
  windowTitle: '',
  sourceLang: '',
  targetLang: '',
  notes: '',
};

const DEFAULT_HOOK_CONFIG: HookConfig = {
  enabledSources: ['uia'],
  showOverlay: true,
  autoCopy: false,
  enabled: true,
  uiaIntervalMs: 500,
  ocrIntervalMs: 5000,
};

const LANG_OPTIONS = [
  { value: '', label: 'Auto' },
  { value: 'en', label: 'English' },
  { value: 'zh', label: '中文' },
  { value: 'ja', label: '日本語' },
  { value: 'ko', label: '한국어' },
  { value: 'fr', label: 'Français' },
  { value: 'de', label: 'Deutsch' },
  { value: 'es', label: 'Español' },
  { value: 'ru', label: 'Русский' },
];

// ── Reusable profile form (create & edit) ──

function HookProfileForm({
  initial,
  onSave,
  onCancel,
  saveLabel,
}: {
  initial: HookProfileFormData;
  onSave: (data: HookProfileFormData) => void;
  onCancel: () => void;
  saveLabel: string;
}) {
  const { t } = useI18n();
  const [form, setForm] = useState<HookProfileFormData>(initial);

  return (
    <div className="p-4 border border-primary/30 rounded-lg bg-bg-primary space-y-3">
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="block text-xs text-text-secondary mb-1">
            {t('settings.hookProfile.name')} *
          </label>
          <input
            type="text"
            value={form.name}
            onChange={(e) => setForm({ ...form, name: e.target.value })}
            placeholder="如: 星露谷物语"
            className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
          />
        </div>
        <div>
          <label className="block text-xs text-text-secondary mb-1">
            {t('settings.hookProfile.process')}
          </label>
          <input
            type="text"
            value={form.processName}
            onChange={(e) => setForm({ ...form, processName: e.target.value })}
            placeholder="如: game.exe"
            className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
          />
        </div>
      </div>
      <div>
        <label className="block text-xs text-text-secondary mb-1">
          {t('settings.hookProfile.window')}
        </label>
        <input
          type="text"
          value={form.windowTitle}
          onChange={(e) => setForm({ ...form, windowTitle: e.target.value })}
          placeholder="Stardew Valley"
          className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
        />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="block text-xs text-text-secondary mb-1">
            {t('settings.hookProfile.sourceLang')}
          </label>
          <select
            value={form.sourceLang}
            onChange={(e) => setForm({ ...form, sourceLang: e.target.value })}
            className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none cursor-pointer"
          >
            {LANG_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className="block text-xs text-text-secondary mb-1">
            {t('settings.hookProfile.targetLang')}
          </label>
          <select
            value={form.targetLang}
            onChange={(e) => setForm({ ...form, targetLang: e.target.value })}
            className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none cursor-pointer"
          >
            {LANG_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>
      </div>
      <div>
        <label className="block text-xs text-text-secondary mb-1">
          {t('settings.hookProfile.notes')}
        </label>
        <input
          type="text"
          value={form.notes}
          onChange={(e) => setForm({ ...form, notes: e.target.value })}
          className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
        />
      </div>
      <div className="flex justify-end gap-2">
        <button
          onClick={onCancel}
          className="px-3 py-1.5 text-sm text-text-secondary hover:text-text-primary flex items-center gap-1"
        >
          <X size={14} />
          {t('settings.hookProfile.cancel')}
        </button>
        <button
          onClick={() => onSave(form)}
          disabled={!form.name.trim()}
          className="px-4 py-1.5 text-sm bg-primary text-primary-fg rounded hover:bg-primary/90 disabled:opacity-50 flex items-center gap-1"
        >
          <Check size={14} />
          {saveLabel}
        </button>
      </div>
    </div>
  );
}

export default function HookProfileSettings() {
  const { t } = useI18n();
  const [profiles, setProfiles] = useState<HookProfile[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);

  const loadProfiles = useCallback(async () => {
    try {
      const [allProfiles, active] = await Promise.all([
        invoke<HookProfile[]>('get_hook_profiles'),
        invoke<HookProfile | null>('get_active_hook_profile'),
      ]);
      setProfiles(allProfiles);
      setActiveId(active?.id ?? null);
    } catch (err) {
      console.error('Failed to load hook profiles:', err);
    }
  }, []);

  useEffect(() => {
    void loadProfiles();
  }, [loadProfiles]);

  const handleCreate = async (data: HookProfileFormData) => {
    try {
      await invoke('create_hook_profile', {
        name: data.name.trim(),
        hookConfig: DEFAULT_HOOK_CONFIG,
      });
      const allProfiles = await invoke<HookProfile[]>('get_hook_profiles');
      const created = allProfiles[allProfiles.length - 1];
      if (created) {
        const updates: HookProfileUpdate = {};
        if (data.processName) updates.processName = data.processName;
        if (data.windowTitle) updates.windowTitlePattern = data.windowTitle;
        if (data.notes) updates.notes = data.notes;
        if (data.sourceLang) updates.sourceLang = data.sourceLang;
        if (data.targetLang) updates.targetLang = data.targetLang;
        if (Object.keys(updates).length > 0) {
          await invoke('update_hook_profile', { id: created.id, updates });
        }
      }
      setShowCreateForm(false);
      await loadProfiles();
    } catch (err) {
      console.error('Failed to create profile:', err);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await invoke('delete_hook_profile', { id });
      await loadProfiles();
    } catch (err) {
      console.error('Failed to delete profile:', err);
    }
  };

  const handleActivate = async (id: string | null) => {
    try {
      await invoke('activate_hook_profile', { id });
      await loadProfiles();
    } catch (err) {
      console.error('Failed to activate profile:', err);
    }
  };

  const handleUpdate = async (id: string, data: HookProfileFormData) => {
    try {
      await invoke('update_hook_profile', {
        id,
        updates: {
          name: data.name,
          processName: data.processName || undefined,
          windowTitlePattern: data.windowTitle || undefined,
          notes: data.notes,
          sourceLang: data.sourceLang || undefined,
          targetLang: data.targetLang || undefined,
        },
      });
      setEditingId(null);
      await loadProfiles();
    } catch (err) {
      console.error('Failed to update profile:', err);
    }
  };

  const formatTime = (ts?: number) => {
    if (!ts) return t('settings.hookProfile.never');
    return new Date(ts * 1000).toLocaleString('zh-CN');
  };

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">{t('settings.hookProfile.pageTitle')}</h1>
        <p className="ui-page-desc">{t('settings.hookProfile.pageDesc')}</p>
      </div>

      {/* Active Profile Indicator */}
      {activeId && (
        <div className="flex items-center gap-2 px-3 py-2 bg-primary/10 border border-primary/20 rounded-lg">
          <Check size={16} className="text-primary" />
          <span className="text-sm text-primary font-medium">
            {t('settings.hookProfile.activeLabel', {
              name: profiles.find((p) => p.id === activeId)?.name ?? t('settings.hookProfile.unknown'),
            })}
          </span>
          <button
            onClick={() => void handleActivate(null)}
            className="ml-auto text-xs text-text-secondary hover:text-text-primary"
          >
            {t('settings.hookProfile.deactivate')}
          </button>
        </div>
      )}

      {/* Profile List */}
      <Card title={t('settings.hookProfile.listTitle', { count: profiles.length })}>
        <div className="space-y-3">
          {profiles.map((profile) => (
            <ProfileCard
              key={profile.id}
              profile={profile}
              isActive={profile.id === activeId}
              isEditing={editingId === profile.id}
              onActivate={() => void handleActivate(profile.id)}
              onDelete={() => void handleDelete(profile.id)}
              onStartEdit={() => setEditingId(profile.id)}
              onCancelEdit={() => setEditingId(null)}
              onSaveEdit={(data) => void handleUpdate(profile.id, data)}
              formatTime={formatTime}
            />
          ))}

          {profiles.length === 0 && (
            <p className="text-sm text-text-tertiary text-center py-6">
              {t('settings.hookProfile.empty')}
            </p>
          )}

          {/* Create Form */}
          {showCreateForm ? (
            <HookProfileForm
              initial={EMPTY_FORM}
              onSave={handleCreate}
              onCancel={() => setShowCreateForm(false)}
              saveLabel={t('settings.hookProfile.createBtn')}
            />
          ) : (
            <button
              onClick={() => setShowCreateForm(true)}
              className="w-full flex items-center justify-center gap-2 py-2.5 border border-dashed border-border rounded-lg text-text-secondary hover:text-primary hover:border-primary transition-colors"
            >
              <Plus size={16} />
              <span className="text-sm">{t('settings.hookProfile.create')}</span>
            </button>
          )}
        </div>
      </Card>

      {/* Usage Tips */}
      <Card title={t('settings.hookProfile.tipsTitle')}>
        <div className="text-xs text-text-secondary space-y-1.5">
          <p>• {t('settings.hookProfile.tip1')}</p>
          <p>• {t('settings.hookProfile.tip2')}</p>
          <p>• {t('settings.hookProfile.tip3')}</p>
          <p>• {t('settings.hookProfile.tip4')}</p>
        </div>
      </Card>
    </div>
  );
}

interface ProfileCardProps {
  profile: HookProfile;
  isActive: boolean;
  isEditing: boolean;
  onActivate: () => void;
  onDelete: () => void;
  onStartEdit: () => void;
  onCancelEdit: () => void;
  onSaveEdit: (data: HookProfileFormData) => void;
  formatTime: (ts?: number) => string;
}

function ProfileCard({
  profile,
  isActive,
  isEditing,
  onActivate,
  onDelete,
  onStartEdit,
  onCancelEdit,
  onSaveEdit,
  formatTime,
}: ProfileCardProps) {
  const { t } = useI18n();

  if (isEditing) {
    return (
      <HookProfileForm
        initial={{
          name: profile.name,
          processName: profile.processName ?? '',
          windowTitle: profile.windowTitlePattern ?? '',
          sourceLang: profile.sourceLang ?? '',
          targetLang: profile.targetLang ?? '',
          notes: profile.notes,
        }}
        onSave={onSaveEdit}
        onCancel={onCancelEdit}
        saveLabel={t('settings.hookProfile.save')}
      />
    );
  }

  return (
    <div
      className={`flex items-start gap-3 p-3 rounded-lg border ${
        isActive
          ? 'bg-primary/5 border-primary/30'
          : 'bg-bg-primary border-border hover:border-border/80'
      }`}
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <h4 className="text-sm font-medium text-text-primary">{profile.name}</h4>
          {isActive && (
            <span className="text-xs px-1.5 py-0.5 bg-primary/20 text-primary rounded">
              {t('settings.hookProfile.activating')}
            </span>
          )}
        </div>

        <div className="mt-1 space-y-0.5">
          {profile.processName && (
            <div className="flex items-center gap-1.5 text-xs text-text-secondary">
              <Monitor size={12} />
              <span>{profile.processName}</span>
            </div>
          )}
          {profile.windowTitlePattern && (
            <p className="text-xs text-text-secondary">
              {t('settings.hookProfile.windowLabel', { title: profile.windowTitlePattern })}
            </p>
          )}
          {(profile.sourceLang || profile.targetLang) && (
            <p className="text-xs text-text-secondary">
              {profile.sourceLang ?? '?'} → {profile.targetLang ?? '?'}
            </p>
          )}
          <div className="flex items-center gap-1.5 text-xs text-text-tertiary">
            <Clock size={12} />
            <span>
              {t('settings.hookProfile.lastUsed', { time: formatTime(profile.lastUsed) })}
            </span>
          </div>
          {profile.notes && <p className="text-xs text-text-tertiary italic">{profile.notes}</p>}
        </div>
      </div>

      <div className="flex items-center gap-1 shrink-0">
        {!isActive && (
          <button
            onClick={onActivate}
            className="px-2 py-1 text-xs bg-primary/10 text-primary rounded hover:bg-primary/20"
          >
            {t('settings.hookProfile.activate')}
          </button>
        )}
        <button
          onClick={onStartEdit}
          className="p-1.5 text-text-tertiary hover:text-primary rounded"
          title={t('settings.hookProfile.edit')}
        >
          <Edit3 size={14} />
        </button>
        <button
          onClick={onDelete}
          className="p-1.5 text-text-tertiary hover:text-red-500 rounded"
          title={t('settings.hookProfile.delete')}
        >
          <Trash2 size={14} />
        </button>
      </div>
    </div>
  );
}
