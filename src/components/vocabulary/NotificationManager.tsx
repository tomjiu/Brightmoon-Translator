import { useEffect, useState, type FC } from 'react';
import { Bell, BellOff, Clock, Target, Trophy } from 'lucide-react';
import {
  checkDailyReminder,
  checkDueCardsReminder,
  checkMilestoneCelebration,
  sendDesktopNotification,
  type NotificationSettings,
} from '../../services/notification';
import PageHeader from '../PageHeader';
import Switch from '../Switch';

const DEFAULTS: NotificationSettings = {
  enabled: false,
  dailyReminderTime: '09:00',
  dueCardsThreshold: 20,
  milestoneEnabled: true,
};

export const NotificationManager: FC = () => {
  const [settings, setSettings] = useState<NotificationSettings>(DEFAULTS);
  const [testMsg, setTestMsg] = useState<string | null>(null);

  useEffect(() => {
    try {
      const raw = localStorage.getItem('notificationSettings');
      if (raw) {
        const parsed = JSON.parse(raw) as Partial<NotificationSettings>;
        setSettings({ ...DEFAULTS, ...parsed });
      }
    } catch {
      /* ignore corrupt localStorage */
    }
  }, []);

  useEffect(() => {
    if (!settings.enabled) return;

    const dailyReminderInterval = setInterval(() => {
      const now = new Date();
      const [targetHour, targetMinute] = settings.dailyReminderTime.split(':').map(Number);
      if (now.getHours() === targetHour && now.getMinutes() === targetMinute) {
        void checkDailyReminder().catch(() => undefined);
      }
    }, 60_000);

    const dueCardsInterval = setInterval(() => {
      void checkDueCardsReminder(settings.dueCardsThreshold).catch(() => undefined);
    }, 1_800_000);

    const milestoneInterval = setInterval(() => {
      if (settings.milestoneEnabled) {
        void checkMilestoneCelebration().catch(() => undefined);
      }
    }, 3_600_000);

    return () => {
      clearInterval(dailyReminderInterval);
      clearInterval(dueCardsInterval);
      clearInterval(milestoneInterval);
    };
  }, [settings]);

  const saveSettings = (next: NotificationSettings) => {
    setSettings(next);
    localStorage.setItem('notificationSettings', JSON.stringify(next));
  };

  const testNotification = async () => {
    setTestMsg(null);
    try {
      await sendDesktopNotification('学习提醒', '通知已开启，到点会提醒你复习。');
      setTestMsg('已发送测试通知（若未弹出，请检查系统通知权限）');
    } catch {
      setTestMsg('发送失败，请检查系统是否允许本应用通知');
    }
  };

  return (
    <div className="space-y-5 animate-fadeIn">
      <PageHeader
        title="学习提醒"
        description="到点提醒复习，不打扰翻译主流程。默认关闭。"
        icon={Bell}
        actions={
          <button
            type="button"
            onClick={() => void testNotification()}
            className="px-3 py-1.5 text-sm border border-border rounded-lg hover:bg-bg-tertiary"
          >
            测试通知
          </button>
        }
      />

      {testMsg && <p className="ui-caption">{testMsg}</p>}

      <section className="ui-surface p-4 space-y-3">
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2 min-w-0">
            {settings.enabled ? (
              <Bell className="w-4 h-4 text-primary shrink-0" />
            ) : (
              <BellOff className="w-4 h-4 text-text-secondary shrink-0" />
            )}
            <div className="min-w-0">
              <h3 className="ui-section-title">启用提醒</h3>
              <p className="ui-caption">关闭后不再发送任何学习相关通知</p>
            </div>
          </div>
          <Switch
            checked={settings.enabled}
            onChange={(enabled) => saveSettings({ ...settings, enabled })}
          />
        </div>
      </section>

      <section className={`ui-surface p-4 space-y-3 ${!settings.enabled ? 'opacity-50' : ''}`}>
        <div className="flex items-center gap-2">
          <Clock className="w-4 h-4 text-text-secondary" />
          <h3 className="ui-section-title">每日提醒</h3>
        </div>
        <p className="ui-caption">在设定时刻提醒开始学习（应用需在运行中）</p>
        <div className="flex items-center gap-3">
          <label className="ui-caption">时间</label>
          <input
            type="time"
            value={settings.dailyReminderTime}
            disabled={!settings.enabled}
            onChange={(e) => saveSettings({ ...settings, dailyReminderTime: e.target.value })}
            className="px-3 py-1.5 bg-bg-primary rounded-lg border border-border text-sm disabled:opacity-50"
          />
        </div>
      </section>

      <section className={`ui-surface p-4 space-y-3 ${!settings.enabled ? 'opacity-50' : ''}`}>
        <div className="flex items-center gap-2">
          <Target className="w-4 h-4 text-text-secondary" />
          <h3 className="ui-section-title">待复习数量</h3>
        </div>
        <p className="ui-caption">待复习词条达到该数量时提醒（约每 30 分钟检查）</p>
        <div className="flex items-center gap-3">
          <input
            type="number"
            min={1}
            max={200}
            disabled={!settings.enabled}
            value={settings.dueCardsThreshold}
            onChange={(e) =>
              saveSettings({
                ...settings,
                dueCardsThreshold: Math.min(200, Math.max(1, parseInt(e.target.value, 10) || 20)),
              })
            }
            className="w-20 px-3 py-1.5 bg-bg-primary rounded-lg border border-border text-sm disabled:opacity-50"
          />
          <span className="ui-caption">条</span>
        </div>
      </section>

      <section className={`ui-surface p-4 ${!settings.enabled ? 'opacity-50' : ''}`}>
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2 min-w-0">
            <Trophy className="w-4 h-4 text-text-secondary shrink-0" />
            <div>
              <h3 className="ui-section-title">里程碑</h3>
              <p className="ui-caption">连续学习达到 3 / 7 / 30 / 100 天时通知</p>
            </div>
          </div>
          <Switch
            checked={settings.milestoneEnabled}
            disabled={!settings.enabled}
            onChange={(milestoneEnabled) => saveSettings({ ...settings, milestoneEnabled })}
          />
        </div>
      </section>
    </div>
  );
};
