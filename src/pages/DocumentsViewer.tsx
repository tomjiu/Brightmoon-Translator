import { useState } from 'react';
import PdfViewer from './PdfViewer';
import EpubViewer from './EpubViewer';
import SubtitleViewer from './SubtitleViewer';
import { useI18n } from '../i18n';
import { FileText, BookOpen, Subtitles } from 'lucide-react';

type DocFormat = 'pdf' | 'epub' | 'subtitle';

const tabs: Array<{ id: DocFormat; icon: typeof FileText; labelKey: string }> = [
  { id: 'pdf', icon: FileText, labelKey: 'documents.pdf' },
  { id: 'epub', icon: BookOpen, labelKey: 'documents.epub' },
  { id: 'subtitle', icon: Subtitles, labelKey: 'documents.subtitle' },
];

function DocumentsViewer() {
  const [activeTab, setActiveTab] = useState<DocFormat>('pdf');
  const { t } = useI18n();

  return (
    <div className="h-full flex flex-col">
      {/* Tab Bar */}
      <div className="flex items-center gap-1 px-4 py-2 border-b border-border bg-bg-secondary">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
              activeTab === tab.id
                ? 'bg-primary text-white'
                : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
            }`}
            onClick={() => setActiveTab(tab.id)}
          >
            <tab.icon size={14} />
            {t(tab.labelKey)}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {activeTab === 'pdf' && <PdfViewer />}
        {activeTab === 'epub' && <EpubViewer />}
        {activeTab === 'subtitle' && <SubtitleViewer />}
      </div>
    </div>
  );
}

export default DocumentsViewer;
