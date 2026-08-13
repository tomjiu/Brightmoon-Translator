import { useState, lazy, Suspense } from 'react';
import { Database, BarChart3, Loader2 } from 'lucide-react';
import { useI18n } from '../i18n';
import PageLayout from '../components/PageLayout';

const TmManager = lazy(() => import('./TmManager'));
const MetricsDashboard = lazy(() => import('./MetricsDashboard'));

function LazyFallback() {
  return (
    <div className="flex items-center justify-center h-full text-text-secondary">
      <Loader2 size={18} className="animate-spin mr-2" />
      Loading...
    </div>
  );
}

type HistoryTab = 'tm' | 'metrics';

function History() {
  const [activeTab, setActiveTab] = useState<HistoryTab>('tm');
  const { t } = useI18n();

  const tabs = [
    { id: 'tm', icon: Database, label: t('history.tabs.tm') },
    { id: 'metrics', icon: BarChart3, label: t('history.tabs.metrics') },
  ];

  return (
    <PageLayout
      tabs={tabs}
      activeTab={activeTab}
      onTabChange={(id) => setActiveTab(id as HistoryTab)}
      scrollable={false}
    >
      <Suspense fallback={<LazyFallback />}>
        {activeTab === 'tm' && <TmManager />}
        {activeTab === 'metrics' && <MetricsDashboard />}
      </Suspense>
    </PageLayout>
  );
}

export default History;
