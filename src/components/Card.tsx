import type { ReactNode } from 'react';

interface CardProps {
  title?: string;
  description?: string;
  children: ReactNode;
  className?: string;
}

export default function Card({ title, description, children, className = '' }: CardProps) {
  return (
    <div
      className={`ui-card ui-card-hover p-5 ${className}`}
    >
      {title && (
        <div className="mb-4 pb-3 border-b border-border">
          <h3 className="ui-section-title">{title}</h3>
          {description && <p className="ui-page-desc leading-relaxed">{description}</p>}
        </div>
      )}
      {children}
    </div>
  );
}
