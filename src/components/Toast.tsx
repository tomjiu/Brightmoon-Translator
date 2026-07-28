import { useToastStore, type ToastType } from '../stores/toastStore';
import { AlertCircle, CheckCircle, AlertTriangle, Info, X } from 'lucide-react';

const iconMap: Record<ToastType, typeof AlertCircle> = {
  error: AlertCircle,
  success: CheckCircle,
  warning: AlertTriangle,
  info: Info,
};

const toneMap: Record<ToastType, string> = {
  error: 'border-border-strong',
  success: 'border-border',
  warning: 'border-border-strong',
  info: 'border-border',
};

export default function ToastContainer() {
  const { toasts, removeToast } = useToastStore();

  if (toasts.length === 0) return null;

  return (
    <div className="fixed top-12 right-4 z-[40] flex flex-col gap-2 max-w-sm pointer-events-none [&>*]:pointer-events-auto">
      {toasts.map((toast) => {
        const Icon = iconMap[toast.type];
        return (
          <div
            key={toast.id}
            className={`bg-bg-secondary text-text-primary px-4 py-3 rounded-xl shadow-elevated border ${toneMap[toast.type]} flex items-start gap-3 animate-slide-in backdrop-blur-sm`}
          >
            <Icon size={18} className="shrink-0 mt-0.5 text-text-secondary" />
            <div className="flex-1 min-w-0">
              <div className="text-sm font-medium tracking-tight">{toast.message}</div>
              {toast.detail && (
                <div className="text-xs text-text-secondary mt-1 truncate">{toast.detail}</div>
              )}
            </div>
            <button
              onClick={() => removeToast(toast.id)}
              className="shrink-0 text-text-secondary hover:text-text-primary transition-colors"
              aria-label="dismiss"
            >
              <X size={14} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
