import { AlertCircle, Bot, CheckCircle, Languages } from 'lucide-react';
import Badge from '../../../components/Badge';
import Switch from '../../../components/Switch';

interface EngineCardProps {
  name: string;
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
  status: 'connected' | 'warning' | 'error';
  badges: Array<{ label: string; variant: 'success' | 'warning' | 'error' | 'info' }>;
  description: string;
  children?: React.ReactNode;
  hideToggle?: boolean;
  alwaysShowChildren?: boolean;
}

export default function EngineCard({
  name,
  enabled,
  onToggle,
  status,
  badges,
  description,
  children,
  hideToggle,
  alwaysShowChildren,
}: EngineCardProps) {
  const statusIcons = {
    connected: <CheckCircle size={15} className="text-text-secondary" />,
    warning: <AlertCircle size={15} className="text-text-secondary" />,
    error: <AlertCircle size={15} className="text-text-primary" />,
  };

  const Icon = name.includes('LLM') || name.includes('大模型') ? Bot : Languages;

  return (
    <div className="p-3.5 border border-border rounded-xl bg-bg-secondary">
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-start gap-3 flex-1 min-w-0">
          <div className="w-9 h-9 rounded-lg bg-bg-tertiary border border-border flex items-center justify-center shrink-0 text-text-secondary">
            <Icon size={18} strokeWidth={1.75} />
          </div>

          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 mb-0.5">
              <h4 className="text-sm font-medium tracking-tight text-text-primary">{name}</h4>
              {statusIcons[status]}
            </div>
            <p className="text-xs text-text-secondary mb-2 leading-relaxed">{description}</p>
            <div className="flex flex-wrap gap-1.5">
              {badges.map((badge, idx) => (
                <Badge key={idx} variant={badge.variant}>
                  {badge.label}
                </Badge>
              ))}
            </div>
          </div>
        </div>

        {!hideToggle && <Switch checked={enabled} onChange={onToggle} />}
      </div>

      {(alwaysShowChildren || enabled) && children && (
        <div className="mt-3 pt-3 border-t border-border">{children}</div>
      )}
    </div>
  );
}
