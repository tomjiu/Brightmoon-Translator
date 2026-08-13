import { useState, lazy, Suspense } from 'react';
import { Search, BookMarked, HardDrive, Loader2 } from 'lucide-react';
import { useI18n } from '../i18n';
import PageLayout from '../components/PageLayout';

const DictionarySearch = lazy(() => import('./DictionarySearch'));
const WordBook = lazy(() => import('./WordBook'));
const DictOptimization = lazy(() => import('./DictOptimization'));

function LazyFallback() {
  return (
    <div className="flex items-center justify-center h-full text-text-secondary">
      <Loader2 size={18} className="animate-spin mr-2" />
      Loading...
    </div>
  );
}

type DictionaryTab = 'dictionary' | 'wordbook' | 'dictopt';

function Vocabulary() {
  const [activeTab, setActiveTab] = useState<DictionaryTab>('dictionary');
  const { t } = useI18n();

  const tabs = [
    { id: 'dictionary', icon: Search, label: t('vocabulary.tabs.dictionary') },
    { id: 'wordbook', icon: BookMarked, label: t('vocabulary.tabs.wordbook') },
    { id: 'dictopt', icon: HardDrive, label: t('vocabulary.tabs.dictopt') },
  ];

  return (
    <PageLayout
      tabs={tabs}
      activeTab={activeTab}
      onTabChange={(id) => setActiveTab(id as DictionaryTab)}
      scrollable={false}
    >
      <Suspense fallback={<LazyFallback />}>
        {activeTab === 'dictionary' && <DictionarySearch />}
        {activeTab === 'wordbook' && <WordBook />}
        {activeTab === 'dictopt' && <DictOptimization />}
      </Suspense>
    </PageLayout>
  );
}

export default Vocabulary;
