import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import type { HistoryItem } from "../types";
import { Search, Trash2, Copy, Check, X, Download, ChevronLeft, ChevronRight } from "lucide-react";

const PAGE_SIZE = 50;

function History() {
  const [items, setItems] = useState<HistoryItem[]>([]);
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [currentPage, setCurrentPage] = useState(1);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const debounceTimer = useRef<ReturnType<typeof setTimeout>>();
  const { t } = useI18n();

  useEffect(() => {
    loadHistory();
  }, []);

  // Debounce search input
  useEffect(() => {
    if (debounceTimer.current) {
      clearTimeout(debounceTimer.current);
    }
    debounceTimer.current = setTimeout(() => {
      setDebouncedSearch(search);
      setCurrentPage(1); // Reset to first page on search
    }, 300);

    return () => {
      if (debounceTimer.current) {
        clearTimeout(debounceTimer.current);
      }
    };
  }, [search]);

  const loadHistory = async () => {
    try {
      const history = await invoke<HistoryItem[]>("get_history");
      setItems(history);
    } catch (err) {
      console.error("Failed to load history:", err);
    }
  };

  const clearHistory = async () => {
    try {
      await invoke("clear_history");
      setItems([]);
      setSelectedIds(new Set());
      setCurrentPage(1);
    } catch (err) {
      console.error("Failed to clear history:", err);
    }
  };

  const deleteItem = async (id: string) => {
    try {
      await invoke("delete_history_item", { id });
      setItems((prev) => prev.filter((item) => item.id !== id));
      setSelectedIds((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    } catch (err) {
      console.error("Failed to delete history item:", err);
    }
  };

  const batchDelete = async () => {
    if (selectedIds.size === 0) return;
    try {
      await invoke("batch_delete_history", { ids: Array.from(selectedIds) });
      setItems((prev) => prev.filter((item) => !selectedIds.has(item.id)));
      setSelectedIds(new Set());
    } catch (err) {
      console.error("Failed to batch delete history:", err);
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
    const pageItemIds = paginatedItems.map((item) => item.id);
    const allSelected = pageItemIds.every((id) => selectedIds.has(id));
    if (allSelected) {
      setSelectedIds((prev) => {
        const next = new Set(prev);
        pageItemIds.forEach((id) => next.delete(id));
        return next;
      });
    } else {
      setSelectedIds((prev) => {
        const next = new Set(prev);
        pageItemIds.forEach((id) => next.add(id));
        return next;
      });
    }
  };

  const copyText = (text: string, id: string) => {
    navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1500);
  };

  const exportCsv = () => {
    const escapeCsv = (text: string) => {
      if (text.includes(",") || text.includes('"') || text.includes("\n")) {
        return `"${text.replace(/"/g, '""')}"`;
      }
      return text;
    };

    const header = "时间,源文本,译文,源语言,目标语言,引擎";
    const rows = items.map((item) => {
      const time = new Date(item.timestamp).toLocaleString("zh-CN");
      return [
        escapeCsv(time),
        escapeCsv(item.sourceText),
        escapeCsv(item.translatedText),
        escapeCsv(item.from),
        escapeCsv(item.to),
        escapeCsv(item.engine),
      ].join(",");
    });

    const csv = "\uFEFF" + header + "\n" + rows.join("\n");
    const blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `翻译历史_${new Date().toISOString().slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const filtered = items.filter(
    (item) =>
      item.sourceText.toLowerCase().includes(debouncedSearch.toLowerCase()) ||
      item.translatedText.toLowerCase().includes(debouncedSearch.toLowerCase())
  );

  // Pagination
  const totalPages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const paginatedItems = filtered.slice(
    (currentPage - 1) * PAGE_SIZE,
    currentPage * PAGE_SIZE
  );

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
        <h1 className="text-2xl font-bold">{t("history.title")}</h1>
        <div className="flex items-center gap-3">
          <div className="relative">
            <Search
              size={16}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary"
            />
            <input
              type="text"
              placeholder={t("history.search")}
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
              {t("history.deleteSelected", { count: selectedIds.size })}
            </button>
          )}
          <button
            className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-4 py-2 text-sm hover:bg-primary hover:text-bg-primary hover:border-primary transition-colors flex items-center gap-1.5"
            onClick={exportCsv}
            disabled={items.length === 0}
          >
            <Download size={14} />
            {t("history.exportCsv")}
          </button>
          <button
            className="bg-bg-tertiary text-error border border-border rounded-lg px-4 py-2 text-sm hover:bg-error hover:text-white hover:border-error transition-colors flex items-center gap-1.5"
            onClick={clearHistory}
          >
            <Trash2 size={14} />
            {t("history.clearAll")}
          </button>
        </div>
      </div>

      {/* List */}
      <div className="flex-1 overflow-y-auto space-y-2.5">
        {paginatedItems.length === 0 ? (
          <div className="flex items-center justify-center h-full text-text-secondary text-sm">
            {debouncedSearch ? t("history.noResults") : t("history.noHistory")}
          </div>
        ) : (
          <>
            {/* Select All */}
            {filtered.length > 0 && (
              <div className="flex items-center gap-2 px-1 pb-1">
                <label className="flex items-center gap-2 text-xs text-text-secondary cursor-pointer">
                  <input
                    type="checkbox"
                    checked={paginatedItems.every((item) => selectedIds.has(item.id))}
                    onChange={toggleSelectAll}
                    className="rounded border-border accent-primary"
                  />
                  {t("history.selectAll")}
                </label>
                <span className="text-xs text-text-secondary">
                  {t("history.totalRecords", { count: filtered.length })}
                </span>
              </div>
            )}

            {paginatedItems.map((item) => (
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
                    title={t("history.deleteItem")}
                  >
                    <X size={14} />
                  </button>
                </div>

                <div className="flex justify-between items-center mb-2">
                  <span className="text-sm pr-6">{item.sourceText}</span>
                  <button
                    className="bg-bg-tertiary border border-border text-text-secondary rounded-md px-2 py-1 text-xs hover:bg-primary hover:text-bg-primary transition-colors flex items-center gap-1 flex-shrink-0 ml-2"
                    onClick={() => copyText(item.sourceText, `src-${item.id}`)}
                  >
                    {copiedId === `src-${item.id}` ? (
                      <Check size={12} />
                    ) : (
                      <Copy size={12} />
                    )}
                  </button>
                </div>
                <div className="flex justify-between items-center mb-2">
                  <span className="text-sm text-primary pr-6">
                    {item.translatedText}
                  </span>
                  <button
                    className="bg-bg-tertiary border border-border text-text-secondary rounded-md px-2 py-1 text-xs hover:bg-primary hover:text-bg-primary transition-colors flex items-center gap-1 flex-shrink-0 ml-2"
                    onClick={() =>
                      copyText(item.translatedText, `dst-${item.id}`)
                    }
                  >
                    {copiedId === `dst-${item.id}` ? (
                      <Check size={12} />
                    ) : (
                      <Copy size={12} />
                    )}
                  </button>
                </div>
                <div className="flex justify-between text-xs text-text-secondary">
                  <span className="uppercase font-medium">{item.engine}</span>
                  <span>{formatTime(item.timestamp)}</span>
                </div>
              </div>
            ))}
          </>
        )}
      </div>

      {/* Pagination */}
      {totalPages > 1 && (
        <div className="flex justify-between items-center mt-4 pt-4 border-t border-border">
          <span className="text-xs text-text-secondary">
            {t("history.page", { current: currentPage, total: totalPages, count: filtered.length })}
          </span>
          <div className="flex items-center gap-2">
            <button
              className="bg-bg-tertiary border border-border text-text-secondary rounded-lg px-3 py-1.5 text-sm hover:bg-primary hover:text-white hover:border-primary transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
              disabled={currentPage === 1}
            >
              <ChevronLeft size={16} />
            </button>
            <button
              className="bg-bg-tertiary border border-border text-text-secondary rounded-lg px-3 py-1.5 text-sm hover:bg-primary hover:text-white hover:border-primary transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              onClick={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
              disabled={currentPage === totalPages}
            >
              <ChevronRight size={16} />
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export default History;
