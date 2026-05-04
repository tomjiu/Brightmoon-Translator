import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus, Trash2, Book } from "lucide-react";

interface GlossaryEntry {
  source: string;
  target: string;
  context?: string;
}

function Glossary() {
  const [entries, setEntries] = useState<Record<string, GlossaryEntry[]>>({});
  const [langPair, setLangPair] = useState("en-zh");
  const [newSource, setNewSource] = useState("");
  const [newTarget, setNewTarget] = useState("");
  const [newContext, setNewContext] = useState("");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    loadGlossary();
  }, []);

  const loadGlossary = async () => {
    try {
      const allEntries = await invoke<Record<string, GlossaryEntry[]>>(
        "get_all_glossary"
      );
      setEntries(allEntries);
    } catch (err) {
      console.error("Failed to load glossary:", err);
    }
  };

  const addEntry = async () => {
    if (!newSource.trim() || !newTarget.trim()) return;

    setLoading(true);
    try {
      await invoke("add_glossary_entry", {
        langPair,
        source: newSource.trim(),
        target: newTarget.trim(),
        context: newContext.trim() || null,
      });
      setNewSource("");
      setNewTarget("");
      setNewContext("");
      await loadGlossary();
    } catch (err) {
      console.error("Failed to add entry:", err);
    } finally {
      setLoading(false);
    }
  };

  const removeEntry = async (langPair: string, source: string) => {
    try {
      await invoke("remove_glossary_entry", { langPair, source });
      await loadGlossary();
    } catch (err) {
      console.error("Failed to remove entry:", err);
    }
  };

  const langPairs = [
    { value: "en-zh", label: "英 → 中" },
    { value: "zh-en", label: "中 → 英" },
    { value: "ja-zh", label: "日 → 中" },
    { value: "zh-ja", label: "中 → 日" },
    { value: "ko-zh", label: "韩 → 中" },
    { value: "zh-ko", label: "中 → 韩" },
  ];

  return (
    <div className="flex flex-col h-full p-6">
      <div className="flex items-center gap-3 mb-6">
        <Book size={24} className="text-primary" />
        <h1 className="text-xl font-bold text-text-primary">术语表管理</h1>
      </div>

      {/* Add Entry Form */}
      <div className="bg-bg-secondary border border-border rounded-xl p-4 mb-6">
        <h2 className="text-sm font-semibold text-text-secondary mb-3">
          添加术语
        </h2>
        <div className="flex gap-3">
          <select
            value={langPair}
            onChange={(e) => setLangPair(e.target.value)}
            className="bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm"
          >
            {langPairs.map((lp) => (
              <option key={lp.value} value={lp.value}>
                {lp.label}
              </option>
            ))}
          </select>
          <input
            type="text"
            value={newSource}
            onChange={(e) => setNewSource(e.target.value)}
            placeholder="原文"
            className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm"
          />
          <input
            type="text"
            value={newTarget}
            onChange={(e) => setNewTarget(e.target.value)}
            placeholder="译文"
            className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm"
          />
          <input
            type="text"
            value={newContext}
            onChange={(e) => setNewContext(e.target.value)}
            placeholder="上下文(可选)"
            className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm"
          />
          <button
            onClick={addEntry}
            disabled={loading || !newSource.trim() || !newTarget.trim()}
            className="bg-primary text-bg-primary rounded-lg px-4 py-2 text-sm font-semibold hover:bg-primary-hover transition-colors disabled:opacity-50 flex items-center gap-2"
          >
            <Plus size={16} />
            添加
          </button>
        </div>
      </div>

      {/* Glossary Entries */}
      <div className="flex-1 overflow-y-auto">
        {Object.keys(entries).length === 0 ? (
          <div className="flex items-center justify-center h-full text-text-secondary">
            暂无术语条目
          </div>
        ) : (
          Object.entries(entries).map(([pair, pairEntries]) => (
            <div
              key={pair}
              className="bg-bg-secondary border border-border rounded-xl mb-4 overflow-hidden"
            >
              <div className="bg-bg-tertiary px-4 py-2 border-b border-border">
                <span className="text-sm font-semibold text-primary">
                  {langPairs.find((lp) => lp.value === pair)?.label || pair}
                </span>
                <span className="text-xs text-text-secondary ml-2">
                  ({pairEntries.length} 条)
                </span>
              </div>
              <div className="divide-y divide-border">
                {pairEntries.map((entry, index) => (
                  <div
                    key={index}
                    className="flex items-center justify-between px-4 py-3 hover:bg-bg-tertiary/50"
                  >
                    <div className="flex-1">
                      <span className="text-sm text-text-primary font-medium">
                        {entry.source}
                      </span>
                      <span className="text-text-secondary mx-2">→</span>
                      <span className="text-sm text-primary">
                        {entry.target}
                      </span>
                      {entry.context && (
                        <span className="text-xs text-text-secondary ml-2">
                          ({entry.context})
                        </span>
                      )}
                    </div>
                    <button
                      onClick={() => removeEntry(pair, entry.source)}
                      className="text-text-secondary hover:text-error transition-colors p-1"
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>
                ))}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

export default Glossary;
