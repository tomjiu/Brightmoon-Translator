import { useState, useCallback } from "react";
import { useToastStore } from "../stores/toastStore";
import { aiMultiRoundTranslate, type MultiRoundResult } from "../services/ai";
import { useI18n } from "../i18n";
import { RefreshCw, Loader2, Check, ChevronDown } from "lucide-react";

interface AiMultiRoundButtonProps {
  text: string;
  fromLang: string;
  toLang: string;
  onResult: (result: MultiRoundResult) => void;
}

export default function AiMultiRoundButton({
  text,
  fromLang,
  toLang,
  onResult,
}: AiMultiRoundButtonProps) {
  const addToast = useToastStore((s) => s.addToast);
  const { t } = useI18n();

  const [isTranslating, setIsTranslating] = useState(false);
  const [rounds, setRounds] = useState(3);
  const [showRounds, setShowRounds] = useState(false);

  const handleMultiRound = useCallback(async () => {
    if (!text) {
      addToast({
        type: "warning",
        message: t("aiMultiRound.enterText"),
        duration: 3000,
      });
      return;
    }

    setIsTranslating(true);

    try {
      const result = await aiMultiRoundTranslate({
        text,
        fromLang,
        toLang,
        rounds,
      });
      onResult(result);
      addToast({
        type: "success",
        message: t("aiMultiRound.roundsComplete", { count: result.rounds.length }),
        duration: 3000,
      });
    } catch (err) {
      addToast({
        type: "error",
        message: t("aiMultiRound.failed"),
        detail: String(err),
        duration: 5000,
      });
    } finally {
      setIsTranslating(false);
    }
  }, [text, fromLang, toLang, rounds, onResult, addToast]);

  return (
    <div className="relative inline-flex">
      {/* Main button */}
      <button
        onClick={handleMultiRound}
        disabled={isTranslating}
        className="flex items-center gap-1.5 px-3 py-1.5 text-sm bg-gradient-to-r from-green-500 to-teal-500 text-white rounded-l-md hover:from-green-600 hover:to-teal-600 disabled:opacity-50 transition-all"
      >
        {isTranslating ? (
          <Loader2 className="w-4 h-4 animate-spin" />
        ) : (
          <RefreshCw className="w-4 h-4" />
        )}
        {isTranslating ? t("aiMultiRound.translating") : t("aiMultiRound.optimize")}
      </button>

      {/* Rounds selector */}
      <div className="relative">
        <button
          onClick={() => setShowRounds(!showRounds)}
          disabled={isTranslating}
          className="flex items-center gap-1 px-2 py-1.5 text-sm bg-gradient-to-r from-green-500 to-teal-500 text-white rounded-r-md border-l border-white/20 hover:from-green-600 hover:to-teal-600 disabled:opacity-50 transition-all"
        >
          <span className="text-xs">{t("aiMultiRound.roundsLabel", { count: rounds })}</span>
          <ChevronDown className="w-3 h-3" />
        </button>

        {showRounds && (
          <div className="absolute right-0 top-full mt-1 w-32 bg-bg-secondary border border-border rounded-lg shadow-lg z-50 overflow-hidden">
            {[2, 3].map((r) => (
              <button
                key={r}
                onClick={() => {
                  setRounds(r);
                  setShowRounds(false);
                }}
                className={`w-full flex items-center justify-between px-3 py-2 text-sm hover:bg-bg-tertiary transition-colors ${
                  rounds === r ? "bg-primary/10 text-primary" : ""
                }`}
              >
                <span>{t("aiMultiRound.roundsItem", { count: r })}</span>
                {rounds === r && <Check className="w-4 h-4" />}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
