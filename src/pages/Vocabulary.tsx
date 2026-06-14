import { useState } from 'react';
import Dictionary from './Dictionary';
import Glossary from './Glossary';
import VocabularyLearning from './VocabularyLearning';
import DictionarySearch from './DictionarySearch';
import { useI18n } from '../i18n';
import { Book, BookOpen, GraduationCap, Search } from 'lucide-react';

type VocabTab = 'dictionary' | 'wordbook' | 'glossary' | 'learning';

function Vocabulary() {
  const [activeTab, setActiveTab] = useState<VocabTab>('dictionary');
  const { t } = useI18n();

  return (
    <div className="h-full flex flex-col">
      {/* Tab Bar */}
      <div className="flex items-center gap-1 px-4 py-2 border-b border-border bg-bg-secondary">
        <button
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'dictionary'
              ? 'bg-primary text-white'
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
              ? 'bg-primary text-white'
              : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
          }`}
          onClick={() => setActiveTab('learning')}
        >
          <GraduationCap size={14} />
          AI 学习
        </button>
        <button
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'wordbook'
              ? 'bg-primary text-white'
              : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
          }`}
          onClick={() => setActiveTab('wordbook')}
        >
          <Book size={14} />
          {t('vocabulary.wordbook')}
        </button>
        <button
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
            activeTab === 'glossary'
              ? 'bg-primary text-white'
              : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
          }`}
          onClick={() => setActiveTab('glossary')}
        >
          <BookOpen size={14} />
          {t('vocabulary.glossary')}
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {activeTab === 'dictionary' && <DictionarySearch />}
        {activeTab === 'learning' && <VocabularyLearning />}
        {activeTab === 'wordbook' && <Dictionary />}
        {activeTab === 'glossary' && <Glossary />}
      </div>
    </div>
  );
}

export default Vocabulary;
