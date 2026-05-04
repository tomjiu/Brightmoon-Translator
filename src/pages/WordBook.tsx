import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { Search, Trash2, Copy, Check, X, Download, Plus, Edit2, Save } from "lucide-react";

interface WordBookItem {
  id: string;
  word: string;
  translation: string;
  fromLang: string;
  toLang: string;
  note: string;
  timestamp: number;
}

function WordBook() {
  const [items, setItems] = useState<WordBookItem[]>([]);
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [editingNoteId, setEditingNoteId] = useState<string | null>(null);
  const [noteText, setNoteText] = useState("");
  const [showAddForm, setShowAddForm] = useState(false);
  const [newWord, setNewWord] = useState("");
  const [newTranslation, setNewTranslation] = useState("");
  const [newNote, setNewNote] = useState("");
  const debounceTimer = useRef<ReturnType<typeof setTimeout>>();
  const { t } = useI18n();

  useEffect(() => {
    loadWordBook();
  }, []);

  useEffect(() => {
    if (debounceTimer.current) {
      clearTimeout(debounceTimer.current);
    }
    debounceTimer.current = setTimeout(() => {
      setDebouncedSearch(search);
    }, 300);

    return () => {
      if (debounceTimer.current) {
        clearTimeout(debounceTimer.current);
      }
    };
  }, [search]);

  const loadWordBook = async () => {
    try {
      const data = await invoke<WordBookItem[]>("get_wordbook");
      setItems(data);
    } catch (err) {
      console.error("Failed to load wordbook:", err);
    }
  };

  const searchWordBook = async (query: string) => {
    try {
      const data = await invoke<WordBookItem[]>("search_wordbook", { query });
      setItems(data);
    } catch (err) {
      console.error("Failed to search wordbook:", err);
    }
  };

  useEffect(() => {
    if (debouncedSearch) {
      searchWordBook(debouncedSearch);
    } else {
      loadWordBook();
    }
  }, [debouncedSearch]);

  const addWord = async () => {
    if (!newWord.trim() || !newTranslation.trim()) return;
    try {
      await invoke("add_wordbook_entry", {
        word: newWord.trim(),
        translation: newTranslation.trim(),
        fromLang: "auto",
        toLang: "zh",
        note: newNote.trim() || null,
      });
      setNewWord("");
      setNewTranslation("");
      setNewNote("");
      setShowAddForm(false);
      loadWordBook();
    } catch (err) {
      console.error("Failed to add word:", err);
    }
  };

  const updateNote = async (id: string) => {
    try {
      await invoke("update_wordbook_note", { id, note: noteText });
      setEditingNoteId(null);
      setNoteText("");
      loadWordBook();
    } catch (err) {
      console.error("Failed to update note:", err);
    }
  };

  const deleteItem = async (id: string) => {
    try {
      await invoke("delete_wordbook_entry", { id });
      setItems((prev) => prev.filter((item) => item.id !== id));
      setSelectedIds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    } catch (err) {
      console.error("Failed to delete word:", err);
    }
  };

  const batchDelete = async () => {
    if (selectedIds.size === 0) return;
    try {
      await invoke("batch_delete_wordbook", { ids: Array.from(selectedIds) });
      setItems((prev) => prev.filter((item) => !selectedIds.has(item.id)));
      setSelectedIds(new Set());
    } catch (err) {
      console.error("Failed to batch delete:", err);
    }
  };

  const clearAll = async () => {
    try {
      await invoke("clear_wordbook");
      setItems([]);
      setSelectedIds(new Set());
    } catch (err) {
      console.error("Failed to clear wordbook:", err);
    }
  };

  const toggleSelect = (id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const toggleSelectAll = () => {
    const allSelected = items.every((item) => selectedIds.has(item.id));
    if (allSelected) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(items.map((item) => item.id)));
    }
  };

  const copyText = (text: string, id: string) => {
    navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1500);
  };

  const exportCsv = async () => {
    try {
      const csv = await invoke<string>("export_wordbook_csv");
      const blob = new Blob(["\uFEFF" + csv], { type: "text/csv;charset=utf-8;" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `生词本_${new Date().toISOString().slice(0, 10)}.csv`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error("Failed to export csv:", err);
    }
  };

  const startEditNote = (item: WordBookItem) => {
    setEditingNoteId(item.id);
    setNoteText(item.note);
  };

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleString("zh-CN", {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  return (
    <div className="h-full flex flex-col p-6">
      {/* Header */}
      <div className="flex justify-between items-center mb-5">
        <h1 className="text-2xl font-bold">{t("wordbook.title")}</h1>
        <div className="flex items-center gap-3">
          <div className="relative">
            <Search
              size={16}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary"
            />
            <input
              type="text"
              placeholder={t("wordbook.search")}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="bg-bg-secondary text-text-primary border border-border rounded-lg pl-9 pr-3 py-2 text-sm w-48 focus:border-primary outline-none"
            />
          </div>
          {selectedIds.size > 0 && (
            <button
              className="bg-error text-white border border-error rounded-lg px-4 py-2 text-sm hover:bg-error/80 transition-colors flex items-center gap-1.5"
              onClick={batchDelete}
            >
              <Trash2 size={14} />
              {t("wordbook.deleteSelected", { count: selectedIds.size })}
            </button>
          )}
          <button
            className="bg-primary text-white border border-primary rounded-lg px-4 py-2 text-sm hover:bg-primary/80 transition-colors flex items-center gap-1.5"
            onClick={() => setShowAddForm(!showAddForm)}
          >
            <Plus size={14} />
            {t("wordbook.addWord")}
          </button>
          <button
            className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-4 py-2 text-sm hover:bg-primary hover:text-bg-primary hover:border-primary transition-colors flex items-center gap-1.5"
            onClick={exportCsv}
            disabled={items.length === 0}
          >
            <Download size={14} />
            {t("wordbook.exportCsv")}
          </button>
          <button
            className="bg-bg-tertiary text-error border border-border rounded-lg px-4 py-2 text-sm hover:bg-error hover:text-white hover:border-error transition-colors flex items-center gap-1.5"
            onClick={clearAll}
          >
            <Trash2 size={14} />
            {t("wordbook.clearAll")}
          </button>
        </div>
      </div>

      {/* Add Word Form */}
      {showAddForm && (
        <div className="bg-bg-secondary border border-border rounded-xl p-4 mb-4">
          <div className="flex gap-3">
            <input
              type="text"
              placeholder={t("wordbook.wordPlaceholder")}
              value={newWord}
              onChange={(e) => setNewWord(e.target.value)}
              className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
            />
            <input
              type="text"
              placeholder={t("wordbook.translationPlaceholder")}
              value={newTranslation}
              onChange={(e) => setNewTranslation(e.target.value)}
              className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
            />
            <input
              type="text"
              placeholder={t("wordbook.notePlaceholder")}
              value={newNote}
              onChange={(e) => setNewNote(e.target.value)}
              className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none"
            />
            <button
              className="bg-primary text-white rounded-lg px-4 py-2 text-sm hover:bg-primary/80 transition-colors"
              onClick={addWord}
              disabled={!newWord.trim() || !newTranslation.trim()}
            >
              {t("wordbook.add")}
            </button>
          </div>
        </div>
      )}

      {/* List */}
      <div className="flex-1 overflow-y-auto space-y-2.5">
        {items.length === 0 ? (
          <div className="flex items-center justify-center h-full text-text-secondary text-sm">
            {debouncedSearch ? t("wordbook.noResults") : t("wordbook.noWords")}
          </div>
        ) : (
          <>
            {/* Select All */}
            <div className="flex items-center gap-2 px-1 pb-1">
              <label className="flex items-center gap-2 text-xs text-text-secondary cursor-pointer">
                <input
                  type="checkbox"
                  checked={items.every((item) => selectedIds.has(item.id))}
                  onChange={toggleSelectAll}
                  className="rounded border-border accent-primary"
                />
                {t("wordbook.selectAll")}
              </label>
              <span className="text-xs text-text-secondary">
                {t("wordbook.totalWords", { count: items.length })}
              </span>
            </div>

            {items.map((item) => (
              <div
                key={item.id}
                className={`bg-bg-secondary border rounded-xl p-3.5 group relative ${
                  selectedIds.has(item.id) ? "border-primary" : "border-border"
                }`}
              >
                {/* Checkbox & Delete button */}
                <div className="absolute top-2 right-2 flex items-center gap-1">
                  <input
                    type="checkbox"
                    checked={selectedIds.has(item.id)}
                    onChange={() => toggleSelect(item.id)}
                    className="rounded border-border accent-primary opacity-0 group-hover:opacity-100 transition-opacity"
                  />
                  <button
                    className="opacity-0 group-hover:opacity-100 transition-opacity p-1 rounded-md hover:bg-error/20 text-text-secondary hover:text-error"
                    onClick={() => deleteItem(item.id)}
                    title={t("wordbook.deleteItem")}
                  >
                    <X size={14} />
                  </button>
                </div>

                <div className="flex justify-between items-center mb-2">
                  <span className="text-sm font-medium pr-6">{item.word}</span>
                  <button
                    className="bg-bg-tertiary border border-border text-text-secondary rounded-md px-2 py-1 text-xs hover:bg-primary hover:text-bg-primary transition-colors flex items-center gap-1 flex-shrink-0 ml-2"
                    onClick={() => copyText(item.word, `word-${item.id}`)}
                  >
                    {copiedId === `word-${item.id}` ? (
                      <Check size={12} />
                    ) : (
                      <Copy size={12} />
                    )}
                  </button>
                </div>
                <div className="flex justify-between items-center mb-2">
                  <span className="text-sm text-primary pr-6">
                    {item.translation}
                  </span>
                  <button
                    className="bg-bg-tertiary border border-border text-text-secondary rounded-md px-2 py-1 text-xs hover:bg-primary hover:text-bg-primary transition-colors flex items-center gap-1 flex-shrink-0 ml-2"
                    onClick={() => copyText(item.translation, `trans-${item.id}`)}
                  >
                    {copiedId === `trans-${item.id}` ? (
                      <Check size={12} />
                    ) : (
                      <Copy size={12} />
                    )}
                  </button>
                </div>

                {/* Note */}
                {editingNoteId === item.id ? (
                  <div className="flex gap-2 mb-2">
                    <input
                      type="text"
                      value={noteText}
                      onChange={(e) => setNoteText(e.target.value)}
                      className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-2 py-1 text-xs focus:border-primary outline-none"
                      autoFocus
                    />
                    <button
                      className="text-primary hover:text-primary/80 transition-colors"
                      onClick={() => updateNote(item.id)}
                    >
                      <Save size={14} />
                    </button>
                    <button
                      className="text-text-secondary hover:text-text-primary transition-colors"
                      onClick={() => setEditingNoteId(null)}
                    >
                      <X size={14} />
                    </button>
                  </div>
                ) : item.note ? (
                  <div className="flex items-center gap-2 mb-2">
                    <span className="text-xs text-text-secondary italic">{item.note}</span>
                    <button
                      className="text-text-secondary hover:text-text-primary transition-colors opacity-0 group-hover:opacity-100"
                      onClick={() => startEditNote(item)}
                    >
                      <Edit2 size={12} />
                    </button>
                  </div>
                ) : (
                  <button
                    className="text-xs text-text-secondary hover:text-text-primary transition-colors opacity-0 group-hover:opacity-100 mb-2"
                    onClick={() => startEditNote(item)}
                  >
                    {t("wordbook.addNote")}
                  </button>
                )}

                <div className="flex justify-between text-xs text-text-secondary">
                  <span className="uppercase font-medium">
                    {item.fromLang} → {item.toLang}
                  </span>
                  <span>{formatTime(item.timestamp)}</span>
                </div>
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
}

export default WordBook;
