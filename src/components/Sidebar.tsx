import type { ReactNode } from 'react';

interface SidebarItemProps {
  icon: ReactNode;
  label: string;
  active?: boolean;
  onClick?: () => void;
}

function SidebarItem({ icon, label, active = false, onClick }: SidebarItemProps) {
  return (
    <button
      onClick={onClick}
      className={`w-full flex items-center gap-3 px-4 py-3 rounded-lg transition-colors ${
        active
          ? 'bg-primary/10 text-primary border border-primary/30'
          : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
      }`}
    >
      <span className="shrink-0">{icon}</span>
      <span className="text-sm font-medium">{label}</span>
    </button>
  );
}

interface SidebarProps {
  items: Array<{
    id: string;
    icon: ReactNode;
    label: string;
  }>;
  activeId: string;
  onChange: (id: string) => void;
}

export default function Sidebar({ items, activeId, onChange }: SidebarProps) {
  return (
    <div className="w-64 bg-bg-secondary border-r border-border flex-shrink-0">
      <nav className="p-4 space-y-1">
        {items.map((item) => (
          <SidebarItem
            key={item.id}
            icon={item.icon}
            label={item.label}
            active={activeId === item.id}
            onClick={() => onChange(item.id)}
          />
        ))}
      </nav>
    </div>
  );
}
