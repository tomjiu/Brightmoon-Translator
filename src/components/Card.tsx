import type { ReactNode } from 'react';

interface CardProps {
  title?: string;
  description?: string;
  children: ReactNode;
  className?: string;
}

export default function Card({ title, description, children, className = '' }: CardProps) {
  return (
    <div className={`bg-bg-secondary border border-border rounded-lg p-4 shadow-sm ${className}`}>
      {title && (
        <div className="mb-3">
          <h3 className="text-sm font-medium text-text-primary">{title}</h3>
          {description && <p className="text-xs text-text-secondary mt-0.5">{description}</p>}
        </div>
      )}
      {children}
    </div>
  );
}
