import { create } from 'zustand';

type Theme = 'dark' | 'light';

interface ThemeState {
  theme: Theme;
  toggleTheme: () => void;
  setTheme: (theme: Theme) => void;
}

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
      applyTheme(newTheme);
      return { theme: newTheme };
    });
  },

  setTheme: (theme: Theme) => {
    localStorage.setItem('theme', theme);
    applyTheme(theme);
    set({ theme });
  },
}));

function applyTheme(theme: Theme) {
  const root = document.documentElement;
  root.classList.remove('dark', 'light');
  root.classList.add(theme);
  // Keep selection/hover overlay cards in sync (native webview, not DOM)
  void import('../services/invoke')
    .then(({ safeInvoke }) => safeInvoke('set_overlay_theme', { theme }))
    .catch((err: unknown) => {
      console.debug('set_overlay_theme skipped', err);
    });
}

// Apply theme on load
applyTheme(getInitialTheme());
