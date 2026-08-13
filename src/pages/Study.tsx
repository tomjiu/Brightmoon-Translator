import { useState, lazy, Suspense } from 'react';
import { LearningStatsDashboard } from '../components/vocabulary';
import PageLayout from '../components/PageLayout';
import {
  GraduationCap,
  RefreshCw,
  Zap,
  BarChart3,
  Brain,
  Database,
  Loader2,
} from 'lucide-react';
import { useI18n } from '../i18n';

const VocabularyLearning = lazy(() => import('./VocabularyLearning'));
const VocabularyReview = lazy(() => import('./VocabularyReview'));
const LearningModes = lazy(() => import('./LearningModes'));
const DataIO = lazy(() => import('./DataIO'));
const FsrsOptimization = lazy(() => import('./FsrsOptimization'));

function LazyFallback() {
  return (
    <div className="flex items-center justify-center h-full text-text-secondary">
      <Loader2 size={18} className="animate-spin mr-2" />
      Loading...
    </div>
  );
}

type StudyTab = 'learning' | 'review' | 'modes' | 'statistics' | 'fsrs' | 'data';

function Study() {
  const [activeTab, setActiveTab] = useState<StudyTab>('learning');
  const { t } = useI18n();

  const tabs = [
    { id: 'learning', icon: GraduationCap, label: t('vocabulary.tabs.learning') },
    { id: 'review', icon: RefreshCw, label: t('vocabulary.tabs.review') },
    { id: 'modes', icon: Zap, label: t('vocabulary.tabs.modes') },
    { id: 'statistics', icon: BarChart3, label: t('vocabulary.tabs.statistics') },
    { id: 'fsrs', icon: Brain, label: t('vocabulary.tabs.fsrs') },
    { id: 'data', icon: Database, label: t('vocabulary.tabs.data') },
  ];

  return (
    <PageLayout
      tabs={tabs}
      activeTab={activeTab}
      onTabChange={(id) => setActiveTab(id as StudyTab)}
      scrollable={false}
    >
      <Suspense fallback={<LazyFallback />}>
        {activeTab === 'learning' && <VocabularyLearning />}
        {activeTab === 'review' && <VocabularyReview />}
        {activeTab === 'modes' && <LearningModes />}
        {activeTab === 'statistics' && <LearningStatsDashboard />}
        {activeTab === 'fsrs' && <FsrsOptimization />}
        {activeTab === 'data' && <DataIO />}
      </Suspense>
    </PageLayout>
  );
}

export default Study;
