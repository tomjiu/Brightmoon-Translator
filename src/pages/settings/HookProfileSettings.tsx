// HookProfileSettings - Hook 配置文件管理（Phase 3.1）
import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Plus, Trash2, Check, Monitor, Clock, Edit3 } from 'lucide-react';
import Card from '../../components/Card';

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

const DEFAULT_HOOK_CONFIG: HookConfig = {
  enabledSources: ['uia'],
  showOverlay: true,
  autoCopy: false,
  enabled: true,
  uiaIntervalMs: 500,
  ocrIntervalMs: 5000,
};

export default function HookProfileSettings() {
  const [profiles, setProfiles] = useState<HookProfile[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);

  // Create form
  const [newName, setNewName] = useState('');
  const [newProcessName, setNewProcessName] = useState('');
  const [newWindowTitle, setNewWindowTitle] = useState('');
  const [newNotes, setNewNotes] = useState('');
  const [newSourceLang, setNewSourceLang] = useState('');
  const [newTargetLang, setNewTargetLang] = useState('');

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

  const handleCreate = async () => {
    if (!newName.trim()) return;
    try {
      await invoke('create_hook_profile', {
        name: newName.trim(),
        hookConfig: DEFAULT_HOOK_CONFIG,
      });
      // Update optional fields
      const allProfiles = await invoke<HookProfile[]>('get_hook_profiles');
      const created = allProfiles[allProfiles.length - 1];
      if (created) {
        const updates: HookProfileUpdate = {};
        if (newProcessName) updates.processName = newProcessName;
        if (newWindowTitle) updates.windowTitlePattern = newWindowTitle;
        if (newNotes) updates.notes = newNotes;
        if (newSourceLang) updates.sourceLang = newSourceLang;
        if (newTargetLang) updates.targetLang = newTargetLang;
        if (Object.keys(updates).length > 0) {
          await invoke('update_hook_profile', { id: created.id, updates });
        }
      }
      resetForm();
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

  const handleUpdate = async (id: string, updates: HookProfileUpdate) => {
    try {
      await invoke('update_hook_profile', { id, updates });
      setEditingId(null);
      await loadProfiles();
    } catch (err) {
      console.error('Failed to update profile:', err);
    }
  };

  const resetForm = () => {
    setNewName('');
    setNewProcessName('');
    setNewWindowTitle('');
    setNewNotes('');
    setNewSourceLang('');
    setNewTargetLang('');
    setShowCreateForm(false);
  };

  const formatTime = (ts?: number) => {
    if (!ts) return '从未';
    return new Date(ts * 1000).toLocaleString('zh-CN');
  };

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">Hook 配置文件</h1>
        <p className="ui-page-desc">为不同游戏/应用保存独立的钩取和翻译配置（Phase 3.1）</p>
      </div>

      {/* Active Profile Indicator */}
      {activeId && (
        <div className="flex items-center gap-2 px-3 py-2 bg-primary/10 border border-primary/20 rounded-lg">
          <Check size={16} className="text-primary" />
          <span className="text-sm text-primary font-medium">
            当前激活: {profiles.find((p) => p.id === activeId)?.name ?? '未知'}
          </span>
          <button
            onClick={() => void handleActivate(null)}
            className="ml-auto text-xs text-text-secondary hover:text-text-primary"
          >
            取消激活
          </button>
        </div>
      )}

      {/* Profile List */}
      <Card title={`配置文件 (${profiles.length})`}>
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
              onSaveEdit={(updates) => void handleUpdate(profile.id, updates)}
              formatTime={formatTime}
            />
          ))}

          {profiles.length === 0 && (
            <p className="text-sm text-text-tertiary text-center py-6">
              暂无配置文件。为不同游戏创建独立配置，切换时自动应用。
            </p>
          )}

          {/* Create Form */}
          {showCreateForm ? (
            <div className="p-4 border border-primary/30 rounded-lg bg-bg-primary space-y-3">
              <h4 className="text-sm font-medium text-text-primary">新建配置文件</h4>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs text-text-secondary mb-1">名称 *</label>
                  <input
                    type="text"
                    value={newName}
                    onChange={(e) => setNewName(e.target.value)}
                    placeholder="如: 星露谷物语"
                    className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
                  />
                </div>
                <div>
                  <label className="block text-xs text-text-secondary mb-1">进程名</label>
                  <input
                    type="text"
                    value={newProcessName}
                    onChange={(e) => setNewProcessName(e.target.value)}
                    placeholder="如: game.exe"
                    className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
                  />
                </div>
              </div>
              <div>
                <label className="block text-xs text-text-secondary mb-1">窗口标题关键词</label>
                <input
                  type="text"
                  value={newWindowTitle}
                  onChange={(e) => setNewWindowTitle(e.target.value)}
                  placeholder="如: Stardew Valley"
                  className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
                />
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs text-text-secondary mb-1">源语言</label>
                  <input
                    type="text"
                    value={newSourceLang}
                    onChange={(e) => setNewSourceLang(e.target.value)}
                    placeholder="如: ja"
                    className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
                  />
                </div>
                <div>
                  <label className="block text-xs text-text-secondary mb-1">目标语言</label>
                  <input
                    type="text"
                    value={newTargetLang}
                    onChange={(e) => setNewTargetLang(e.target.value)}
                    placeholder="如: zh"
                    className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
                  />
                </div>
              </div>
              <div>
                <label className="block text-xs text-text-secondary mb-1">备注</label>
                <input
                  type="text"
                  value={newNotes}
                  onChange={(e) => setNewNotes(e.target.value)}
                  placeholder="可选备注..."
                  className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
                />
              </div>
              <div className="flex justify-end gap-2">
                <button
                  onClick={resetForm}
                  className="px-3 py-1.5 text-sm text-text-secondary hover:text-text-primary"
                >
                  取消
                </button>
                <button
                  onClick={() => void handleCreate()}
                  disabled={!newName.trim()}
                  className="px-4 py-1.5 text-sm bg-primary text-primary-fg rounded hover:bg-primary/90 disabled:opacity-50"
                >
                  创建
                </button>
              </div>
            </div>
          ) : (
            <button
              onClick={() => setShowCreateForm(true)}
              className="w-full flex items-center justify-center gap-2 py-2.5 border border-dashed border-border rounded-lg text-text-secondary hover:text-primary hover:border-primary transition-colors"
            >
              <Plus size={16} />
              <span className="text-sm">新建配置文件</span>
            </button>
          )}
        </div>
      </Card>

      {/* Usage Tips */}
      <Card title="使用说明">
        <div className="text-xs text-text-secondary space-y-1.5">
          <p>
            • 创建配置文件后，点击 <strong>激活</strong> 使其生效
          </p>
          <p>
            • 设置 <strong>进程名</strong> 或 <strong>窗口标题</strong> 后，匹配到对应游戏时自动切换
          </p>
          <p>• 每个配置文件可以独立设置源语言/目标语言</p>
          <p>
            • 配置文件保存在{' '}
            <code className="bg-bg-tertiary px-1 rounded">
              ~/.config/moontranslator/hook_profiles.json
            </code>
          </p>
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
  onSaveEdit: (updates: HookProfileUpdate) => void;
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
  const [editName, setEditName] = useState(profile.name);
  const [editProcess, setEditProcess] = useState(profile.processName ?? '');
  const [editTitle, setEditTitle] = useState(profile.windowTitlePattern ?? '');
  const [editNotes, setEditNotes] = useState(profile.notes);
  const [editSrc, setEditSrc] = useState(profile.sourceLang ?? '');
  const [editTgt, setEditTgt] = useState(profile.targetLang ?? '');

  if (isEditing) {
    return (
      <div className="p-4 border border-primary/30 rounded-lg bg-bg-primary space-y-3">
        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="block text-xs text-text-secondary mb-1">名称</label>
            <input
              type="text"
              value={editName}
              onChange={(e) => setEditName(e.target.value)}
              className="w-full px-3 py-1.5 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
            />
          </div>
          <div>
            <label className="block text-xs text-text-secondary mb-1">进程名</label>
            <input
              type="text"
              value={editProcess}
              onChange={(e) => setEditProcess(e.target.value)}
              className="w-full px-3 py-1.5 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
            />
          </div>
        </div>
        <div>
          <label className="block text-xs text-text-secondary mb-1">窗口标题关键词</label>
          <input
            type="text"
            value={editTitle}
            onChange={(e) => setEditTitle(e.target.value)}
            className="w-full px-3 py-1.5 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
          />
        </div>
        <div className="grid grid-cols-3 gap-3">
          <div>
            <label className="block text-xs text-text-secondary mb-1">源语言</label>
            <input
              type="text"
              value={editSrc}
              onChange={(e) => setEditSrc(e.target.value)}
              className="w-full px-3 py-1.5 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
            />
          </div>
          <div>
            <label className="block text-xs text-text-secondary mb-1">目标语言</label>
            <input
              type="text"
              value={editTgt}
              onChange={(e) => setEditTgt(e.target.value)}
              className="w-full px-3 py-1.5 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
            />
          </div>
          <div>
            <label className="block text-xs text-text-secondary mb-1">备注</label>
            <input
              type="text"
              value={editNotes}
              onChange={(e) => setEditNotes(e.target.value)}
              className="w-full px-3 py-1.5 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
            />
          </div>
        </div>
        <div className="flex justify-end gap-2">
          <button
            onClick={onCancelEdit}
            className="px-3 py-1.5 text-sm text-text-secondary hover:text-text-primary"
          >
            取消
          </button>
          <button
            onClick={() =>
              onSaveEdit({
                name: editName,
                processName: editProcess || undefined,
                windowTitlePattern: editTitle || undefined,
                notes: editNotes,
                sourceLang: editSrc || undefined,
                targetLang: editTgt || undefined,
              })
            }
            className="px-4 py-1.5 text-sm bg-primary text-primary-fg rounded hover:bg-primary/90"
          >
            保存
          </button>
        </div>
      </div>
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
            <span className="text-xs px-1.5 py-0.5 bg-primary/20 text-primary rounded">激活中</span>
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
            <p className="text-xs text-text-secondary">窗口: {profile.windowTitlePattern}</p>
          )}
          {(profile.sourceLang || profile.targetLang) && (
            <p className="text-xs text-text-secondary">
              {profile.sourceLang ?? '?'} → {profile.targetLang ?? '?'}
            </p>
          )}
          <div className="flex items-center gap-1.5 text-xs text-text-tertiary">
            <Clock size={12} />
            <span>上次使用: {formatTime(profile.lastUsed)}</span>
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
            激活
          </button>
        )}
        <button
          onClick={onStartEdit}
          className="p-1.5 text-text-tertiary hover:text-primary rounded"
          title="编辑"
        >
          <Edit3 size={14} />
        </button>
        <button
          onClick={onDelete}
          className="p-1.5 text-text-tertiary hover:text-red-500 rounded"
          title="删除"
        >
          <Trash2 size={14} />
        </button>
      </div>
    </div>
  );
}
