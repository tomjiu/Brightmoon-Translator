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

  const tabs: Array<{ id: StudyTab; icon: typeof GraduationCap; labelKey: string }> = [
    { id: 'learning', icon: GraduationCap, labelKey: 'vocabulary.tabs.learning' },
    { id: 'review', icon: RefreshCw, labelKey: 'vocabulary.tabs.review' },
    { id: 'modes', icon: Zap, labelKey: 'vocabulary.tabs.modes' },
    { id: 'statistics', icon: BarChart3, labelKey: 'vocabulary.tabs.statistics' },
    { id: 'fsrs', icon: Brain, labelKey: 'vocabulary.tabs.fsrs' },
    { id: 'data', icon: Database, labelKey: 'vocabulary.tabs.data' },
  ];

  return (
    <PageLayout
      toolbar={
        <div className="relative">
          <div className="flex items-center gap-1 overflow-x-auto">
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
          <div className="pointer-events-none absolute inset-y-0 right-0 w-6 bg-gradient-to-l from-bg-chrome to-transparent" />
          <div className="pointer-events-none absolute inset-y-0 left-0 w-6 bg-gradient-to-r from-bg-chrome to-transparent" />
        </div>
      }
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
