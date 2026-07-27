import type { ButtonHTMLAttributes, ReactNode } from 'react';

type Variant = 'primary' | 'secondary' | 'ghost' | 'danger';
type Size = 'sm' | 'md' | 'icon';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  children: ReactNode;
}

const variants: Record<Variant, string> = {
  primary: 'bg-primary text-primary-fg hover:bg-primary-hover border border-transparent shadow-sm',
  secondary:
    'bg-bg-tertiary text-text-primary border border-border hover:border-border-strong hover:bg-bg-secondary',
  ghost:
    'bg-transparent text-text-secondary border border-transparent hover:bg-bg-tertiary hover:text-text-primary',
  danger:
    'bg-bg-tertiary text-text-primary border border-border-strong hover:bg-primary hover:text-primary-fg',
};

const sizes: Record<Size, string> = {
  sm: 'h-8 px-3 text-xs rounded-md gap-1.5',
  md: 'h-9 px-4 text-sm rounded-lg gap-2',
  icon: 'h-9 w-9 rounded-lg justify-center',
};

export default function Button({
  variant = 'secondary',
  size = 'md',
  className = '',
  disabled,
  children,
  type = 'button',
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      disabled={disabled}
      className={`inline-flex items-center font-medium transition-colors duration-150 ease-out disabled:opacity-40 disabled:pointer-events-none ${variants[variant]} ${sizes[size]} ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
