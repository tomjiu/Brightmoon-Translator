import { useCallback, useState, useEffect, lazy, Suspense, useMemo } from 'react';
import { listen } from '@tauri-apps/api/event';
import { safeInvoke, invokeOrThrow } from './services/invoke';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { isTauriRuntime } from './services/tauriRuntime';
const MainTranslator = lazy(() => import('./pages/MainTranslator'));
const Settings = lazy(() => import('./pages/Settings'));
const DocumentsViewer = lazy(() => import('./pages/DocumentsViewer'));
const Vocabulary = lazy(() => import('./pages/Vocabulary'));
const MetricsDashboard = lazy(() => import('./pages/MetricsDashboard'));
const TmManager = lazy(() => import('./pages/TmManager'));
const HookMonitor = lazy(() => import('./components/HookMonitor'));
import ErrorBoundary from './components/ErrorBoundary';
import OcrScreenshotSelector from './components/OcrScreenshotSelector';
import OcrRegionFrame from './components/OcrRegionFrame';
import OcrScreenshotTranslator from './components/OcrScreenshotTranslator';
import TitleBar from './components/TitleBar';
import { AIGenerationProgress } from './components/vocabulary';
import { useThemeStore } from './stores/themeStore';
import { useToastStore } from './stores/toastStore';
import { useConfigStore } from './stores/configStore';
import { useTranslateStore } from './stores/translateStore';
import ToastContainer from './components/Toast';
import { useI18n } from './i18n';
import {
  Languages,
  Settings as SettingsIcon,
  Sun,
  Moon,
  Pin,
  FileText,
  Zap,
  BookOpen,
  BarChart3,
  Database,
  Loader2,
} from 'lucide-react';

type Page = 'translator' | 'hook' | 'documents' | 'vocabulary' | 'metrics' | 'tm' | 'settings';

interface NavItem {
  id: Page;
  icon: typeof Languages;
  label: string;
  group: 'core' | 'read' | 'data' | 'system';
}

const windowMode = new URLSearchParams(window.location.search).get('window');

function App() {
  if (windowMode === 'ocr-screenshot') {
    return <OcrScreenshotSelector />;
  }
  if (windowMode === 'ocr-region-frame') {
    return <OcrRegionFrame />;
  }

  return <MainApp />;
}

function MainApp() {
  const [page, setPage] = useState<Page>('translator');
  const [pinned, setPinned] = useState(false);
  const [ocrLaunchNonce, setOcrLaunchNonce] = useState(0);
  const { theme, toggleTheme } = useThemeStore();
  const addToast = useToastStore((s) => s.addToast);
  const loadConfig = useConfigStore((s) => s.loadConfig);
  const configLoaded = useConfigStore((s) => s.loaded);
  const clipboardMonitorPref = useConfigStore((s) => s.config.clipboardMonitor);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);
  const setClipboardMonitorChangeHandler = useTranslateStore(
    (s) => s.setClipboardMonitorChangeHandler,
  );
  const syncClipboardMonitorFromConfig = useTranslateStore((s) => s.syncClipboardMonitorFromConfig);
  const { t } = useI18n();
  const isTauri = isTauriRuntime();

  // Load saved config from disk (not mere defaults) before any settings save
  useEffect(() => {
    if (!isTauri) return;
    loadConfig();
  }, [isTauri, loadConfig]);

  // Persist MainTranslator clipboard toggle into config.clipboardMonitor
  useEffect(() => {
    if (!isTauri) return;
    setClipboardMonitorChangeHandler((enabled) => {
      updateConfig((prev) => ({ ...prev, clipboardMonitor: enabled }));
      void saveConfig();
    });
    return () => setClipboardMonitorChangeHandler(null);
  }, [isTauri, setClipboardMonitorChangeHandler, updateConfig, saveConfig]);

  // Honor config.clipboardMonitor after load (and when settings toggle changes)
  useEffect(() => {
    if (!isTauri || !configLoaded) return;
    void syncClipboardMonitorFromConfig(!!clipboardMonitorPref);
  }, [isTauri, configLoaded, clipboardMonitorPref, syncClipboardMonitorFromConfig]);

  // Tray toggles backend config then emits this so FE store + listener stay in sync
  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    void listen<boolean>('clipboard-monitor-toggled', (e) => {
      const enabled = !!e.payload;
      updateConfig((prev) => ({ ...prev, clipboardMonitor: enabled }));
      void syncClipboardMonitorFromConfig(enabled);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, [isTauri, updateConfig, syncClipboardMonitorFromConfig]);

  const startOcrScreenshot = useCallback(() => {
    setOcrLaunchNonce((n) => n + 1);
  }, []);

  const togglePin = useCallback(async () => {
    if (!isTauri) {
      addToast({
        type: 'info',
        message: t('common.desktopOnly') || 'Desktop-only action',
        duration: 2500,
      });
      return;
    }

    try {
      const result = await invokeOrThrow<boolean>('toggle_always_on_top');
      setPinned(result);
    } catch (err) {
      console.error('Failed to toggle pin:', err);
    }
  }, [addToast, isTauri, t]);

  useEffect(() => {
    if (!isTauri) return;

    // Listen for navigation events from tray
    const unlistenNav = listen<string>('navigate', (event) => {
      const pageMap: Record<string, Page> = {
        settings: 'settings',
        translator: 'translator',
        hook: 'hook',
        documents: 'documents',
        vocabulary: 'vocabulary',
        metrics: 'metrics',
        tm: 'tm',
      };
      if (pageMap[event.payload]) {
        setPage(pageMap[event.payload]);
      }
    });

    const unlistenOcrScreenshot = listen('trigger-ocr-screenshot', () => {
      startOcrScreenshot();
    });

    // Listen for translate-selection shortcut (Ctrl+Shift+Y)
    const unlistenTranslateSelection = listen('trigger-translate-selection', async () => {
      const [_, err] = await safeInvoke('trigger_selection_translate');
      if (err) {
        console.error('Failed to translate selection:', err);
        const msg = err.message;
        if (msg.includes('No text selected')) {
          addToast({ type: 'warning', message: t('selection.noSelection'), duration: 3000 });
        } else {
          addToast({
            type: 'error',
            message: t('selection.translateFailed'),
            detail: msg,
            duration: 5000,
          });
        }
      }
    });

    // Listen for auto-copy events
    const unlistenAutoCopy = listen<string>('auto-copy', async (event) => {
      try {
        await navigator.clipboard.writeText(event.payload);
      } catch (err) {
        console.error('Failed to auto-copy:', err);
      }
    });

    // Listen for replace-translate shortcut (Ctrl+Shift+R)
    // Backend uses SelectionProviderManager to get selection, no frontend clipboard read needed
    const unlistenReplaceTranslate = listen('trigger-replace-translate', async () => {
      const [result, err] = await safeInvoke<{
        original: string;
        replacement: string;
        success: boolean;
        error: string | null;
        fallbackToOverlay: boolean;
      }>('replace_translate');
      if (err) {
        console.error('Failed to replace translate:', err);
        const msg = err.message;
        if (msg.includes('No text selected')) {
          addToast({ type: 'warning', message: t('replace.noSelection'), duration: 3000 });
        } else {
          addToast({ type: 'error', message: t('replace.hardFail'), detail: msg, duration: 5000 });
        }
        return;
      }
      if (result!.success) {
        addToast({ type: 'success', message: t('replace.success'), duration: 2000 });
      } else if (result!.error === 'cancelled') {
        // Second hotkey while replace in-flight: cancel-only, no toast spam
        return;
      } else {
        // Soft failure: clipboard paste failed but translation exists
        const errMsg = result!.error || t('replace.unknownError');
        const isClipboardLocked = errMsg.includes('OpenClipboard') || errMsg.includes('clipboard');
        addToast({
          type: 'warning',
          message: isClipboardLocked ? t('replace.clipboardLocked') : t('replace.softFail'),
          detail: result!.replacement,
          duration: 5000,
        });
        // Show overlay fallback so the user can still see the translation
        if (result!.fallbackToOverlay && result!.replacement) {
          const [cursorPos, cursorErr] = await safeInvoke<[number, number]>('get_cursor_position');
          if (!cursorErr && cursorPos) {
            await invokeOrThrow('update_overlay', {
              x: cursorPos[0] + 20,
              y: cursorPos[1] + 20,
              width: 350,
              height: 200,
              text: result!.replacement,
              source: result!.original,
              showControls: true,
            });
          }
        }
      }
    });

    // Save window position on move/resize
    const appWindow = getCurrentWindow();
    let saveDebounce: ReturnType<typeof setTimeout> | null = null;

    const saveWindowPosition = async () => {
      try {
        const size = await appWindow.outerSize();
        const pos = await appWindow.outerPosition();
        await safeInvoke(
          'save_window_position',
          {
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
          },
          { silent: true },
        );
      } catch (err) {
        // Ignore
      }
    };

    const debouncedSave = () => {
      if (saveDebounce) clearTimeout(saveDebounce);
      saveDebounce = setTimeout(saveWindowPosition, 500);
    };

    const unlistenMoved = appWindow.onMoved(debouncedSave);
    const unlistenResized = appWindow.onResized(debouncedSave);

    return () => {
      if (saveDebounce) clearTimeout(saveDebounce);
      unlistenMoved.then((fn) => fn());
      unlistenResized.then((fn) => fn());
      unlistenNav.then((fn) => fn());
      unlistenOcrScreenshot.then((fn) => fn());
      unlistenTranslateSelection.then((fn) => fn());
      unlistenAutoCopy.then((fn) => fn());
      unlistenReplaceTranslate.then((fn) => fn());
    };
  }, [addToast, isTauri, startOcrScreenshot, t]);

  const navItems: NavItem[] = useMemo(
    () => [
      { id: 'translator', icon: Languages, label: t('nav.translator'), group: 'core' },
      { id: 'hook', icon: Zap, label: t('nav.hook'), group: 'core' },
      { id: 'documents', icon: FileText, label: t('nav.documents'), group: 'core' },
      { id: 'vocabulary', icon: BookOpen, label: t('nav.vocabulary'), group: 'core' },
      { id: 'metrics', icon: BarChart3, label: t('nav.metrics'), group: 'system' },
      { id: 'tm', icon: Database, label: t('nav.tm') || 'TM', group: 'system' },
      { id: 'settings', icon: SettingsIcon, label: t('nav.settings'), group: 'system' },
    ],
    [t],
  );

  // Group nav items for rendering with separators
  const navGroups = useMemo(
    () => [
      { key: 'core', items: navItems.filter((i) => i.group === 'core') },
      { key: 'system', items: navItems.filter((i) => i.group === 'system') },
    ],
    [navItems],
  );

  return (
    <div className="flex flex-col h-screen bg-bg-primary ui-normalize-type">
      <TitleBar />

      <div className="flex flex-1 min-h-0">
        {/* Icon rail — monochrome chrome, no decorative logo */}
        <nav className="w-14 bg-bg-chrome border-r border-border flex flex-col items-center py-3 overflow-y-auto">
          {navGroups.map((group, groupIndex) => (
            <div key={group.key} className="w-full flex flex-col items-center gap-1">
              {groupIndex > 0 && <div className="w-6 h-px bg-border my-2 shrink-0" />}
              {group.items.map((item) => {
                const Icon = item.icon;
                const isActive = page === item.id;

                return (
                  <button
                    key={item.id}
                    className={`w-10 h-10 rounded-xl flex items-center justify-center transition-colors duration-150 ease-out shrink-0 ${
                      isActive
                        ? 'bg-primary text-primary-fg shadow-sm'
                        : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
                    }`}
                    onClick={() => setPage(item.id)}
                    title={item.label}
                  >
                    <Icon size={18} strokeWidth={isActive ? 2.25 : 1.75} />
                  </button>
                );
              })}
            </div>
          ))}

          <div className="flex-1 min-h-2" />

          <div className="w-full flex flex-col items-center gap-1 shrink-0">
            <div className="w-6 h-px bg-border my-2" />

            <button
              className={`w-10 h-10 rounded-xl flex items-center justify-center transition-colors duration-150 ease-out ${
                pinned
                  ? 'bg-primary text-primary-fg shadow-sm'
                  : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
              }`}
              onClick={togglePin}
              title={pinned ? t('common.unpin') : t('common.pin')}
            >
              <Pin size={18} />
            </button>

            <button
              className="w-10 h-10 rounded-xl flex items-center justify-center text-text-secondary hover:bg-bg-tertiary hover:text-text-primary transition-colors duration-150 ease-out"
              onClick={toggleTheme}
              title={theme === 'dark' ? t('common.lightMode') : t('common.darkMode')}
            >
              {theme === 'dark' ? <Sun size={18} /> : <Moon size={18} />}
            </button>
          </div>
        </nav>

        <main className="flex-1 overflow-hidden bg-bg-primary">
          <ErrorBoundary key={page}>
            <Suspense
              fallback={
                <div className="flex-1 flex items-center justify-center h-full">
                  <Loader2 className="w-8 h-8 text-primary animate-spin" />
                </div>
              }
            >
              {page === 'translator' && <MainTranslator onOcrScreenshot={startOcrScreenshot} />}
              {page === 'documents' && <DocumentsViewer />}
              {page === 'vocabulary' && <Vocabulary />}
              {page === 'settings' && <Settings />}
              {page === 'metrics' && <MetricsDashboard />}
              {page === 'tm' && <TmManager />}
              {page === 'hook' && <HookMonitor />}
            </Suspense>
          </ErrorBoundary>
        </main>
      </div>

      <OcrScreenshotTranslator launchNonce={ocrLaunchNonce} />
      <AIGenerationProgress />
      <ToastContainer />
    </div>
  );
}

export default App;
