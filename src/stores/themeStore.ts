import { create } from 'zustand';
import { isTauriRuntime } from '../services/tauriRuntime';

type Theme = 'dark' | 'light';

interface ThemeState {
  theme: Theme;
  toggleTheme: () => void;
  setTheme: (theme: Theme) => void;
}

const THEME_EVENT = 'app-theme-changed';

const getInitialTheme = (): Theme => {
  const stored = localStorage.getItem('theme');
  if (stored === 'light' || stored === 'dark') return stored;
  // Default to dark theme
  return 'dark';
};

export const useThemeStore = create<ThemeState>((set) => ({
  theme: getInitialTheme(),

  toggleTheme: () => {
    set((state) => {
      const newTheme = state.theme === 'dark' ? 'light' : 'dark';
      localStorage.setItem('theme', newTheme);
      applyTheme(newTheme, { broadcast: true });
      return { theme: newTheme };
    });
  },

  setTheme: (theme: Theme) => {
    localStorage.setItem('theme', theme);
    applyTheme(theme, { broadcast: true });
    set({ theme });
  },
}));

/**
 * Apply theme class to <html>.
 * `broadcast` = true → also emit a Tauri event so other webview windows
 * (ocr-region-frame, ocr-screenshot) stay in sync. Listener below applies
 * the class WITHOUT broadcasting, so there is no echo loop.
 */
function applyTheme(theme: Theme, opts: { broadcast?: boolean } = {}) {
  applyThemeClass(theme);
  if (opts.broadcast && isTauriRuntime()) {
    void import('@tauri-apps/api/event')
      .then(({ emit }) => emit(THEME_EVENT, theme))
      .catch(() => {
        /* non-tauri / window closing */
      });
  }
  // Keep selection/hover overlay cards in sync (native webview, not DOM)
  void import('../services/invoke')
    .then(({ safeInvoke }) => safeInvoke('set_overlay_theme', { theme }))
    .catch((err: unknown) => {
      console.debug('set_overlay_theme skipped', err);
    });
}

function applyThemeClass(theme: Theme) {
  const root = document.documentElement;
  root.classList.remove('dark', 'light');
  root.classList.add(theme);
}

// Apply theme on load (localStorage is shared across same-origin webview windows
// in WebView2; if it is not, the theme-changed event from main will correct it).
applyThemeClass(getInitialTheme());

// Cross-window theme sync: main window broadcasts, every other window applies.
if (isTauriRuntime()) {
  void import('@tauri-apps/api/event')
    .then(({ listen, emit }) => {
      listen<Theme>(THEME_EVENT, (event) => {
        applyThemeClass(event.payload);
        useThemeStore.setState({ theme: event.payload });
      }).catch(() => undefined);

      // S5-fix: sub-windows (ocr-region-frame, ocr-screenshot) may not share
      // localStorage with the main window in WebView2. On load, they default
      // to 'dark' and miss the earlier theme-changed broadcast. Emit a
      // one-shot request so the main window replies with the current theme.
      // The main window handler is registered in App.tsx.
      void emit('theme-sync-request', null).catch(() => undefined);
      listen<Theme>('theme-sync-reply', (event) => {
        applyThemeClass(event.payload);
        useThemeStore.setState({ theme: event.payload });
      }).catch(() => undefined);
    })
    .catch(() => {
      /* tauri runtime unavailable */
    });
}
