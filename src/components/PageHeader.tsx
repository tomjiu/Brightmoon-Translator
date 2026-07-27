import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import Icon from './Icon';

interface PageHeaderProps {
  title: string;
  description?: string;
  icon?: LucideIcon;
  actions?: ReactNode;
  className?: string;
}

/** Shared page chrome: title scale + optional lucide icon + actions. */
export default function PageHeader({
  title,
  description,
  icon,
  actions,
  className = '',
}: PageHeaderProps) {
  return (
    <div className={`flex items-center justify-between gap-3 mb-5 ${className}`.trim()}>
      <div className="min-w-0 flex items-center gap-2.5">
        {icon && <Icon icon={icon} size="lg" className="text-primary" />}
        <div className="min-w-0">
          <h1 className="ui-page-title truncate">{title}</h1>
          {description && <p className="ui-page-desc">{description}</p>}
        </div>
      </div>
      {actions && (
        <div className="flex items-center gap-2 shrink-0 flex-wrap justify-end">{actions}</div>
      )}
    </div>
  );
}
