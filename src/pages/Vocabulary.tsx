import { useState, lazy, Suspense } from 'react';
import { LearningStatsDashboard } from '../components/vocabulary';
import {
  Search,
  GraduationCap,
  Book,
  RefreshCw,
  BarChart3,
  Zap,
  Database,
  Brain,
  HardDrive,
  Loader2,
} from 'lucide-react';
import { useI18n } from '../i18n';

const VocabularyLearning = lazy(() => import('./VocabularyLearning'));
const VocabularyReview = lazy(() => import('./VocabularyReview'));
const DictionarySearch = lazy(() => import('./DictionarySearch'));
const WordBook = lazy(() => import('./WordBook'));
const LearningModes = lazy(() => import('./LearningModes'));
const DataIO = lazy(() => import('./DataIO'));
const FsrsOptimization = lazy(() => import('./FsrsOptimization'));
const DictOptimization = lazy(() => import('./DictOptimization'));

function LazyFallback() {
  return (
    <div className="flex items-center justify-center h-32 text-text-secondary">
      <Loader2 size={18} className="animate-spin mr-2" />
      Loading...
    </div>
  );
}

type VocabTab =
  | 'dictionary'
  | 'learning'
  | 'review'
  | 'modes'
  | 'wordbook'
  | 'statistics'
  | 'data'
  | 'fsrs'
  | 'dictopt';

function Vocabulary() {
  const [activeTab, setActiveTab] = useState<VocabTab>('dictionary');
  const { t } = useI18n();

  const tabs: Array<{ id: VocabTab; icon: typeof Search; labelKey: string }> = [
    { id: 'dictionary', icon: Search, labelKey: 'vocabulary.tabs.dictionary' },
    { id: 'learning', icon: GraduationCap, labelKey: 'vocabulary.tabs.learning' },
    { id: 'review', icon: RefreshCw, labelKey: 'vocabulary.tabs.review' },
    { id: 'modes', icon: Zap, labelKey: 'vocabulary.tabs.modes' },
    { id: 'wordbook', icon: Book, labelKey: 'vocabulary.tabs.wordbook' },
    { id: 'statistics', icon: BarChart3, labelKey: 'vocabulary.tabs.statistics' },
    { id: 'data', icon: Database, labelKey: 'vocabulary.tabs.data' },
    { id: 'fsrs', icon: Brain, labelKey: 'vocabulary.tabs.fsrs' },
    { id: 'dictopt', icon: HardDrive, labelKey: 'vocabulary.tabs.dictopt' },
  ];

  return (
    <div className="h-full flex flex-col">
      <div className="ui-chrome flex items-center gap-1 px-4 py-2.5 border-b border-border overflow-x-auto">
        {tabs.map(({ id, icon: Icon, labelKey }) => (
          <button
            key={id}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
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
          {activeTab === 'dictionary' && <DictionarySearch />}
          {activeTab === 'learning' && <VocabularyLearning />}
          {activeTab === 'review' && <VocabularyReview />}
          {activeTab === 'modes' && <LearningModes />}
          {activeTab === 'wordbook' && <WordBook />}
          {activeTab === 'statistics' && <LearningStatsDashboard />}
          {activeTab === 'data' && <DataIO />}
          {activeTab === 'fsrs' && <FsrsOptimization />}
          {activeTab === 'dictopt' && <DictOptimization />}
        </Suspense>
      </div>
    </div>
  );
}

export default Vocabulary;
