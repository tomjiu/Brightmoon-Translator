// VocabularyLearning - 词汇学习主页面

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useVocabularyStore } from '../stores/vocabularyStore';
import { LearningStatsPanel, WelcomeGuide } from '../components/vocabulary';
import {
  BookOpen,
  Plus,
  Trash2,
  Play,
  GraduationCap,
  BarChart3,
  FileText,
  Clipboard,
  X,
  Upload,
  Volume2,
  Sparkles,
} from 'lucide-react';

interface ExamWordlist {
  exam: string;
  examZh: string;
  wordCount: number;
  icon: string;
  description: string;
}
interface PlanSummary {
  plan: { id: string; name: string; description?: string; totalWords: number; dailyTarget: number };
  progress: {
    totalWords: number;
    learnedWords: number;
    remainingWords: number;
    completionRate: number;
  };
  todayTarget: number;
  todayCompleted: number;
}
interface WordDetail {
  word: string;
  phonetics: Array<{ text?: string; audio?: string; source: string }>;
  chineseTranslation?: string;
  englishDefinitions: string[];
  oxfordDefinition?: string;
  collinsEntries: Array<{
    pos: string;
    posCn: string;
    englishDef: string;
    examples: Array<{ en: string; zh: string }>;
  }>;
  onlineMeanings: Array<{
    partOfSpeech: string;
    definitions: Array<{ definition: string; example?: string }>;
  }>;
  examples: Array<{ en: string; zh: string }>;
  usAudioUrl?: string;
  ukAudioUrl?: string;
  sources: string[];
  imageUrl?: string;
  aiContent?: {
    mnemonics?: Array<{ content: string }>;
    examples?: Array<{ text: string; context: string }>;
    etymology?: { origin: string };
  };
}

type ViewMode = 'plans' | 'study' | 'stats';
type ImportMode = null | 'file' | 'text';

export default function VocabularyLearning() {
  const [viewMode, setViewMode] = useState<ViewMode>('plans');
  const [importMode, setImportMode] = useState<ImportMode>(null);
  const [examWordlists, setExamWordlists] = useState<ExamWordlist[]>([]);
  const [plans, setPlans] = useState<PlanSummary[]>([]);
  const [todayWords, setTodayWords] = useState<string[]>([]);
  const [currentIdx, setCurrentIdx] = useState(0);
  const [activePlanId, setActivePlanId] = useState<string | null>(null);
  const [showAnswer, setShowAnswer] = useState(false);
  const [wordDetail, setWordDetail] = useState<WordDetail | null>(null);
  const [busy, setBusy] = useState(false);
  const [statusMsg, setStatusMsg] = useState('');
  const [importName, setImportName] = useState('');
  const [importDaily, setImportDaily] = useState(30);
  const [importText, setImportText] = useState('');
  const [importFilePath, setImportFilePath] = useState('');

  const { startSession, endSession } = useVocabularyStore();

  const loadData = useCallback(async () => {
    try {
      const [exams, myPlans] = await Promise.all([
        invoke<ExamWordlist[]>('get_exam_wordlists'),
        invoke<PlanSummary[]>('get_learning_plans'),
      ]);
      setExamWordlists(exams);
      setPlans(myPlans);
    } catch (err) {
      console.error(err);
    }
  }, []);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const handleCreatePreset = async (exam: string) => {
    setBusy(true);
    setStatusMsg('正在创建计划...');
    try {
      await invoke('create_learning_plan', { exam, dailyTarget: 30 });
      await loadData();
      setStatusMsg('');
    } catch (err) {
      setStatusMsg(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleImportFile = async () => {
    if (!importName.trim()) {
      setStatusMsg('请输入计划名称');
      return;
    }
    setBusy(true);
    setStatusMsg('正在导入...');
    try {
      await invoke('import_wordlist_from_file', {
        filePath: importFilePath,
        planName: importName.trim(),
        dailyTarget: importDaily,
      });
      await loadData();
      setImportMode(null);
      setImportName('');
      setImportFilePath('');
      setStatusMsg('');
    } catch (err) {
      setStatusMsg(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleImportText = async () => {
    if (!importName.trim() || !importText.trim()) {
      setStatusMsg('请填写完整');
      return;
    }
    setBusy(true);
    setStatusMsg('正在导入...');
    try {
      await invoke('import_wordlist_from_text', {
        text: importText,
        planName: importName.trim(),
        dailyTarget: importDaily,
      });
      await loadData();
      setImportMode(null);
      setImportName('');
      setImportText('');
      setStatusMsg('');
    } catch (err) {
      setStatusMsg(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleSelectFile = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        filters: [{ name: '词表文件', extensions: ['txt', 'md', 'csv', 'docx', 'pdf'] }],
      });
      if (selected && typeof selected === 'string') {
        setImportFilePath(selected);
        if (!importName) {
          setImportName(
            `${
              selected
                .split(/[/\\]/)
                .pop()
                ?.replace(/\.\w+$/, '') || ''
            }词表`,
          );
        }
      }
    } catch {
      /* dialog cancelled */
    }
  };

  const handleStartStudy = async (planId: string) => {
    try {
      const words = await invoke<string[]>('get_plan_today_words', { planId });
      if (words.length === 0) {
        setStatusMsg('今日已完成！');
        return;
      }
      setTodayWords(words);
      setCurrentIdx(0);
      setActivePlanId(planId);
      setShowAnswer(false);
      setViewMode('study');
      startSession();
      void loadWordDetail(words[0]);
    } catch (err) {
      setStatusMsg(String(err));
    }
  };

  const loadWordDetail = async (word: string) => {
    setWordDetail(null);
    try {
      // 用 study_word 一站式命令：查词典 + 创建卡牌 + 准备 AI 内容
      const data = await invoke<WordDetail & { phonetic?: string }>('study_word', { word });
      setWordDetail({
        word: data.word,
        phonetics: data.phonetic ? [{ text: data.phonetic, source: 'ECDICT' }] : [],
        chineseTranslation: data.chineseTranslation,
        englishDefinitions: data.englishDefinitions || [],
        collinsEntries: data.collinsEntries || [],
        onlineMeanings: [],
        examples: data.examples || [],
        usAudioUrl: data.usAudioUrl,
        ukAudioUrl: data.ukAudioUrl,
        sources: data.sources || [],
        imageUrl: data.imageUrl,
        aiContent: data.aiContent,
      });
    } catch (err) {
      console.error('study_word failed:', err);
      // fallback
      try {
        interface LookupMeaning {
          partOfSpeech: string;
          definitions: Array<{ definition: string }>;
        }
        const basic = await invoke<{
          word: string;
          phonetic?: string;
          meanings?: LookupMeaning[];
        }>('lookup_word_detail', { word });
        setWordDetail({
          word: basic.word,
          phonetics: basic.phonetic ? [{ text: basic.phonetic, source: 'ECDICT' }] : [],
          chineseTranslation: basic.meanings?.find((m) => m.partOfSpeech === '中文释义')
            ?.definitions[0]?.definition,
          englishDefinitions:
            basic.meanings
              ?.filter((m) => m.partOfSpeech !== '中文释义')
              .flatMap((m) => m.definitions.map((d) => d.definition)) || [],
          collinsEntries: [],
          onlineMeanings: [],
          examples: [],
          sources: ['ECDICT'],
        });
      } catch {
        setWordDetail(null);
      }
    }
  };

  const handleKnow = async () => {
    if (activePlanId && todayWords[currentIdx]) {
      await invoke('mark_word_learned', { planId: activePlanId, word: todayWords[currentIdx] });
    }
    goNext();
  };

  const goNext = () => {
    const next = currentIdx + 1;
    if (next >= todayWords.length) {
      endSession();
      setViewMode('plans');
      void loadData();
      return;
    }
    setCurrentIdx(next);
    setShowAnswer(false);
    void loadWordDetail(todayWords[next]);
  };

  const playAudio = (url: string) => {
    new Audio(url).play();
  };

  const handleDelete = async (id: string) => {
    // eslint-disable-next-line no-alert -- destructive confirm; no dialog component available
    if (!confirm('确定删除？')) return;
    await invoke('delete_learning_plan', { planId: id });
    await loadData();
  };

  return (
    <div className="h-full flex">
      {/* Sidebar */}
      <div className="w-52 border-r border-border bg-bg-secondary flex flex-col">
        <div className="p-4">
          <div className="flex items-center gap-2 mb-4">
            <GraduationCap size={16} className="text-primary" />
            <h2 className="text-sm font-semibold text-text-primary">词汇学习</h2>
          </div>
          <div className="space-y-1">
            <NavBtn
              active={viewMode === 'plans'}
              onClick={() => setViewMode('plans')}
              icon={<BookOpen size={16} />}
              label="学习计划"
            />
            <NavBtn
              active={viewMode === 'stats'}
              onClick={() => setViewMode('stats')}
              icon={<BarChart3 size={16} />}
              label="统计"
            />
          </div>
        </div>
        {viewMode !== 'stats' && (
          <div className="px-4 pb-4 mt-auto">
            <LearningStatsPanel />
          </div>
        )}
      </div>

      <div className="flex-1 overflow-y-auto">
        {statusMsg && (
          <div className="px-6 py-2 bg-primary/10 text-primary text-xs">{statusMsg}</div>
        )}

        {/* ===== 计划列表 ===== */}
        {viewMode === 'plans' && (
          <div className="p-6 max-w-4xl mx-auto space-y-6">
            {plans.length === 0 && (
              <WelcomeGuide
                onStartLearning={() => setViewMode('plans')}
                onStartReview={() => {
                  /* review entry handled elsewhere */
                }}
              />
            )}

            {plans.length > 0 && (
              <div>
                <h3 className="text-sm font-semibold text-text-secondary mb-3">我的学习计划</h3>
                <div className="space-y-2">
                  {plans.map(({ plan, progress }) => (
                    <div
                      key={plan.id}
                      className="flex items-center gap-4 p-3 bg-bg-secondary border border-border rounded-lg"
                    >
                      <div className="flex-1 min-w-0">
                        <h4 className="text-sm font-medium text-text-primary">{plan.name}</h4>
                        <div className="flex items-center gap-3 mt-1 text-xs text-text-secondary">
                          <span>
                            {progress.learnedWords}/{progress.totalWords}
                          </span>
                          <span>每日 {plan.dailyTarget} 词</span>
                        </div>
                        <div className="mt-1.5 h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
                          <div
                            className="h-full bg-primary rounded-full"
                            style={{ width: `${progress.completionRate}%` }}
                          />
                        </div>
                      </div>
                      <button
                        onClick={() => void handleStartStudy(plan.id)}
                        className="flex items-center gap-1 px-3 py-1.5 bg-primary text-primary-fg text-xs rounded-lg hover:bg-primary/90"
                      >
                        <Play size={14} /> 学习
                      </button>
                      <button
                        onClick={() => void handleDelete(plan.id)}
                        className="p-1.5 text-text-tertiary hover:text-red-500"
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                  ))}
                </div>
              </div>
            )}

            <div>
              <h3 className="text-sm font-semibold text-text-secondary mb-3">创建考试计划</h3>
              <div className="grid grid-cols-3 gap-2">
                {examWordlists.map((ex) => (
                  <div
                    key={ex.exam}
                    className="flex items-center gap-3 p-3 bg-bg-secondary border border-border rounded-lg"
                  >
                    <span className="text-xl">{ex.icon}</span>
                    <div className="flex-1 min-w-0">
                      <h4 className="text-sm font-medium text-text-primary">{ex.examZh}</h4>
                      <p className="text-xs text-text-tertiary">{ex.wordCount} 词</p>
                    </div>
                    <button
                      onClick={() => void handleCreatePreset(ex.exam)}
                      disabled={busy}
                      className="p-1.5 text-text-tertiary hover:text-primary hover:bg-bg-tertiary rounded disabled:opacity-40"
                      title="创建"
                    >
                      <Plus size={16} />
                    </button>
                  </div>
                ))}
              </div>
            </div>

            <div>
              <h3 className="text-sm font-semibold text-text-secondary mb-3">导入词表</h3>
              {importMode === null ? (
                <div className="flex gap-2">
                  <button
                    onClick={() => setImportMode('file')}
                    className="flex items-center gap-2 px-4 py-2.5 bg-bg-secondary border border-border rounded-lg hover:border-primary/50 text-sm text-text-secondary"
                  >
                    <FileText size={16} /> 从文件导入
                  </button>
                  <button
                    onClick={() => setImportMode('text')}
                    className="flex items-center gap-2 px-4 py-2.5 bg-bg-secondary border border-border rounded-lg hover:border-primary/50 text-sm text-text-secondary"
                  >
                    <Clipboard size={16} /> 粘贴文本导入
                  </button>
                </div>
              ) : (
                <div className="p-4 bg-bg-secondary border border-border rounded-lg space-y-3">
                  <div className="flex items-center justify-between">
                    <h4 className="text-sm font-medium">
                      {importMode === 'file' ? '从文件导入' : '粘贴文本导入'}
                    </h4>
                    <button
                      onClick={() => setImportMode(null)}
                      className="text-text-tertiary hover:text-text-primary"
                    >
                      <X size={16} />
                    </button>
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label className="block text-xs text-text-secondary mb-1">计划名称</label>
                      <input
                        type="text"
                        value={importName}
                        onChange={(e) => setImportName(e.target.value)}
                        placeholder="我的词表"
                        className="w-full px-3 py-2 text-sm bg-bg-primary border border-border rounded focus:ring-2 focus:ring-primary outline-none"
                      />
                    </div>
                    <div>
                      <label className="block text-xs text-text-secondary mb-1">每日目标</label>
                      <input
                        type="number"
                        value={importDaily}
                        onChange={(e) => setImportDaily(Number(e.target.value))}
                        min={5}
                        max={100}
                        className="w-full px-3 py-2 text-sm bg-bg-primary border border-border rounded focus:ring-2 focus:ring-primary outline-none"
                      />
                    </div>
                  </div>
                  {importMode === 'file' ? (
                    <>
                      <div className="flex gap-2">
                        <button
                          onClick={handleSelectFile}
                          className="flex items-center gap-2 px-4 py-2 bg-bg-tertiary text-text-secondary rounded border border-border text-sm"
                        >
                          <Upload size={14} /> 选择文件
                        </button>
                        {importFilePath && (
                          <span className="text-xs text-text-secondary self-center truncate">
                            {importFilePath}
                          </span>
                        )}
                      </div>
                      <p className="text-xs text-text-tertiary">支持 txt, md, csv, docx, pdf</p>
                      <button
                        onClick={handleImportFile}
                        disabled={busy || !importFilePath}
                        className="w-full py-2 bg-primary text-primary-fg rounded disabled:opacity-50 text-sm"
                      >
                        {busy ? '导入中...' : '开始导入'}
                      </button>
                    </>
                  ) : (
                    <>
                      <textarea
                        value={importText}
                        onChange={(e) => setImportText(e.target.value)}
                        placeholder="每行一个单词："
                        rows={5}
                        className="w-full px-3 py-2 text-sm bg-bg-primary border border-border rounded resize-none font-mono"
                      />
                      <button
                        onClick={handleImportText}
                        disabled={busy || !importText.trim()}
                        className="w-full py-2 bg-primary text-primary-fg rounded disabled:opacity-50 text-sm"
                      >
                        {busy ? '导入中...' : '开始导入'}
                      </button>
                    </>
                  )}
                </div>
              )}
            </div>
          </div>
        )}

        {/* ===== 学习模式 ===== */}
        {viewMode === 'study' && (
          <div className="h-full flex flex-col">
            <div className="px-6 pt-4 flex items-center justify-between text-xs text-text-secondary">
              <span>
                {currentIdx + 1} / {todayWords.length}
              </span>
              <button
                onClick={() => {
                  endSession();
                  setViewMode('plans');
                  void loadData();
                }}
                className="hover:text-text-primary"
              >
                退出
              </button>
            </div>
            <div className="px-6">
              <div className="h-1 bg-bg-tertiary rounded-full overflow-hidden">
                <div
                  className="h-full bg-primary rounded-full transition-all"
                  style={{ width: `${(currentIdx / todayWords.length) * 100}%` }}
                />
              </div>
            </div>

            <div className="flex-1 overflow-y-auto p-6">
              <div className="max-w-2xl mx-auto">
                {/* 背景图 */}
                {wordDetail?.imageUrl && !showAnswer && (
                  <div
                    className="relative mb-6 rounded-xl overflow-hidden"
                    style={{ height: '200px' }}
                  >
                    <img
                      src={wordDetail.imageUrl}
                      alt={todayWords[currentIdx]}
                      className="w-full h-full object-cover"
                      onError={(e) => {
                        (e.target as HTMLImageElement).style.display = 'none';
                      }}
                    />
                    <div className="absolute inset-0 bg-gradient-to-t from-black/60 via-transparent to-transparent" />
                    <div className="absolute bottom-4 left-0 right-0 text-center">
                      <h1 className="text-4xl font-bold text-white drop-shadow-lg">
                        {todayWords[currentIdx]}
                      </h1>
                    </div>
                  </div>
                )}

                {/* 音频按钮（图片模式下也显示） */}
                {wordDetail?.imageUrl && !showAnswer && (
                  <div className="flex items-center justify-center gap-3 mb-4">
                    {wordDetail.phonetics[0]?.text && (
                      <span className="text-text-secondary">/{wordDetail.phonetics[0].text}/</span>
                    )}
                    {wordDetail.usAudioUrl && (
                      <button
                        onClick={() => playAudio(wordDetail.usAudioUrl!)}
                        className="flex items-center gap-1 px-2 py-0.5 text-xs bg-bg-tertiary text-primary rounded hover:bg-bg-tertiary"
                      >
                        <Volume2 size={12} /> 美音
                      </button>
                    )}
                    {wordDetail.ukAudioUrl && (
                      <button
                        onClick={() => playAudio(wordDetail.ukAudioUrl!)}
                        className="flex items-center gap-1 px-2 py-0.5 text-xs bg-green-50 text-green-600 rounded hover:bg-green-100"
                      >
                        <Volume2 size={12} /> 英音
                      </button>
                    )}
                  </div>
                )}

                {/* 单词头部（无图时显示） */}
                {(!wordDetail?.imageUrl || showAnswer) && (
                  <div className="text-center mb-6">
                    <h1 className="text-4xl font-bold text-text-primary mb-2">
                      {todayWords[currentIdx]}
                    </h1>
                    <div className="flex items-center justify-center gap-3">
                      {wordDetail?.phonetics[0]?.text && (
                        <span className="text-text-secondary">
                          /{wordDetail.phonetics[0].text}/
                        </span>
                      )}
                      {wordDetail?.usAudioUrl && (
                        <button
                          onClick={() => playAudio(wordDetail.usAudioUrl!)}
                          className="flex items-center gap-1 px-2 py-0.5 text-xs bg-bg-tertiary text-primary rounded hover:bg-bg-tertiary"
                        >
                          <Volume2 size={12} /> 美音
                        </button>
                      )}
                      {wordDetail?.ukAudioUrl && (
                        <button
                          onClick={() => playAudio(wordDetail.ukAudioUrl!)}
                          className="flex items-center gap-1 px-2 py-0.5 text-xs bg-green-50 text-green-600 rounded hover:bg-green-100"
                        >
                          <Volume2 size={12} /> 英音
                        </button>
                      )}
                    </div>
                  </div>
                )}

                {!showAnswer ? (
                  <div className="text-center">
                    <button
                      onClick={() => setShowAnswer(true)}
                      className="px-10 py-3 bg-primary text-primary-fg rounded-lg hover:bg-primary/90 text-lg"
                    >
                      显示释义
                    </button>
                  </div>
                ) : (
                  <div className="space-y-4 animate-fadeIn">
                    {/* 中文释义 */}
                    {wordDetail?.chineseTranslation && (
                      <div className="p-4 bg-bg-secondary border border-border rounded-lg">
                        <h3 className="text-xs font-semibold text-primary mb-2">中文释义</h3>
                        <p className="text-text-primary leading-relaxed">
                          {wordDetail.chineseTranslation}
                        </p>
                      </div>
                    )}

                    {/* 柯林斯释义（权威英英 + 双语例句） */}
                    {wordDetail?.collinsEntries && wordDetail.collinsEntries.length > 0 && (
                      <div className="p-4 bg-bg-secondary border border-border rounded-lg">
                        <h3 className="text-xs font-semibold text-orange-600 mb-2">
                          📖 柯林斯词典
                        </h3>
                        <div className="space-y-3">
                          {wordDetail.collinsEntries.slice(0, 4).map((ce, i) => (
                            <div key={i}>
                              <div className="flex items-center gap-2 mb-1">
                                <span className="text-xs px-1.5 py-0.5 bg-orange-100 text-orange-700 rounded font-mono">
                                  {ce.pos}
                                </span>
                                {ce.posCn && (
                                  <span className="text-xs text-text-secondary">{ce.posCn}</span>
                                )}
                              </div>
                              <p className="text-sm text-text-primary mb-1">{ce.englishDef}</p>
                              {ce.examples.slice(0, 1).map((ex, j) => (
                                <div
                                  key={j}
                                  className="ml-2 pl-2 border-l-2 border-orange-200 mt-1"
                                >
                                  <p className="text-xs text-text-primary italic">{ex.en}</p>
                                  <p className="text-xs text-text-secondary">{ex.zh}</p>
                                </div>
                              ))}
                            </div>
                          ))}
                        </div>
                      </div>
                    )}

                    {/* ECDICT 英文释义 */}
                    {wordDetail?.englishDefinitions &&
                      wordDetail.englishDefinitions.length > 0 &&
                      !wordDetail.collinsEntries.length && (
                        <div className="p-4 bg-bg-secondary border border-border rounded-lg">
                          <h3 className="text-xs font-semibold text-primary mb-2">英文释义</h3>
                          <ul className="space-y-0.5">
                            {wordDetail.englishDefinitions.slice(0, 5).map((d, i) => (
                              <li key={i} className="text-sm text-text-primary">
                                • {d}
                              </li>
                            ))}
                          </ul>
                        </div>
                      )}

                    {/* 有道例句 */}
                    {wordDetail?.examples && wordDetail.examples.length > 0 && (
                      <div className="p-4 bg-bg-secondary border border-border rounded-lg">
                        <h3 className="text-xs font-semibold text-neutral-500 mb-2">📝 双语例句</h3>
                        <div className="space-y-2">
                          {wordDetail.examples.slice(0, 3).map((ex, i) => (
                            <div key={i} className="pl-3 border-l-2 border-border">
                              <p className="text-sm text-text-primary">{ex.en}</p>
                              <p className="text-xs text-text-secondary">{ex.zh}</p>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}

                    {/* DictionaryAPI.dev 释义 */}
                    {wordDetail?.onlineMeanings && wordDetail.onlineMeanings.length > 0 && (
                      <div className="p-4 bg-bg-secondary border border-border rounded-lg">
                        <h3 className="text-xs font-semibold text-green-600 mb-2">🌐 在线释义</h3>
                        {wordDetail.onlineMeanings.map((m, i) => (
                          <div key={i} className="mb-2 last:mb-0">
                            <span className="text-xs text-primary font-semibold">
                              {m.partOfSpeech}
                            </span>
                            {m.definitions.slice(0, 2).map((d, j) => (
                              <p key={j} className="text-sm text-text-primary mt-0.5">
                                • {d.definition}
                              </p>
                            ))}
                          </div>
                        ))}
                      </div>
                    )}

                    {/* AI 生成内容 */}
                    {wordDetail?.aiContent && (
                      <>
                        {wordDetail.aiContent.etymology && (
                          <div className="p-4 bg-bg-tertiary dark:bg-bg-tertiary border border-border dark:border-border rounded-lg">
                            <h3 className="text-xs font-semibold text-primary dark:text-primary mb-2">
                              🔤 词源分析
                            </h3>
                            <p className="text-sm text-text-primary">
                              {wordDetail.aiContent.etymology.origin}
                            </p>
                          </div>
                        )}
                        {wordDetail.aiContent.mnemonics &&
                          wordDetail.aiContent.mnemonics.length > 0 && (
                            <div className="p-4 bg-amber-50 dark:bg-amber-950/20 border border-amber-200 dark:border-amber-800 rounded-lg">
                              <h3 className="text-xs font-semibold text-amber-700 dark:text-amber-400 mb-2">
                                💡 助记法
                              </h3>
                              {wordDetail.aiContent.mnemonics.map((m, i) => (
                                <p key={i} className="text-sm text-text-primary mb-1 last:mb-0">
                                  {m.content}
                                </p>
                              ))}
                            </div>
                          )}
                        {wordDetail.aiContent.examples &&
                          wordDetail.aiContent.examples.length > 0 && (
                            <div className="p-4 bg-green-50 dark:bg-green-950/20 border border-green-200 dark:border-green-800 rounded-lg">
                              <h3 className="text-xs font-semibold text-green-700 dark:text-green-400 mb-2">
                                📝 AI 例句
                              </h3>
                              {wordDetail.aiContent.examples.map((ex, i) => (
                                <div key={i} className="mb-1.5 last:mb-0">
                                  <p className="text-sm text-text-primary italic">{ex.text}</p>
                                  <p className="text-xs text-text-secondary">{ex.context}</p>
                                </div>
                              ))}
                            </div>
                          )}
                      </>
                    )}

                    {/* AI 内容加载/配置提示 */}
                    {!wordDetail?.aiContent && (
                      <div className="p-4 bg-amber-50 dark:bg-amber-950/20 border border-amber-200 dark:border-amber-800 rounded-lg text-center">
                        <Sparkles size={20} className="inline text-amber-500 mb-1 animate-pulse" />
                        <p className="text-xs text-text-secondary mt-1">
                          AI 助记法、词根分析、个性化例句正在生成中...
                        </p>
                        <p className="text-xs text-text-tertiary mt-1">
                          首次加载需要几秒钟，请稍候
                        </p>
                      </div>
                    )}
                  </div>
                )}
              </div>
            </div>

            {/* 操作按钮 */}
            {showAnswer && (
              <div className="border-t border-border p-6">
                <div className="max-w-2xl mx-auto grid grid-cols-2 gap-4">
                  <button
                    onClick={goNext}
                    className="py-3 bg-red-500 text-white rounded-lg hover:bg-red-600 font-medium"
                  >
                    😕 不认识
                  </button>
                  <button
                    onClick={handleKnow}
                    className="py-3 bg-green-500 text-white rounded-lg hover:bg-green-600 font-medium"
                  >
                    😊 认识
                  </button>
                </div>
              </div>
            )}
          </div>
        )}

        {viewMode === 'stats' && (
          <div className="p-6 max-w-2xl">
            <h1 className="ui-page-title mb-6">学习统计</h1>
            <LearningStatsPanel />
          </div>
        )}
      </div>
    </div>
  );
}

function NavBtn({
  active,
  onClick,
  icon,
  label,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  label: string;
}) {
  return (
    <button
      className={`w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm transition-colors ${active ? 'bg-primary text-primary-fg' : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'}`}
      onClick={onClick}
    >
      {icon} {label}
    </button>
  );
}
