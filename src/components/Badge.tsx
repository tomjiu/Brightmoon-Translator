interface BadgeProps {
  children: React.ReactNode;
  variant?: 'default' | 'success' | 'warning' | 'error' | 'info';
  className?: string;
}

export default function Badge({ children, variant = 'default', className = '' }: BadgeProps) {
  const variants = {
    default: 'bg-bg-tertiary text-text-secondary border-border',
    success: 'bg-bg-tertiary text-text-primary border-border-strong',
    warning: 'bg-bg-tertiary text-text-secondary border-border-strong',
    error: 'bg-primary/10 text-text-primary border-border-strong',
    info: 'bg-bg-tertiary text-text-primary border-border',
  };

  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-medium tracking-wide border ${variants[variant]} ${className}`}
    >
      {children}
    </span>
  );
}
