import { useState, lazy, Suspense } from 'react';
import { Database, BarChart3, Loader2 } from 'lucide-react';
import { useI18n } from '../i18n';

const TmManager = lazy(() => import('./TmManager'));
const MetricsDashboard = lazy(() => import('./MetricsDashboard'));

function LazyFallback() {
  return (
    <div className="flex items-center justify-center h-32 text-text-secondary">
      <Loader2 size={18} className="animate-spin mr-2" />
      Loading...
    </div>
  );
}

type HistoryTab = 'tm' | 'metrics';

function History() {
  const [activeTab, setActiveTab] = useState<HistoryTab>('tm');
  const { t } = useI18n();

  const tabs: Array<{ id: HistoryTab; icon: typeof Database; labelKey: string }> = [
    { id: 'tm', icon: Database, labelKey: 'history.tabs.tm' },
    { id: 'metrics', icon: BarChart3, labelKey: 'history.tabs.metrics' },
  ];

  return (
    <div className="h-full flex flex-col">
      <div className="ui-chrome flex items-center gap-1 px-4 py-2 border-b border-border overflow-x-auto">
        {tabs.map(({ id, icon: Icon, labelKey }) => (
          <button
            key={id}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors shrink-0 ${
              activeTab === id
                ? 'bg-primary text-primary-fg'
                : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
            }`}
            onClick={() => setActiveTab(id)}
          >
            <Icon size={14} />
            {t(labelKey)}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-hidden">
        <Suspense fallback={<LazyFallback />}>
          {activeTab === 'tm' && <TmManager />}
          {activeTab === 'metrics' && <MetricsDashboard />}
        </Suspense>
      </div>
    </div>
  );
}

export default History;
