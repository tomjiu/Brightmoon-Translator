import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import MainTranslator from "./pages/MainTranslator";
import Settings from "./pages/Settings";
import History from "./pages/History";
import Glossary from "./pages/Glossary";
import Tools from "./pages/Tools";
import WordBook from "./pages/WordBook";
import PdfViewer from "./pages/PdfViewer";
import EpubViewer from "./pages/EpubViewer";
import SubtitleViewer from "./pages/SubtitleViewer";
import Plugins from "./pages/Plugins";
import OcrMonitor from "./components/OcrMonitor";
import { useThemeStore } from "./stores/themeStore";
import { useTranslateStore } from "./stores/translateStore";
import { useI18n } from "./i18n";
import {
  Languages,
  History as HistoryIcon,
  Settings as SettingsIcon,
  Book,
  Sun,
  Moon,
  Wrench,
  Pin,
  Bookmark,
  FileText,
  BookOpen,
  Puzzle,
  Scan,
  Subtitles,
} from "lucide-react";

type Page = "translator" | "settings" | "history" | "glossary" | "tools" | "wordbook" | "pdf" | "epub" | "subtitle" | "plugins" | "ocr";

interface NavItem {
  id: Page;
  icon: typeof Languages;
  label: string;
  group: "core" | "read" | "data" | "system";
}

function App() {
  const [page, setPage] = useState<Page>("translator");
  const [pinned, setPinned] = useState(false);
  const { theme, toggleTheme } = useThemeStore();
  const { setSourceText } = useTranslateStore();
  const { t } = useI18n();

  const togglePin = async () => {
    try {
      const result = await invoke<boolean>("toggle_always_on_top");
      setPinned(result);
    } catch (err) {
      console.error("Failed to toggle pin:", err);
    }
  };

  useEffect(() => {
    // Listen for navigation events from tray
    const unlistenNav = listen<string>("navigate", (event) => {
      const pageMap: Record<string, Page> = {
        settings: "settings",
        history: "history",
        translator: "translator",
        glossary: "glossary",
        tools: "tools",
        wordbook: "wordbook",
        pdf: "pdf",
        epub: "epub",
        subtitle: "subtitle",
        plugins: "plugins",
        ocr: "ocr",
      };
      if (pageMap[event.payload]) {
        setPage(pageMap[event.payload]);
      }
    });

    // Listen for translate-selection shortcut (Ctrl+Shift+Y)
    const unlistenTranslateSelection = listen("trigger-translate-selection", async () => {
      try {
        // Unified pipeline: selection provider → translate → overlay
        await invoke("trigger_selection_translate");
      } catch (err) {
        console.error("Failed to translate selection:", err);
      }
    });

    // Listen for auto-copy events
    const unlistenAutoCopy = listen<string>("auto-copy", async (event) => {
      try {
        await navigator.clipboard.writeText(event.payload);
      } catch (err) {
        console.error("Failed to auto-copy:", err);
      }
    });

    // Listen for replace-typing events (replace translate feature)
    const unlistenReplaceTyping = listen<string>("replace-typing", async (event) => {
      try {
        // Use the new replace_text_in_app command to paste text into the active application
        await invoke("replace_text_in_app", { text: event.payload });
      } catch (err) {
        console.error("Failed to replace typing:", err);
      }
    });

    // Listen for replace-translate shortcut (Ctrl+Shift+R)
    const unlistenReplaceTranslate = listen("trigger-replace-translate", async () => {
      try {
        // Read clipboard text (the selected text should be copied first)
        const text = await navigator.clipboard.readText();
        if (text && text.trim()) {
          // Call replace_translate command
          await invoke("replace_translate", { text: text.trim() });
        }
      } catch (err) {
        console.error("Failed to replace translate:", err);
      }
    });

    // Save window position on move/resize
    const appWindow = getCurrentWindow();
    let saveDebounce: ReturnType<typeof setTimeout> | null = null;

    const saveWindowPosition = async () => {
      try {
        const size = await appWindow.outerSize();
        const pos = await appWindow.outerPosition();
        await invoke("save_window_position", {
          x: pos.x,
          y: pos.y,
          width: size.width,
          height: size.height,
        });
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
      unlistenTranslateSelection.then((fn) => fn());
      unlistenAutoCopy.then((fn) => fn());
      unlistenReplaceTyping.then((fn) => fn());
      unlistenReplaceTranslate.then((fn) => fn());
    };
  }, [setSourceText]);

  const navItems: NavItem[] = [
    // Core translation features
    { id: "translator", icon: Languages, label: t("nav.translator"), group: "core" },
    { id: "ocr", icon: Scan, label: t("nav.ocr"), group: "core" },
    // Reading & document translation
    { id: "pdf", icon: FileText, label: t("nav.pdf"), group: "read" },
    { id: "epub", icon: BookOpen, label: t("nav.epub"), group: "read" },
    { id: "subtitle", icon: Subtitles, label: t("nav.subtitle"), group: "read" },
    // Data & vocabulary
    { id: "history", icon: HistoryIcon, label: t("nav.history"), group: "data" },
    { id: "wordbook", icon: Bookmark, label: t("nav.wordbook"), group: "data" },
    { id: "glossary", icon: Book, label: t("nav.glossary"), group: "data" },
    // System & tools
    { id: "tools", icon: Wrench, label: t("nav.tools"), group: "system" },
    { id: "plugins", icon: Puzzle, label: t("nav.plugins"), group: "system" },
    { id: "settings", icon: SettingsIcon, label: t("nav.settings"), group: "system" },
  ];

  // Group nav items for rendering with separators
  const navGroups = [
    { key: "core", items: navItems.filter((i) => i.group === "core") },
    { key: "read", items: navItems.filter((i) => i.group === "read") },
    { key: "data", items: navItems.filter((i) => i.group === "data") },
    { key: "system", items: navItems.filter((i) => i.group === "system") },
  ];

  return (
    <div className="flex h-screen bg-bg-primary">
      {/* Sidebar */}
      <nav className="w-14 bg-bg-secondary border-r border-border flex flex-col items-center py-3 overflow-y-auto">
        {/* Logo */}
        <div className="w-9 h-9 rounded-lg bg-gradient-to-br from-primary to-accent flex items-center justify-center mb-4 shrink-0">
          <span className="text-white font-bold text-sm">M</span>
        </div>

        {/* Nav Groups */}
        {navGroups.map((group, groupIndex) => (
          <div key={group.key} className="w-full flex flex-col items-center gap-1">
            {/* Separator */}
            {groupIndex > 0 && (
              <div className="w-6 h-px bg-border my-1 shrink-0" />
            )}
            {/* Nav Items */}
            {group.items.map((item) => {
              const Icon = item.icon;
              const isActive = page === item.id;

              return (
                <button
                  key={item.id}
                  className={`w-10 h-10 rounded-lg flex items-center justify-center transition-colors shrink-0 ${
                    isActive
                      ? "bg-primary text-white shadow-md shadow-primary/25"
                      : "text-text-secondary hover:bg-bg-tertiary hover:text-text-primary"
                  }`}
                  onClick={() => setPage(item.id as Page)}
                  title={item.label}
                >
                  <Icon size={18} />
                </button>
              );
            })}
          </div>
        ))}

        {/* Spacer */}
        <div className="flex-1 min-h-2" />

        {/* Bottom Actions */}
        <div className="w-full flex flex-col items-center gap-1 shrink-0">
          <div className="w-6 h-px bg-border my-1" />

          {/* Pin Toggle */}
          <button
            className={`w-10 h-10 rounded-lg flex items-center justify-center transition-colors ${
              pinned
                ? "bg-primary text-white shadow-md shadow-primary/25"
                : "text-text-secondary hover:bg-bg-tertiary hover:text-text-primary"
            }`}
            onClick={togglePin}
            title={pinned ? t("common.unpin") : t("common.pin")}
          >
            <Pin size={18} />
          </button>

          {/* Theme Toggle */}
          <button
            className="w-10 h-10 rounded-lg flex items-center justify-center text-text-secondary hover:bg-bg-tertiary hover:text-text-primary transition-colors"
            onClick={toggleTheme}
            title={theme === "dark" ? t("common.lightMode") : t("common.darkMode")}
          >
            {theme === "dark" ? <Sun size={18} /> : <Moon size={18} />}
          </button>
        </div>
      </nav>

      {/* Main Content */}
      <main className="flex-1 overflow-hidden">
        {page === "translator" && <MainTranslator />}
        {page === "ocr" && (
          <div className="flex flex-col h-full p-4 overflow-y-auto">
            <OcrMonitor />
          </div>
        )}
        {page === "settings" && <Settings />}
        {page === "history" && <History />}
        {page === "wordbook" && <WordBook />}
        {page === "pdf" && <PdfViewer />}
        {page === "epub" && <EpubViewer />}
        {page === "subtitle" && <SubtitleViewer />}
        {page === "glossary" && <Glossary />}
        {page === "tools" && <Tools />}
        {page === "plugins" && <Plugins />}
      </main>
    </div>
  );
}

export default App;
