import { useState, useCallback } from 'react';
import { useToastStore } from '../stores/toastStore';
import { aiPolishTranslation, type PolishStyle } from '../services/ai';
import { useI18n } from '../i18n';
import { Sparkles, ChevronDown, Loader2 } from 'lucide-react';

interface AiPolishButtonProps {
  sourceText: string;
  translatedText: string;
  fromLang: string;
  toLang: string;
  onPolished: (polishedText: string) => void;
}

const POLISH_STYLES: Array<{ id: PolishStyle; labelKey: string; icon: string }> = [
  { id: 'natural', labelKey: 'aiPolish.styleNatural', icon: '✨' },
  { id: 'formal', labelKey: 'aiPolish.styleFormal', icon: '📋' },
  { id: 'casual', labelKey: 'aiPolish.styleCasual', icon: '💬' },
  { id: 'technical', labelKey: 'aiPolish.styleTechnical', icon: '⚙️' },
  { id: 'literary', labelKey: 'aiPolish.styleLiterary', icon: '📖' },
];

export default function AiPolishButton({
  sourceText,
  translatedText,
  fromLang,
  toLang,
  onPolished,
}: AiPolishButtonProps) {
  const addToast = useToastStore((s) => s.addToast);
  const { t } = useI18n();

  const [isPolishing, setIsPolishing] = useState(false);
  const [showStyles, setShowStyles] = useState(false);
  const [selectedStyle, setSelectedStyle] = useState<PolishStyle>('natural');

  const handlePolish = useCallback(
    async (style?: PolishStyle) => {
      if (!sourceText || !translatedText) {
        addToast({
          type: 'warning',
          message: t('aiPolish.translateFirst'),
          duration: 3000,
        });
        return;
      }

      setIsPolishing(true);
      setShowStyles(false);

      try {
        const polished = await aiPolishTranslation({
          sourceText,
          translatedText,
          fromLang,
          toLang,
          style: style || selectedStyle,
        });
        onPolished(polished);
        addToast({
          type: 'success',
          message: t('aiPolish.completed'),
          duration: 2000,
        });
      } catch (err) {
        addToast({
          type: 'error',
          message: t('aiPolish.failed'),
          detail: String(err),
          duration: 5000,
        });
      } finally {
        setIsPolishing(false);
      }
    },
    [sourceText, translatedText, fromLang, toLang, selectedStyle, onPolished, addToast],
  );

  return (
    <div className="relative inline-flex">
      {/* Main polish button */}
      <button
        onClick={() => handlePolish()}
        disabled={isPolishing}
        className="flex items-center gap-1.5 px-3 py-1.5 text-sm bg-gradient-to-r from-purple-500 to-blue-500 text-white rounded-l-md hover:from-purple-600 hover:to-blue-600 disabled:opacity-50 transition-all"
      >
        {isPolishing ? (
          <Loader2 className="w-4 h-4 animate-spin" />
        ) : (
          <Sparkles className="w-4 h-4" />
        )}
        {isPolishing ? t('aiPolish.polishing') : t('aiPolish.button')}
      </button>

      {/* Style selector dropdown */}
      <div className="relative">
        <button
          onClick={() => setShowStyles(!showStyles)}
          disabled={isPolishing}
          className="flex items-center px-2 py-1.5 text-sm bg-gradient-to-r from-purple-500 to-blue-500 text-white rounded-r-md border-l border-white/20 hover:from-purple-600 hover:to-blue-600 disabled:opacity-50 transition-all"
        >
          <ChevronDown className="w-4 h-4" />
        </button>

        {showStyles && (
          <div className="absolute right-0 top-full mt-1 w-48 bg-bg-secondary border border-border rounded-lg shadow-lg z-50 overflow-hidden">
            {POLISH_STYLES.map((style) => (
              <button
                key={style.id}
                onClick={() => {
                  setSelectedStyle(style.id);
                  handlePolish(style.id);
                }}
                className={`w-full flex items-center gap-2 px-3 py-2 text-sm text-left hover:bg-bg-tertiary transition-colors ${
                  selectedStyle === style.id ? 'bg-primary/10 text-primary' : ''
                }`}
              >
                <span>{style.icon}</span>
                <span>{t(style.labelKey)}</span>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
