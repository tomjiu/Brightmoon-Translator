import type { ReactNode } from 'react';
import type { LucideIcon } from 'lucide-react';
import PageHeader from './PageHeader';

type ToolbarVariant = 'tab' | 'header';
type MaxWidth = 'none' | '3xl' | '4xl';
type ChromeMode = 'auto' | 'none';

interface TabItem {
  id: string;
  label: string;
  icon?: LucideIcon;
}

interface PageLayoutProps {
  /** 页面标题：传入后自动在内容区顶部渲染 PageHeader（位置全局一致） */
  title?: string;
  description?: string;
  icon?: LucideIcon;
  actions?: ReactNode;
  /** 标准 tab 栏：传入后自动渲染（45px ui-page-bar 等高 chrome 栏） */
  tabs?: TabItem[];
  activeTab?: string;
  onTabChange?: (id: string) => void;
  /**
   * 页面栏模式：
   * - 'auto'（默认）：有 tabs/toolbar 渲染对应栏；都没有则渲染等高空栏占位，
   *   保证无 tab 页面（hook/文档翻译）的标题 y 坐标与 tab 页面一致
   * - 'none'：不渲染页面栏（用于 tab 容器内的子页面，页面栏由父级提供）
   */
  chrome?: ChromeMode;
  /** 自定义顶栏内容（高级用法，优先于 tabs） */
  toolbar?: ReactNode;
  /** 顶栏样式：'tab'（紧凑 Tab/返回栏，py-2）| 'header'（PageHeader+工具栏，py-4），默认 'tab' */
  toolbarVariant?: ToolbarVariant;
  /** 内容区限宽档位（贴左不居中），仅 scrollable=true 时生效，默认 'none' 通栏 */
  maxWidth?: MaxWidth;
  /** 内容区是否自身滚动（true=overflow-y-auto，false=overflow-hidden 下放子组件），默认 true */
  scrollable?: boolean;
  /** 额外追加到内容区的 className */
  contentClassName?: string;
  children: ReactNode;
}

const MAX_W_CLASS: Record<MaxWidth, string> = {
  none: '',
  // 限宽一律贴左（不 mx-auto 居中）：保证标题首字 x 坐标与卡片左边距
  // 不随窗口宽度漂移，全局各页面位置一致
  '3xl': 'max-w-3xl w-full',
  '4xl': 'max-w-4xl w-full',
};

const TAB_BTN_CLASS =
  'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors shrink-0';

/**
 * 统一页面骨架：45px 页面栏（tab / 空占位）+ 内容区（统一 padding/间距）+ 可选 PageHeader。
 *
 * 新页面用法：
 * - 普通页：  <PageLayout title="页面" icon={Icon} actions={...}>内容</PageLayout>
 * - tab 页：  <PageLayout tabs={tabs} activeTab={id} onTabChange={fn} scrollable={false}>...</PageLayout>
 * - tab 子页：<PageLayout chrome="none" title="..." icon={...}>内容</PageLayout>
 *
 * 标题坐标、页面栏高度、卡片边距全部由本组件保证一致，页面内不要再手写
 * h-full overflow-y-auto / p-4 md:p-5 lg:p-6 / space-y-5 骨架。
 */
export default function PageLayout({
  title,
  description,
  icon,
  actions,
  tabs,
  activeTab,
  onTabChange,
  chrome = 'auto',
  toolbar,
  toolbarVariant = 'tab',
  maxWidth = 'none',
  scrollable = true,
  contentClassName = '',
  children,
}: PageLayoutProps) {
  // 页面栏：自定义 toolbar > 标准 tabs > 等高空占位栏
  let chromeBar: ReactNode = null;
  if (chrome !== 'none') {
    if (toolbar) {
      const toolbarClass = [
        'ui-chrome shrink-0 border-b border-border',
        toolbarVariant === 'header'
          ? 'px-4 md:px-6 py-4'
          : 'ui-page-bar flex items-center px-4 py-2',
      ].join(' ');
      chromeBar = <div className={toolbarClass}>{toolbar}</div>;
    } else if (tabs && tabs.length > 0) {
      chromeBar = (
        <div className="ui-chrome ui-page-bar shrink-0 border-b border-border flex items-center gap-1 px-4 py-2 overflow-x-auto">
          {tabs.map(({ id, icon: TabIcon, label }) => (
            <button
              key={id}
              className={`${TAB_BTN_CLASS} ${
                activeTab === id
                  ? 'bg-primary text-primary-fg'
                  : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
              }`}
              onClick={() => onTabChange?.(id)}
            >
              {TabIcon && <TabIcon size={14} />}
              {label}
            </button>
          ))}
        </div>
      );
    } else {
      chromeBar = (
        <div className="ui-chrome ui-page-bar shrink-0 border-b border-border" aria-hidden="true" />
      );
    }
  }

  const header = title ? (
    <PageHeader title={title} description={description} icon={icon} actions={actions} />
  ) : null;

  const contentClass = [
    'flex-1 min-h-0',
    scrollable ? 'overflow-y-auto p-4 md:p-5 lg:p-6' : 'overflow-hidden',
    contentClassName,
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div className="h-full flex flex-col">
      {chromeBar}
      <div className={contentClass}>
        {scrollable ? (
          <div className={`w-full space-y-5 ${MAX_W_CLASS[maxWidth]}`.trim()}>
            {header}
            {children}
          </div>
        ) : (
          <>
            {header}
            {children}
          </>
        )}
      </div>
    </div>
  );
}
