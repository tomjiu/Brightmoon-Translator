import type { ReactNode } from 'react';

export interface SidebarItemData {
  id: string;
  icon: ReactNode;
  label: string;
  group?: string;
}

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
      className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg transition-colors duration-150 ease-out ${
        active
          ? 'bg-primary text-primary-fg shadow-sm'
          : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
      }`}
    >
      <span className="shrink-0 opacity-90">{icon}</span>
      <span className="text-[13px] font-medium tracking-tight">{label}</span>
    </button>
  );
}

interface SidebarProps {
  items: SidebarItemData[];
  activeId: string;
  onChange: (id: string) => void;
  groups?: Array<{ key: string; label: string }>;
}

export default function Sidebar({ items, activeId, onChange, groups }: SidebarProps) {
  const renderItems = (list: SidebarItemData[]) =>
    list.map((item) => (
      <SidebarItem
        key={item.id}
        icon={item.icon}
        label={item.label}
        active={activeId === item.id}
        onClick={() => onChange(item.id)}
      />
    ));

  return (
    <div className="w-56 bg-bg-chrome border-r border-border flex-shrink-0 overflow-y-auto">
      <nav className="p-3 space-y-4">
        {groups && groups.length > 0
          ? groups.map((g) => {
              const groupItems = items.filter((i) => i.group === g.key);
              if (groupItems.length === 0) return null;
              return (
                <div key={g.key}>
                  <div className="px-3 mb-1.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-text-secondary/80">
                    {g.label}
                  </div>
                  <div className="space-y-0.5">{renderItems(groupItems)}</div>
                </div>
              );
            })
          : renderItems(items)}
      </nav>
    </div>
  );
}
