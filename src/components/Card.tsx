import type { ReactNode } from 'react';

interface CardProps {
  title?: string;
  description?: string;
  children: ReactNode;
  className?: string;
}

export default function Card({ title, description, children, className = '' }: CardProps) {
  return (
    <div className={`bg-bg-secondary border border-border rounded-xl p-6 shadow-sm ${className}`}>
      {title && (
        <div className="mb-4">
          <h3 className="text-base font-semibold text-text-primary">{title}</h3>
          {description && <p className="text-xs text-text-secondary mt-1">{description}</p>}
        </div>
      )}
      {children}
    </div>
  );
}
