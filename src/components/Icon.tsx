import type { LucideIcon, LucideProps } from 'lucide-react';

/** Canonical icon sizes for monochrome UI (lucide only). */
export type IconSize = 'xs' | 'sm' | 'md' | 'lg';

const SIZE_PX: Record<IconSize, number> = {
  xs: 12,
  sm: 14,
  md: 16,
  lg: 18,
};

const STROKE: Record<IconSize, number> = {
  xs: 2,
  sm: 1.75,
  md: 1.75,
  lg: 1.75,
};

export interface IconProps extends Omit<LucideProps, 'size' | 'strokeWidth'> {
  icon: LucideIcon;
  size?: IconSize;
  active?: boolean;
  className?: string;
}

/** Unified lucide wrapper — prefer this over raw size={24} on page chrome. */
export default function Icon({
  icon: Lucide,
  size = 'md',
  active = false,
  className = '',
  ...rest
}: IconProps) {
  const px = SIZE_PX[size];
  const stroke = active ? 2.25 : STROKE[size];
  return (
    <span
      className={`ui-icon ${className}`.trim()}
      aria-hidden={rest['aria-label'] ? undefined : true}
    >
      <Lucide size={px} strokeWidth={stroke} {...rest} />
    </span>
  );
}
