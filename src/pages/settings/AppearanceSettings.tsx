import { useThemeStore } from '../../stores/themeStore';
import Card from '../../components/Card';
import Badge from '../../components/Badge';

export default function AppearanceSettings() {
  const { theme, setTheme } = useThemeStore();

  const themes = [
    {
      id: 'light',
      name: '浅色模式',
      description: '明亮清新的界面',
      preview: 'bg-white border-2 border-gray-200',
      available: true,
    },
    {
      id: 'dark',
      name: '深色模式',
      description: '护眼的暗色界面',
      preview: 'bg-gray-900 border-2 border-gray-700',
      available: true,
    },
    {
      id: 'nord',
      name: 'Nord 极光',
      description: '冷色调的北欧风格',
      preview: 'bg-gradient-to-br from-blue-900 to-teal-800',
      available: false,
    },
    {
      id: 'dracula',
      name: 'Dracula 吸血鬼',
      description: '高对比度的紫色主题',
      preview: 'bg-gradient-to-br from-purple-900 to-pink-800',
      available: false,
    },
    {
      id: 'monokai',
      name: 'Monokai 经典',
      description: '暖色调的编辑器主题',
      preview: 'bg-gradient-to-br from-yellow-900 to-orange-800',
      available: false,
    },
    {
      id: 'github',
      name: 'GitHub 风格',
      description: '清爽的开发者主题',
      preview: 'bg-gradient-to-br from-gray-50 to-blue-50 border-2 border-blue-200',
      available: false,
    },
  ];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-text-primary">外观主题</h1>
        <p className="text-sm text-text-secondary mt-1">配置应用主题和外观</p>
      </div>

      <Card title="主题模式" description="选择应用的颜色主题">
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {themes.map((themeOption) => (
            <label
              key={themeOption.id}
              className={`relative flex items-start gap-3 p-4 rounded-lg border-2 cursor-pointer transition-all ${
                theme === themeOption.id
                  ? 'border-primary bg-primary/5'
                  : themeOption.available
                    ? 'border-border hover:border-border/60'
                    : 'border-border opacity-60 cursor-not-allowed'
              }`}
            >
              <input
                type="radio"
                name="theme"
                value={themeOption.id}
                checked={theme === themeOption.id}
                disabled={!themeOption.available}
                onChange={() =>
                  themeOption.available && setTheme(themeOption.id as 'light' | 'dark')
                }
                className="mt-1"
              />

              {/* 主题预览色块 */}
              <div className={`w-12 h-12 rounded-lg ${themeOption.preview} shrink-0`} />

              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-1">
                  <p className="font-medium text-text-primary">{themeOption.name}</p>
                  {!themeOption.available && <Badge variant="warning">敬请期待</Badge>}
                </div>
                <p className="text-sm text-text-secondary">{themeOption.description}</p>
              </div>
            </label>
          ))}
        </div>
      </Card>

      <Card title="界面缩放" description="调整界面元素大小">
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">缩放比例</label>
            <select
              disabled
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg opacity-60 cursor-not-allowed"
            >
              <option>100% (默认)</option>
              <option>125%</option>
              <option>150%</option>
            </select>
            <p className="text-xs text-text-secondary mt-1">
              <Badge variant="warning">功能开发中</Badge>
            </p>
          </div>
        </div>
      </Card>
    </div>
  );
}
