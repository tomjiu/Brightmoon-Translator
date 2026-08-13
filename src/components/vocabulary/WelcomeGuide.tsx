import { useState } from 'react';
import { BookOpen, Brain, Target, ArrowRight, CheckCircle } from 'lucide-react';

interface WelcomeGuideProps {
  onStartLearning: () => void;
  onStartReview: () => void;
}

export function WelcomeGuide({ onStartLearning, onStartReview }: WelcomeGuideProps) {
  const [step, setStep] = useState(0);

  const features = [
    {
      icon: <BookOpen className="w-12 h-12 text-primary" />,
      title: '多源词典查询',
      description: '聚合 ECDICT、有道、Collins 等多个词典，提供全面的单词释义和例句',
    },
    {
      icon: <Brain className="w-12 h-12 text-primary" />,
      title: 'AI 智能助记',
      description: '使用 AI 生成词源分析、联想记忆法和个性化例句，让记忆更高效',
    },
    {
      icon: <Target className="w-12 h-12 text-green-400" />,
      title: 'FSRS 间隔复习',
      description: '基于科学的遗忘曲线算法，智能安排复习时间，长期记忆更牢固',
    },
  ];

  if (step < features.length) {
    const feature = features[step];
    return (
      <div className="h-full flex flex-col items-center justify-center p-8 max-w-lg mx-auto">
        <div className="mb-8">{feature.icon}</div>
        <h2 className="text-2xl font-bold mb-4 text-center">{feature.title}</h2>
        <p className="text-text-secondary text-center mb-8 leading-relaxed">
          {feature.description}
        </p>

        {/* Progress dots */}
        <div className="flex gap-2 mb-8">
          {features.map((_, idx) => (
            <div
              key={idx}
              className={`w-2 h-2 rounded-full transition-colors ${
                idx === step ? 'bg-primary' : 'bg-bg-tertiary'
              }`}
            />
          ))}
        </div>

        <button
          onClick={() => setStep(step + 1)}
          className="flex items-center gap-2 px-6 py-3 bg-primary hover:bg-primary-hover rounded-lg transition-colors"
        >
          {step < features.length - 1 ? '继续' : '开始使用'}
          <ArrowRight className="w-4 h-4" />
        </button>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col items-center justify-center p-8 max-w-lg mx-auto">
      <CheckCircle className="w-16 h-16 text-green-400 mb-6" />
      <h2 className="text-2xl font-bold mb-4 text-center">准备就绪！</h2>
      <p className="text-text-secondary text-center mb-8 leading-relaxed">
        选择一个学习计划开始背单词，或者先体验复习功能
      </p>

      <div className="flex gap-4 w-full">
        <button
          onClick={onStartLearning}
          className="ui-card ui-card-hover flex-1 flex flex-col items-center gap-3 p-6"
        >
          <BookOpen className="w-8 h-8 text-primary" />
          <span className="font-semibold">创建学习计划</span>
          <span className="text-xs text-text-secondary">选择考试词表，系统化学习</span>
        </button>

        <button
          onClick={onStartReview}
          className="ui-card ui-card-hover flex-1 flex flex-col items-center gap-3 p-6"
        >
          <Target className="w-8 h-8 text-green-400" />
          <span className="font-semibold">查询单词</span>
          <span className="text-xs text-text-secondary">先查词了解功能</span>
        </button>
      </div>
    </div>
  );
}
