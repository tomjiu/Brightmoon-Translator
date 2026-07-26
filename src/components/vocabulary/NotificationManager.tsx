import { useEffect, useState, type FC } from 'react';
import { Bell, BellOff, Clock, Target, Trophy, CheckCircle } from 'lucide-react';
import {
  checkDailyReminder,
  checkDueCardsReminder,
  checkMilestoneCelebration,
  sendDesktopNotification,
  type NotificationSettings,
} from '../../services/notification';

export const NotificationManager: FC = () => {
  const [settings, setSettings] = useState<NotificationSettings>({
    enabled: true,
    dailyReminderTime: '09:00',
    dueCardsThreshold: 20,
    milestoneEnabled: true,
  });

  useEffect(() => {
    // 加载设置
    const savedSettings = localStorage.getItem('notificationSettings');
    if (savedSettings) {
      setSettings(JSON.parse(savedSettings));
    }
  }, []);

  useEffect(() => {
    if (!settings.enabled) return;

    // 每日学习提醒（每小时检查一次）
    const dailyReminderInterval = setInterval(() => {
      const now = new Date();
      const [targetHour, targetMinute] = settings.dailyReminderTime.split(':').map(Number);

      if (now.getHours() === targetHour && now.getMinutes() === targetMinute) {
        checkDailyReminder().catch((err: unknown) => {
          console.error(err);
        });
      }
    }, 60000); // 每分钟检查一次

    // 待复习卡牌提醒（每30分钟检查一次）
    const dueCardsInterval = setInterval(() => {
      checkDueCardsReminder(settings.dueCardsThreshold).catch((err: unknown) => {
        console.error(err);
      });
    }, 1800000); // 30分钟

    // 里程碑庆祝（每天检查一次，在学习后）
    const milestoneInterval = setInterval(() => {
      if (settings.milestoneEnabled) {
        checkMilestoneCelebration().catch((err: unknown) => {
          console.error(err);
        });
      }
    }, 3600000); // 1小时

    return () => {
      clearInterval(dailyReminderInterval);
      clearInterval(dueCardsInterval);
      clearInterval(milestoneInterval);
    };
  }, [settings]);

  const saveSettings = (newSettings: NotificationSettings) => {
    setSettings(newSettings);
    localStorage.setItem('notificationSettings', JSON.stringify(newSettings));
  };

  const testNotification = async () => {
    try {
      await sendDesktopNotification('🎉 测试通知', '通知功能正常！你将收到学习提醒和里程碑庆祝 ✨');
    } catch (error) {
      console.error('发送测试通知失败:', error);
    }
  };

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold flex items-center gap-2">
          <Bell className="w-7 h-7" />
          学习提醒设置
        </h2>
        <button
          onClick={testNotification}
          className="px-4 py-2 bg-primary hover:bg-primary-hover rounded-lg transition-colors"
        >
          测试通知
        </button>
      </div>

      {/* Enable/Disable */}
      <div className="bg-bg-secondary rounded-lg p-6 border border-border">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            {settings.enabled ? (
              <Bell className="w-6 h-6 text-green-400" />
            ) : (
              <BellOff className="w-6 h-6 text-text-secondary" />
            )}
            <div>
              <h3 className="text-lg font-semibold">
                {settings.enabled ? '提醒已启用' : '提醒已禁用'}
              </h3>
              <p className="text-sm text-text-secondary">
                {settings.enabled ? '你将收到学习提醒和里程碑庆祝' : '所有提醒已关闭'}
              </p>
            </div>
          </div>
          <label className="relative inline-flex items-center cursor-pointer">
            <input
              type="checkbox"
              checked={settings.enabled}
              onChange={(e) => saveSettings({ ...settings, enabled: e.target.checked })}
              className="sr-only peer"
            />
            <div className="w-11 h-6 bg-bg-tertiary peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary"></div>
          </label>
        </div>
      </div>

      {/* Daily Reminder */}
      <div className="bg-bg-secondary rounded-lg p-6 border border-border">
        <div className="flex items-center gap-3 mb-4">
          <Clock className="w-6 h-6 text-primary" />
          <div>
            <h3 className="text-lg font-semibold">每日学习提醒</h3>
            <p className="text-sm text-text-secondary">在指定时间提醒你开始学习</p>
          </div>
        </div>
        <div className="flex items-center gap-4">
          <label className="text-sm text-text-secondary">提醒时间：</label>
          <input
            type="time"
            value={settings.dailyReminderTime}
            onChange={(e) => saveSettings({ ...settings, dailyReminderTime: e.target.value })}
            disabled={!settings.enabled}
            className="px-3 py-2 bg-bg-primary rounded-lg border border-border focus:outline-none focus:ring-2 focus:ring-primary disabled:opacity-50"
          />
        </div>
      </div>

      {/* Due Cards Reminder */}
      <div className="bg-bg-secondary rounded-lg p-6 border border-border">
        <div className="flex items-center gap-3 mb-4">
          <Target className="w-6 h-6 text-yellow-400" />
          <div>
            <h3 className="text-lg font-semibold">待复习卡牌提醒</h3>
            <p className="text-sm text-text-secondary">当待复习卡牌达到阈值时提醒</p>
          </div>
        </div>
        <div className="flex items-center gap-4">
          <label className="text-sm text-text-secondary">提醒阈值：</label>
          <input
            type="number"
            min="1"
            max="100"
            value={settings.dueCardsThreshold}
            onChange={(e) =>
              saveSettings({
                ...settings,
                dueCardsThreshold: parseInt(e.target.value) || 20,
              })
            }
            disabled={!settings.enabled}
            className="px-3 py-2 bg-bg-primary rounded-lg border border-border w-20 focus:outline-none focus:ring-2 focus:ring-primary disabled:opacity-50"
          />
          <span className="text-sm text-text-secondary">张卡牌</span>
        </div>
      </div>

      {/* Milestone Celebration */}
      <div className="bg-bg-secondary rounded-lg p-6 border border-border">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Trophy className="w-6 h-6 text-primary" />
            <div>
              <h3 className="text-lg font-semibold">学习里程碑庆祝</h3>
              <p className="text-sm text-text-secondary">连续学习3/7/30/100天时发送庆祝通知</p>
            </div>
          </div>
          <label className="relative inline-flex items-center cursor-pointer">
            <input
              type="checkbox"
              checked={settings.milestoneEnabled}
              onChange={(e) => saveSettings({ ...settings, milestoneEnabled: e.target.checked })}
              disabled={!settings.enabled}
              className="sr-only peer"
            />
            <div className="w-11 h-6 bg-bg-tertiary peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary disabled:opacity-50"></div>
          </label>
        </div>
      </div>

      {/* Notification Examples */}
      <div className="bg-bg-secondary rounded-lg p-6 border border-border">
        <div className="flex items-center gap-3 mb-4">
          <CheckCircle className="w-6 h-6 text-green-400" />
          <h3 className="text-lg font-semibold">通知示例</h3>
        </div>
        <div className="space-y-3 text-sm">
          <div className="flex items-start gap-3 p-3 bg-bg-primary rounded-lg border border-border">
            <span className="text-xl">📚</span>
            <div>
              <div className="font-semibold">学习提醒</div>
              <div className="text-text-secondary">
                今天还没开始学习哦！坚持每天学习，养成好习惯 💪
              </div>
            </div>
          </div>
          <div className="flex items-start gap-3 p-3 bg-bg-primary rounded-lg border border-border">
            <span className="text-xl">⏰</span>
            <div>
              <div className="font-semibold">复习提醒</div>
              <div className="text-text-secondary">
                有 25 个单词等待复习！趁记忆还清晰，赶紧巩固一下吧 📖
              </div>
            </div>
          </div>
          <div className="flex items-start gap-3 p-3 bg-bg-primary rounded-lg border border-border">
            <span className="text-xl">🔥</span>
            <div>
              <div className="font-semibold">学习里程碑！</div>
              <div className="text-text-secondary">
                恭喜你！已连续学习 7 天！坚持就是胜利，继续加油！🔥
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
