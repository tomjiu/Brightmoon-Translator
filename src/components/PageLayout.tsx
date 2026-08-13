import type { ReactNode } from 'react';

type ToolbarVariant = 'tab' | 'header';
type MaxWidth = 'none' | '3xl' | '4xl';

interface PageLayoutProps {
  /** 固定顶栏内容（Tab 栏 / PageHeader+工具栏 / 返回按钮），由各页面自定义 */
  toolbar?: ReactNode;
  /** 顶栏样式：'tab'（紧凑 Tab/返回栏，py-2）| 'header'（PageHeader+工具栏，py-4），默认 'tab' */
  toolbarVariant?: ToolbarVariant;
  /** 内容区限宽档位，仅 scrollable=true 时生效，默认 '4xl' */
  maxWidth?: MaxWidth;
  /** 内容区是否自身滚动（true=overflow-y-auto，false=overflow-hidden 下放子组件），默认 true */
  scrollable?: boolean;
  /** 额外追加到内容区的 className */
  contentClassName?: string;
  children: ReactNode;
}

const MAX_W_CLASS: Record<MaxWidth, string> = {
  none: '',
  '3xl': 'max-w-3xl mx-auto w-full',
  '4xl': 'max-w-4xl mx-auto w-full',
};

/**
 * 统一页面骨架：h-full flex-col + 固定顶栏 + flex-1 内容区。
 *
 * 所有页面用此组件包裹，保证顶栏背景(ui-chrome)、padding、border、
 * 内容区滚动策略、限宽策略的一致性。顶栏放什么由各页面通过 toolbar slot 自定义。
 */
export default function PageLayout({
  toolbar,
  toolbarVariant = 'tab',
  maxWidth = '4xl',
  scrollable = true,
  contentClassName = '',
  children,
}: PageLayoutProps) {
  const toolbarClass = toolbar
    ? [
        'ui-chrome shrink-0 border-b border-border',
        toolbarVariant === 'header' ? 'px-4 md:px-6 py-4' : 'px-4 py-2',
      ].join(' ')
    : '';

  const contentClass = [
    'flex-1 min-h-0',
    scrollable
      ? `overflow-y-auto p-4 md:p-5 lg:p-6 ${MAX_W_CLASS[maxWidth]}`
      : 'overflow-hidden',
    contentClassName,
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div className="h-full flex flex-col">
      {toolbar && <div className={toolbarClass}>{toolbar}</div>}
      <div className={contentClass}>{children}</div>
    </div>
  );
}
