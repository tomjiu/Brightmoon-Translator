import { useThemeStore } from '../../stores/themeStore';
import Card from '../../components/Card';

export default function AppearanceSettings() {
  const { theme, setTheme } = useThemeStore();

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-text-primary">外观主题</h1>
        <p className="text-sm text-text-secondary mt-1">配置应用主题和外观</p>
      </div>

      <Card title="主题模式" description="选择应用的颜色主题">
        <div className="space-y-3">
          <label className="flex items-start gap-3 p-4 rounded-lg border-2 cursor-pointer transition-all border-border hover:border-border/60">
            <input
              type="radio"
              name="theme"
              value="light"
              checked={theme === 'light'}
              onChange={() => setTheme('light')}
              className="mt-1"
            />
            <div>
              <p className="font-medium text-text-primary">浅色模式</p>
              <p className="text-sm text-text-secondary">明亮清新的界面</p>
            </div>
          </label>

          <label className="flex items-start gap-3 p-4 rounded-lg border-2 cursor-pointer transition-all border-border hover:border-border/60">
            <input
              type="radio"
              name="theme"
              value="dark"
              checked={theme === 'dark'}
              onChange={() => setTheme('dark')}
              className="mt-1"
            />
            <div>
              <p className="font-medium text-text-primary">深色模式</p>
              <p className="text-sm text-text-secondary">护眼的暗色界面</p>
            </div>
          </label>
        </div>
      </Card>
    </div>
  );
}
