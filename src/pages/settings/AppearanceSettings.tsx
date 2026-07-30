import { Moon, Sun } from 'lucide-react';
import { useThemeStore } from '../../stores/themeStore';
import Card from '../../components/Card';
import { useI18n } from '../../i18n';

export default function AppearanceSettings() {
  const { theme, setTheme } = useThemeStore();
  const { t } = useI18n();

  const themes = [
    {
      id: 'dark' as const,
      name: t('settings.appearance.dark'),
      description: t('settings.appearance.darkDesc'),
      icon: Moon,
      swatch: 'bg-black border-border',
      bar: 'bg-neutral-900',
      panel: 'bg-neutral-950',
    },
    {
      id: 'light' as const,
      name: t('settings.appearance.light'),
      description: t('settings.appearance.lightDesc'),
      icon: Sun,
      swatch: 'bg-white border-border',
      bar: 'bg-white',
      panel: 'bg-neutral-100',
    },
  ];

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">{t('settings.appearance.pageTitle')}</h1>
        <p className="ui-page-desc">{t('settings.appearance.pageDesc')}</p>
      </div>

      <Card
        title={t('settings.appearance.themeTitle')}
        description={t('settings.appearance.themeDesc')}
      >
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
          {themes.map((opt) => {
            const Icon = opt.icon;
            const active = theme === opt.id;
            return (
              <button
                key={opt.id}
                type="button"
                onClick={() => setTheme(opt.id)}
                className={`text-left rounded-xl border p-4 transition-all duration-150 ease-out ${
                  active
                    ? 'border-primary bg-primary/5 shadow-sm'
                    : 'border-border hover:border-border-strong bg-bg-secondary'
                }`}
              >
                <div className={`w-full h-20 rounded-lg border overflow-hidden mb-3 ${opt.swatch}`}>
                  <div className={`h-5 border-b border-black/10 ${opt.bar}`} />
                  <div className="flex h-[calc(100%-1.25rem)]">
                    <div className={`w-4 border-r border-black/10 ${opt.bar}`} />
                    <div className={`flex-1 m-1.5 rounded ${opt.panel}`} />
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <Icon size={16} className="text-text-secondary" />
                  <span className="text-sm font-semibold text-text-primary">{opt.name}</span>
                  {active && (
                    <span className="ml-auto text-[10px] uppercase tracking-wider text-text-secondary">
                      {t('settings.appearance.current')}
                    </span>
                  )}
                </div>
                <p className="text-xs text-text-secondary mt-1.5 leading-relaxed">
                  {opt.description}
                </p>
              </button>
            );
          })}
        </div>
      </Card>
    </div>
  );
}
