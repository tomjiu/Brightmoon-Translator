// CardDetail - 卡牌详情组件

import { useCard, useGenerateCardContent } from '../../hooks/useVocabulary';
import {
  type Mnemonic,
  type PersonalizedExample,
  type LearningPhase,
  LearningPhase as LearningPhaseEnum,
  getPhaseDisplayText,
  getPhaseColorClass,
  formatTimestamp,
} from '../../services/vocabulary';

// 从 CardState 推断学习阶段
function inferPhase(fsrsState: { reps: number; stability: number }): LearningPhase {
  if (fsrsState.reps === 0) return LearningPhaseEnum.New;
  if (fsrsState.stability > 21) return LearningPhaseEnum.Mastered;
  return LearningPhaseEnum.Review;
}

interface CardDetailProps {
  cardId: string | null;
  onClose?: () => void;
}

export function CardDetail({ cardId, onClose }: CardDetailProps) {
  const { data: card, isLoading } = useCard(cardId);
  const generateContent = useGenerateCardContent();

  if (!cardId) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500">
        选择一个单词查看详情
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-gray-500">加载中...</div>
      </div>
    );
  }

  if (!card) {
    return <div className="flex items-center justify-center h-full text-gray-500">卡牌不存在</div>;
  }

  const handleGenerateContent = () => {
    if (cardId) {
      generateContent.mutate(cardId);
    }
  };

  return (
    <div className="h-full flex flex-col bg-white">
      {/* 头部 */}
      <div className="p-6 border-b">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-xl font-semibold tracking-tight mb-2">{card.word}</h1>
            <div className="flex items-center gap-2">
              <span
                className={`text-xs px-2 py-1 rounded ${getPhaseColorClass(inferPhase(card.fsrs_state))}`}
              >
                {getPhaseDisplayText(inferPhase(card.fsrs_state))}
              </span>
              {card.base_data.phonetic && (
                <span className="text-sm text-gray-600">/{card.base_data.phonetic}/</span>
              )}
              {card.base_data.part_of_speech && (
                <span className="text-sm text-gray-500">{card.base_data.part_of_speech}</span>
              )}
            </div>
          </div>
          {onClose && (
            <button
              onClick={onClose}
              className="text-gray-500 hover:text-gray-700"
              aria-label="关闭"
            >
              ✕
            </button>
          )}
        </div>
      </div>

      {/* 内容 */}
      <div className="flex-1 overflow-y-auto p-6 space-y-6">
        {/* 基础信息 */}
        <section>
          <h2 className="text-lg font-semibold mb-3">基础信息</h2>
          <div className="space-y-2">
            {card.base_data.definitions.length > 0 && (
              <div>
                <h3 className="text-sm font-medium text-gray-700 mb-1">释义</h3>
                <ul className="list-disc list-inside space-y-1">
                  {card.base_data.definitions.map((def, i) => (
                    <li key={i} className="text-gray-600">
                      {def}
                    </li>
                  ))}
                </ul>
              </div>
            )}
            {card.base_data.translation && (
              <div>
                <h3 className="text-sm font-medium text-gray-700 mb-1">中文翻译</h3>
                <p className="text-gray-600">{card.base_data.translation}</p>
              </div>
            )}
          </div>
        </section>

        {/* AI 生成的内容 */}
        {card.ai_content ? (
          <>
            {/* 词源 */}
            {card.ai_content.etymology && (
              <section>
                <h2 className="text-lg font-semibold mb-3">词源</h2>
                <div className="bg-bg-tertiary p-4 rounded-lg">
                  <p className="text-gray-700 mb-3">{card.ai_content.etymology.origin}</p>
                  {card.ai_content.etymology.root_breakdown.length > 0 && (
                    <div className="space-y-2">
                      <h3 className="text-sm font-medium text-gray-700">词根拆解</h3>
                      {card.ai_content.etymology.root_breakdown.map((root, i) => (
                        <div key={i} className="bg-white p-2 rounded">
                          <span className="font-medium text-primary">{root.part}</span>
                          <span className="text-gray-600"> - {root.meaning}</span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </section>
            )}

            {/* 助记法 */}
            {card.ai_content.mnemonics.length > 0 && (
              <section>
                <h2 className="text-lg font-semibold mb-3">助记法</h2>
                <div className="space-y-3">
                  {card.ai_content.mnemonics.map((mnemonic, i) => (
                    <MnemonicCard key={i} mnemonic={mnemonic} />
                  ))}
                </div>
              </section>
            )}

            {/* 例句 */}
            {card.ai_content.examples.length > 0 && (
              <section>
                <h2 className="text-lg font-semibold mb-3">例句</h2>
                <div className="space-y-3">
                  {card.ai_content.examples.map((example, i) => (
                    <ExampleCard key={i} example={example} />
                  ))}
                </div>
              </section>
            )}

            {/* 常见搭配 */}
            {card.ai_content.collocations && card.ai_content.collocations.length > 0 && (
              <section>
                <h2 className="text-lg font-semibold mb-3">常见搭配</h2>
                <div className="flex flex-wrap gap-2">
                  {card.ai_content.collocations.map((collocation, i) => (
                    <span
                      key={i}
                      className="px-3 py-1.5 bg-green-50 text-green-700 rounded-full text-sm"
                    >
                      {collocation}
                    </span>
                  ))}
                </div>
              </section>
            )}

            {/* 词族 */}
            {card.ai_content.word_family && card.ai_content.word_family.length > 0 && (
              <section>
                <h2 className="text-lg font-semibold mb-3">词族</h2>
                <div className="grid grid-cols-2 gap-2">
                  {card.ai_content.word_family.map((item, i) => (
                    <div key={i} className="flex items-center gap-2 p-2 bg-bg-tertiary rounded">
                      <span className="font-medium text-primary">{item.word}</span>
                      <span className="text-xs text-primary">({item.pos})</span>
                      <span className="text-xs text-gray-500">{item.meaning}</span>
                    </div>
                  ))}
                </div>
              </section>
            )}

            {/* 用法提示 */}
            {card.ai_content.usage_tips && card.ai_content.usage_tips.length > 0 && (
              <section>
                <h2 className="text-lg font-semibold mb-3">用法提示</h2>
                <ul className="space-y-2">
                  {card.ai_content.usage_tips.map((tip, i) => (
                    <li key={i} className="flex items-start gap-2 text-sm text-gray-600">
                      <span className="text-primary mt-1">💡</span>
                      <span>{tip}</span>
                    </li>
                  ))}
                </ul>
              </section>
            )}

            {/* 常见错误 */}
            {card.ai_content.common_mistakes && card.ai_content.common_mistakes.length > 0 && (
              <section>
                <h2 className="text-lg font-semibold mb-3">常见错误</h2>
                <ul className="space-y-2">
                  {card.ai_content.common_mistakes.map((mistake, i) => (
                    <li
                      key={i}
                      className="flex items-start gap-2 text-sm text-red-600 bg-red-50 p-2 rounded"
                    >
                      <span className="mt-1">⚠️</span>
                      <span>{mistake}</span>
                    </li>
                  ))}
                </ul>
              </section>
            )}

            {/* 近义词/反义词 */}
            {((card.ai_content.synonyms && card.ai_content.synonyms.length > 0) ||
              (card.ai_content.antonyms && card.ai_content.antonyms.length > 0)) && (
              <section>
                <h2 className="text-lg font-semibold mb-3">近义词 / 反义词</h2>
                <div className="grid grid-cols-2 gap-4">
                  {card.ai_content.synonyms && card.ai_content.synonyms.length > 0 && (
                    <div>
                      <h3 className="text-sm font-medium text-gray-700 mb-2">近义词</h3>
                      <div className="flex flex-wrap gap-1.5">
                        {card.ai_content.synonyms.map((word, i) => (
                          <span
                            key={i}
                            className="px-2 py-1 bg-bg-tertiary text-primary rounded text-sm"
                          >
                            {word}
                          </span>
                        ))}
                      </div>
                    </div>
                  )}
                  {card.ai_content.antonyms && card.ai_content.antonyms.length > 0 && (
                    <div>
                      <h3 className="text-sm font-medium text-gray-700 mb-2">反义词</h3>
                      <div className="flex flex-wrap gap-1.5">
                        {card.ai_content.antonyms.map((word, i) => (
                          <span
                            key={i}
                            className="px-2 py-1 bg-orange-50 text-orange-600 rounded text-sm"
                          >
                            {word}
                          </span>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              </section>
            )}
          </>
        ) : (
          <div className="text-center py-8">
            <p className="text-gray-500 mb-4">暂无 AI 生成的学习内容</p>
            <button
              onClick={handleGenerateContent}
              disabled={generateContent.isPending}
              className="px-6 py-2 bg-primary text-primary-fg rounded-lg hover:bg-primary-hover disabled:opacity-50"
            >
              {generateContent.isPending ? '生成中...' : 'AI 生成学习内容'}
            </button>
          </div>
        )}

        {/* FSRS 状态 */}
        <section>
          <h2 className="text-lg font-semibold mb-3">学习进度</h2>
          <div className="bg-gray-50 p-4 rounded-lg space-y-2 text-sm">
            <div className="flex justify-between">
              <span className="text-gray-600">复习次数</span>
              <span className="font-medium">{card.fsrs_state.reps} 次</span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-600">遗忘次数</span>
              <span className="font-medium">{card.fsrs_state.lapses} 次</span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-600">记忆稳定性</span>
              <span className="font-medium">{card.fsrs_state.stability.toFixed(1)} 天</span>
            </div>
            <div className="flex justify-between">
              <span className="text-gray-600">难度</span>
              <span className="font-medium">{card.fsrs_state.difficulty.toFixed(2)}</span>
            </div>
            {card.fsrs_state.last_review > 0 && (
              <div className="flex justify-between">
                <span className="text-gray-600">上次复习</span>
                <span className="font-medium">{formatTimestamp(card.fsrs_state.last_review)}</span>
              </div>
            )}
            <div className="flex justify-between">
              <span className="text-gray-600">下次复习</span>
              <span className="font-medium">{formatTimestamp(card.fsrs_state.next_review)}</span>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}

function MnemonicCard({ mnemonic }: { mnemonic: Mnemonic }) {
  const typeLabels = {
    etymology: '词源',
    scene: '场景',
    homophone: '谐音',
    visual: '视觉',
    chunking: '拆分',
    comparison: '对比',
  };

  return (
    <div className="bg-yellow-50 p-4 rounded-lg">
      <div className="flex items-center gap-2 mb-2">
        <span className="text-xs px-2 py-1 bg-yellow-200 text-yellow-800 rounded">
          {typeLabels[mnemonic.mnemonic_type]}
        </span>
        {mnemonic.score && (
          <span className="text-xs text-gray-500">评分: {mnemonic.score.toFixed(1)}</span>
        )}
      </div>
      <p className="text-gray-700">{mnemonic.content}</p>
    </div>
  );
}

function ExampleCard({ example }: { example: PersonalizedExample }) {
  return (
    <div className="bg-green-50 p-4 rounded-lg">
      <p className="text-gray-700 mb-2">{example.text}</p>
      <div className="flex items-center justify-between text-sm">
        <span className="text-gray-500">{example.context}</span>
        <span className="text-xs px-2 py-1 bg-green-200 text-green-800 rounded">
          {example.difficulty}
        </span>
      </div>
    </div>
  );
}
