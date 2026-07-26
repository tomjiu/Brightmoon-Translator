import { useState } from 'react';
import VocabularyLearning from './VocabularyLearning';
import VocabularyReview from './VocabularyReview';
import DictionarySearch from './DictionarySearch';
import WordBook from './WordBook';
import LearningModes from './LearningModes';
import DataIO from './DataIO';
import FsrsOptimization from './FsrsOptimization';
import DictOptimization from './DictOptimization';
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
} from 'lucide-react';

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

  return (
    <div className="h-full flex flex-col">
      {/* Tab Bar */}
      <div className="flex items-center gap-1 px-4 py-2 border-b border-border bg-bg-secondary">
        <button
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'dictionary'
              ? 'bg-primary text-primary-fg'
              : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
          }`}
          onClick={() => setActiveTab('dictionary')}
        >
          <Search size={14} />
          词典查询
        </button>
        <button
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'learning'
              ? 'bg-primary text-primary-fg'
              : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
          }`}
          onClick={() => setActiveTab('learning')}
        >
          <GraduationCap size={14} />
          AI 学习
        </button>
        <button
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'review'
              ? 'bg-primary text-primary-fg'
              : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
          }`}
          onClick={() => setActiveTab('review')}
        >
          <RefreshCw size={14} />
          复习
        </button>
        <button
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'modes'
              ? 'bg-primary text-primary-fg'
              : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
          }`}
          onClick={() => setActiveTab('modes')}
        >
          <Zap size={14} />
          练习
        </button>
        <button
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'wordbook'
              ? 'bg-primary text-primary-fg'
              : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
          }`}
          onClick={() => setActiveTab('wordbook')}
        >
          <Book size={14} />
          生词本
        </button>
        <button
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'statistics'
              ? 'bg-primary text-primary-fg'
              : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
          }`}
          onClick={() => setActiveTab('statistics')}
        >
          <BarChart3 size={14} />
          统计
        </button>
        <button
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'data'
              ? 'bg-primary text-primary-fg'
              : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
          }`}
          onClick={() => setActiveTab('data')}
        >
          <Database size={14} />
          数据
        </button>
        <button
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'fsrs'
              ? 'bg-primary text-primary-fg'
              : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
          }`}
          onClick={() => setActiveTab('fsrs')}
        >
          <Brain size={14} />
          FSRS
        </button>
        <button
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'dictopt'
              ? 'bg-primary text-primary-fg'
              : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
          }`}
          onClick={() => setActiveTab('dictopt')}
        >
          <HardDrive size={14} />
          词典优化
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {activeTab === 'dictionary' && <DictionarySearch />}
        {activeTab === 'learning' && <VocabularyLearning />}
        {activeTab === 'review' && <VocabularyReview />}
        {activeTab === 'modes' && <LearningModes />}
        {activeTab === 'wordbook' && <WordBook />}
        {activeTab === 'statistics' && <LearningStatsDashboard />}
        {activeTab === 'data' && <DataIO />}
        {activeTab === 'fsrs' && <FsrsOptimization />}
        {activeTab === 'dictopt' && <DictOptimization />}
      </div>
    </div>
  );
}

export default Vocabulary;
