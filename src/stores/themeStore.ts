import { create } from 'zustand';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { isTauriRuntime } from '../services/tauriRuntime';

type Theme = 'dark' | 'light' | 'dev' | 'dev-light';

/**
 * Two theme families, each with its own dark/light variant:
 *   mono  → 'dark' / 'light'                    (黑白)
 *   dev   → 'dev'  / 'dev-light'                (月球,light 为白色月面)
 * The sidebar sun/moon button flips the shade INSIDE the current family.
 */
export const isDarkTheme = (t: Theme) => t === 'dark' || t === 'dev';
export const themeFamily = (t: Theme): 'mono' | 'dev' =>
  t === 'dev' || t === 'dev-light' ? 'dev' : 'mono';

/** Return the theme for a family at the given shade, or at the theme's own shade. */
const familyTheme = (family: 'mono' | 'dev', dark: boolean) => {
  if (family === 'dev') return dark ? 'dev' : 'dev-light';
  return dark ? 'dark' : 'light';
};

interface ThemeState {
  theme: Theme;
  toggleTheme: () => void;
  setTheme: (theme: Theme) => void;
}

const THEME_EVENT = 'app-theme-changed';

const themeDbg = (msg: string) => {
  console.error(`[T-DBG] ${msg}`);
  if (isTauriRuntime()) {
    void import('@tauri-apps/api/event')
      .then(({ emit }) => emit('__theme_dbg', msg))
      .catch(() => undefined);
  }
};

const getW = () => {
  try {
    return getCurrentWindow().label;
  } catch {
    return 'browser';
  }
};

const getInitialTheme = (): Theme => {
  const stored = localStorage.getItem('theme');
  if (stored === 'light' || stored === 'dark' || stored === 'dev' || stored === 'dev-light') return stored;
  // Default to the lunar dev theme
  return 'dev';
};

export const useThemeStore = create<ThemeState>((set) => ({
  theme: getInitialTheme(),

  toggleTheme: () => {
    themeDbg(`toggleTheme called, stack=${new Error().stack?.split('\n').slice(1, 4).join(' <- ') ?? '?'}`);
    set((state) => {
      // Sidebar sun/moon toggles the shade INSIDE the current family
      // (黑白 dark↔light / 月球 dev↔dev-light), never between families.
      const family = themeFamily(state.theme);
      const dark = isDarkTheme(state.theme);
      const newTheme = familyTheme(family, !dark);
      localStorage.setItem('theme', newTheme);
      applyTheme(newTheme, { broadcast: true });
      return { theme: newTheme };
    });
  },

  setTheme: (theme: Theme) => {
    themeDbg(`setTheme(${theme}), stack=${new Error().stack?.split('\n').slice(1, 4).join(' <- ') ?? '?'}`);
    localStorage.setItem('theme', theme);
    applyTheme(theme, { broadcast: true });
    set({ theme });
  },
}));

/**
 * Keep the native selection/hover overlay cards (Rust `OVERLAY_LIGHT`) in sync
 * with the DOM theme. **Must only be called from the main window**: the static
 * is process-global, so a sub-window syncing its own (possibly dark-defaulted)
 * theme would flip the shared overlay cards to dark even on a light session.
 */
async function overlayIsOnMain(): Promise<boolean> {
  if (!isTauriRuntime()) return false;
  try {
    return getCurrentWindow().label === 'main';
  } catch {
    return false;
  }
}

async function syncOverlayTheme(theme: Theme) {
  themeDbg(`syncOverlayTheme(${theme}) isMain=${await overlayIsOnMain()}`);
  if (!(await overlayIsOnMain())) return;
  const { safeInvoke } = await import('../services/invoke');
  // The native overlay only knows dark/light; dev / dev-light map by shade.
  await safeInvoke('set_overlay_theme', { theme: isDarkTheme(theme) ? 'dark' : 'light' });
  themeDbg(`set_overlay_theme invoked with ${theme}`);
}

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
  // Keep selection/hover overlay cards in sync (native webview, not DOM).
  // Only the main window pushes the global process-level value; sub-windows
  // are corrected by the cross-window sync below.
  void syncOverlayTheme(theme);
}

function applyThemeClass(theme: Theme) {
  const root = document.documentElement;
  root.classList.remove('dark', 'light', 'dev', 'dev-light');
  if (theme === 'dev') {
    // dev is a dark lunar variant: keep `.dark` alive so the shared
    // `.dark …` status-color mappers in index.css still apply, and toggle
    // the extra `.dev` class that overrides the token palette.
    root.classList.add('dark', 'dev');
  } else if (theme === 'dev-light') {
    // dev-light is the WHITE lunar variant: same trick on the light side.
    root.classList.add('light', 'dev-light');
  } else {
    root.classList.add(theme);
  }
  themeDbg(`applyThemeClass(${theme}) in ${getW()}`);
}

// Apply theme on load (localStorage is shared across same-origin webview windows
// in WebView2; if it is not, the theme-changed event from main will correct it).
applyThemeClass(getInitialTheme());

// bug1 fix: push the persisted theme to Rust at startup too, so the native
// overlay cards match the light session without requiring a manual re-toggle.
if (isTauriRuntime()) {
  void syncOverlayTheme(getInitialTheme());
}

// Cross-window theme sync: main window broadcasts, every other window applies.
if (isTauriRuntime()) {
  void import('@tauri-apps/api/event')
    .then(({ listen, emit }) => {
      listen<Theme>(THEME_EVENT, (event) => {
        themeDbg(`THEME_EVENT listener payload=${event.payload} in ${getW()}`);
        applyThemeClass(event.payload);
        useThemeStore.setState({ theme: event.payload });
      }).catch(() => undefined);

      // S5-fix: sub-windows (ocr-region-frame, ocr-screenshot) may not share
      // localStorage with the main window in WebView2. On load, they default
      // to 'dark' and miss the earlier theme-changed broadcast. Emit a
      // one-shot request so the main window replies with the current theme.
      // The main window handler is registered in App.tsx.
      // The main window itself never requests — it is the authoritative source.
      if (getW() !== 'main') {
        void emit('theme-sync-request', null).catch(() => undefined);
      }
      // The reply is meant for sub-windows ONLY. The main window must never
      // apply it: it both emits replies (App.tsx) and would receive its own
      // echoed broadcast here, so a stale/duplicate reply could overwrite the
      // authoritative theme (observed: light session flipping back to dark).
      listen<Theme>('theme-sync-reply', (event) => {
        if (getW() === 'main') return;
        themeDbg(`theme-sync-reply listener payload=${event.payload} in ${getW()}`);
        applyThemeClass(event.payload);
        useThemeStore.setState({ theme: event.payload });
      }).catch(() => undefined);
    })
    .catch(() => {
      /* tauri runtime unavailable */
    });
}
