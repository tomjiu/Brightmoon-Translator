import { useState, useEffect, useMemo } from 'react';
import { invokeOrThrow } from '../services/invoke';
import { X, Search, Monitor } from 'lucide-react';

interface ProcessInfo {
  pid: number;
  name: string;
  exePath: string;
}

interface ProcessPickerProps {
  isOpen: boolean;
  onClose: () => void;
  onSelect: (pid: number) => void;
}

export default function ProcessPicker({ isOpen, onClose, onSelect }: ProcessPickerProps) {
  const [processes, setProcesses] = useState<ProcessInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedPid, setSelectedPid] = useState<number | null>(null);

  useEffect(() => {
    if (isOpen) {
      loadProcesses();
    }
  }, [isOpen]);

  const loadProcesses = async () => {
    setLoading(true);
    try {
      const list = await invokeOrThrow<ProcessInfo[]>('get_process_list');
      setProcesses(list);
    } catch (error) {
      console.error('Failed to load processes:', error);
    } finally {
      setLoading(false);
    }
  };

  const filteredProcesses = useMemo(() => {
    if (!searchQuery.trim()) return processes;
    const query = searchQuery.toLowerCase();
    return processes.filter(
      (p) =>
        p.name.toLowerCase().includes(query) ||
        p.pid.toString().includes(query) ||
        p.exePath.toLowerCase().includes(query),
    );
  }, [processes, searchQuery]);

  const handleSelect = () => {
    if (selectedPid !== null) {
      onSelect(selectedPid);
      onClose();
    }
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-bg-primary border border-border rounded-xl shadow-lg w-full max-w-2xl mx-4 max-h-[80vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-border">
          <h3 className="text-lg font-semibold text-text-primary flex items-center gap-2">
            <Monitor size={20} className="text-primary" />
            选择进程
          </h3>
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-bg-tertiary text-text-secondary hover:text-text-primary transition-colors"
          >
            <X size={20} />
          </button>
        </div>

        {/* Search */}
        <div className="p-4 border-b border-border">
          <div className="relative">
            <Search
              size={16}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary"
            />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="搜索进程名称、PID 或路径..."
              className="w-full bg-bg-secondary border border-border rounded-lg pl-9 pr-3 py-2 text-sm text-text-primary outline-none focus:border-primary"
              autoFocus
            />
          </div>
        </div>

        {/* Process List */}
        <div className="flex-1 overflow-y-auto p-2">
          {loading ? (
            <div className="flex items-center justify-center py-12 text-text-secondary">
              加载进程列表...
            </div>
          ) : filteredProcesses.length === 0 ? (
            <div className="flex items-center justify-center py-12 text-text-secondary">
              {searchQuery ? '未找到匹配的进程' : '没有可用进程'}
            </div>
          ) : (
            <div className="space-y-1">
              {filteredProcesses.map((proc) => (
                <div
                  key={proc.pid}
                  className={`p-3 rounded-lg cursor-pointer transition-colors ${
                    selectedPid === proc.pid
                      ? 'bg-primary/10 border border-primary'
                      : 'hover:bg-bg-secondary border border-transparent'
                  }`}
                  onClick={() => setSelectedPid(proc.pid)}
                  onDoubleClick={() => {
                    setSelectedPid(proc.pid);
                    handleSelect();
                  }}
                >
                  <div className="flex items-center justify-between gap-3">
                    <div className="flex items-center gap-2 min-w-0 flex-1">
                      <Monitor size={14} className="text-text-secondary shrink-0" />
                      <span className="text-sm font-medium text-text-primary truncate">
                        {proc.name}
                      </span>
                    </div>
                    <span className="text-xs text-text-secondary font-mono shrink-0">
                      PID: {proc.pid}
                    </span>
                  </div>
                  {proc.exePath && (
                    <div className="mt-1 ml-5 text-xs text-text-secondary truncate">
                      {proc.exePath}
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between p-4 border-t border-border">
          <div className="text-xs text-text-secondary">
            {filteredProcesses.length} 个进程
            {searchQuery && ` (搜索结果)`}
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={onClose}
              className="px-4 py-2 rounded-lg text-sm font-medium text-text-secondary hover:bg-bg-tertiary transition-colors"
            >
              取消
            </button>
            <button
              onClick={handleSelect}
              disabled={selectedPid === null}
              className="px-4 py-2 rounded-lg text-sm font-medium bg-primary text-primary-fg hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              确定
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
