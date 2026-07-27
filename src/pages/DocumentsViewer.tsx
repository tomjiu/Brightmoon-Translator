import { useState } from 'react';
import PdfViewer from './PdfViewer';
import EpubViewer from './EpubViewer';
import SubtitleViewer from './SubtitleViewer';
import Glossary from './Glossary';
import { DocxViewer, ExcelViewer, PptxViewer } from './OfficeViewer';
import ImageFileTranslate from './ImageFileTranslate';
import { useI18n } from '../i18n';
import {
  FileText,
  BookOpen,
  Subtitles,
  Book,
  FileSpreadsheet,
  Presentation,
  Image,
} from 'lucide-react';

type DocFormat = 'pdf' | 'epub' | 'subtitle' | 'docx' | 'excel' | 'pptx' | 'image' | 'glossary';

const tabs: Array<{ id: DocFormat; icon: typeof FileText; labelKey: string; fallback: string }> = [
  { id: 'pdf', icon: FileText, labelKey: 'documents.pdf', fallback: 'PDF' },
  { id: 'epub', icon: BookOpen, labelKey: 'documents.epub', fallback: 'EPUB' },
  { id: 'subtitle', icon: Subtitles, labelKey: 'documents.subtitle', fallback: '字幕' },
  { id: 'docx', icon: FileText, labelKey: 'documents.docx', fallback: 'Word' },
  { id: 'excel', icon: FileSpreadsheet, labelKey: 'documents.excel', fallback: 'Excel' },
  { id: 'pptx', icon: Presentation, labelKey: 'documents.pptx', fallback: 'PPT' },
  { id: 'image', icon: Image, labelKey: 'documents.image', fallback: '图片' },
  { id: 'glossary', icon: Book, labelKey: 'vocabulary.glossary', fallback: '术语表' },
];

function DocumentsViewer() {
  const [activeTab, setActiveTab] = useState<DocFormat>('pdf');
  const { t } = useI18n();

  return (
    <div className="h-full flex flex-col">
      {/* Tab Bar */}
      <div className="ui-chrome flex items-center gap-1 px-4 py-2.5 border-b border-border">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
              activeTab === tab.id
                ? 'bg-primary text-primary-fg'
                : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
            }`}
            onClick={() => setActiveTab(tab.id)}
          >
            <tab.icon size={14} />
            {t(tab.labelKey) || tab.fallback}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {activeTab === 'pdf' && <PdfViewer />}
        {activeTab === 'epub' && <EpubViewer />}
        {activeTab === 'subtitle' && <SubtitleViewer />}
        {activeTab === 'docx' && <DocxViewer />}
        {activeTab === 'excel' && <ExcelViewer />}
        {activeTab === 'pptx' && <PptxViewer />}
        {activeTab === 'image' && <ImageFileTranslate />}
        {activeTab === 'glossary' && <Glossary />}
      </div>
    </div>
  );
}

export default DocumentsViewer;
