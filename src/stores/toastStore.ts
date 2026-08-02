import { create } from 'zustand';

export type ToastType = 'info' | 'success' | 'warning' | 'error';

export interface Toast {
  id: string;
  type: ToastType;
  message: string;
  detail?: string;
  duration: number;
}

interface ToastState {
  toasts: Toast[];
  addToast: (toast: Omit<Toast, 'id'>) => void;
  removeToast: (id: string) => void;
  clearAll: () => void;
}

let toastCounter = 0;

// S3-14: track per-toast timeout handles so removeToast/clearAll can clear
// orphan timers. Previously the setTimeout in addToast was fire-and-forget —
// if the user dismissed a toast (or clearAll ran) before the timer fired, the
// orphan callback still executed and filtered the (already-absent) id. This
// was harmless functionally but leaked timer slots and could race with a
// reused id.
const toastTimers = new Map<string, ReturnType<typeof setTimeout>>();

function clearToastTimer(id: string) {
  const handle = toastTimers.get(id);
  if (handle !== undefined) {
    clearTimeout(handle);
    toastTimers.delete(id);
  }
}

export const useToastStore = create<ToastState>((set) => ({
  toasts: [],

  addToast: (toast) => {
    const id = `toast-${++toastCounter}`;
    const newToast: Toast = { ...toast, id };
    set((state) => ({ toasts: [...state.toasts, newToast] }));

    // Auto-remove after duration
    const handle = setTimeout(() => {
      toastTimers.delete(id);
      set((state) => ({
        toasts: state.toasts.filter((t) => t.id !== id),
      }));
    }, toast.duration);
    toastTimers.set(id, handle);
  },

  removeToast: (id) => {
    clearToastTimer(id);
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }));
  },

  clearAll: () => {
    for (const id of toastTimers.keys()) {
      const handle = toastTimers.get(id);
      if (handle !== undefined) clearTimeout(handle);
    }
    toastTimers.clear();
    set({ toasts: [] });
  },
}));
