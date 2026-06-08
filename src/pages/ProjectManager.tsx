import { useState, useEffect } from "react";
import { useProjectStore, TranslationProject } from "../stores/projectStore";
import { useToastStore } from "../stores/toastStore";
import { useI18n } from "../i18n";
import {
  FolderOpen,
  Plus,
  Trash2,
  FileText,
  Download,
  ChevronRight,
  ChevronDown,
  Edit2,
  Check,
  X,
  Upload,
  Languages,
  BarChart3,
  Clock,
  AlertCircle,
} from "lucide-react";

function ProjectManager() {
  const { t } = useI18n();
  const {
    projects,
    currentProject,
    currentFiles,
    currentSegments,
    selectedFileId,
    isLoading,
    error,
    loadProjects,
    createProject,
    updateProject,
    deleteProject,
    setCurrentProject,
    addFile,
    deleteFile,
    setSelectedFile,
    exportProjectJson,
    clearError,
  } = useProjectStore();

  const addToast = useToastStore((s) => s.addToast);

  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const [newProjectName, setNewProjectName] = useState("");
  const [newProjectDesc, setNewProjectDesc] = useState("");
  const [newSourceLang, setNewSourceLang] = useState("auto");
  const [newTargetLang, setNewTargetLang] = useState("zh");

  const [editingProject, setEditingProject] = useState<string | null>(null);
  const [editName, setEditName] = useState("");

  const [expandedFiles, setExpandedFiles] = useState<Set<string>>(new Set());

  useEffect(() => {
    loadProjects();
  }, [loadProjects]);

  useEffect(() => {
    if (error) {
      addToast({ type: "error", message: error, duration: 5000 });
      clearError();
    }
  }, [error, addToast, clearError]);

  const handleCreateProject = async () => {
    if (!newProjectName.trim()) {
      addToast({ type: "warning", message: t("projects.nameRequired"), duration: 3000 });
      return;
    }
    try {
      await createProject(newProjectName, newProjectDesc, newSourceLang, newTargetLang);
      setShowCreateDialog(false);
      setNewProjectName("");
      setNewProjectDesc("");
      addToast({ type: "success", message: t("projects.created"), duration: 2000 });
    } catch (err) {
      // Error handled by store
    }
  };

  const handleDeleteProject = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (window.confirm(t("projects.deleteConfirm"))) {
      await deleteProject(id);
      if (currentProject?.id === id) {
        setCurrentProject(null);
      }
      addToast({ type: "success", message: t("projects.deleted"), duration: 2000 });
    }
  };

  const handleStartEdit = (project: TranslationProject, e: React.MouseEvent) => {
    e.stopPropagation();
    setEditingProject(project.id);
    setEditName(project.name);
  };

  const handleSaveEdit = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (editName.trim()) {
      await updateProject(id, { name: editName });
      addToast({ type: "success", message: t("projects.updated"), duration: 2000 });
    }
    setEditingProject(null);
  };

  const handleSelectProject = (project: TranslationProject) => {
    setCurrentProject(project);
  };

  const handleAddFile = async () => {
    if (!currentProject) return;

    // In a real app, you'd use a file picker dialog
    // For now, simulate adding a file
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".txt,.docx,.pdf,.epub,.srt,.json,.csv";
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (file) {
        const fileType = file.name.split(".").pop() || "txt";
        try {
          // Use file name as path since web File API doesn't expose full path
          await addFile(
            currentProject.id,
            file.name,
            file.name,
            fileType,
            file.size
          );
          addToast({ type: "success", message: t("projects.fileAdded", { name: file.name }), duration: 2000 });
        } catch (err) {
          // Error handled by store
        }
      }
    };
    input.click();
  };

  const handleDeleteFile = async (fileId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (window.confirm(t("projects.removeFileConfirm"))) {
      await deleteFile(fileId);
      addToast({ type: "success", message: t("projects.fileRemoved"), duration: 2000 });
    }
  };

  const toggleFileExpanded = (fileId: string) => {
    setExpandedFiles((prev) => {
      const next = new Set(prev);
      if (next.has(fileId)) {
        next.delete(fileId);
      } else {
        next.add(fileId);
      }
      return next;
    });
  };

  const handleExport = async () => {
    if (!currentProject) return;
    try {
      const json = await exportProjectJson(currentProject.id);
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${currentProject.name}_export.json`;
      a.click();
      URL.revokeObjectURL(url);
      addToast({ type: "success", message: t("projects.exported"), duration: 2000 });
    } catch (err) {
      // Error handled by store
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case "completed":
        return "text-green-500";
      case "translating":
        return "text-blue-500";
      case "error":
        return "text-red-500";
      default:
        return "text-text-secondary";
    }
  };

  const getProgressPercent = (project: TranslationProject) => {
    if (project.totalSegments === 0) return 0;
    return Math.round((project.translatedSegments / project.totalSegments) * 100);
  };

  return (
    <div className="flex h-full">
      {/* Project List Panel */}
      <div className="w-64 border-r border-border bg-bg-secondary flex flex-col">
        <div className="p-3 border-b border-border flex items-center justify-between">
          <h2 className="text-sm font-semibold text-text-primary flex items-center gap-2">
            <FolderOpen size={16} />
            {t("projects.title")}
          </h2>
          <button
            className="p-1.5 rounded-lg hover:bg-bg-tertiary text-text-secondary hover:text-primary transition-colors"
            onClick={() => setShowCreateDialog(true)}
            title={t("projects.newProject")}
          >
            <Plus size={16} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {projects.map((project) => (
            <div
              key={project.id}
              className={`group p-2 rounded-lg cursor-pointer transition-colors ${
                currentProject?.id === project.id
                  ? "bg-primary/10 border border-primary/30"
                  : "hover:bg-bg-tertiary border border-transparent"
              }`}
              onClick={() => handleSelectProject(project)}
            >
              {editingProject === project.id ? (
                <div className="flex items-center gap-1">
                  <input
                    type="text"
                    value={editName}
                    onChange={(e) => setEditName(e.target.value)}
                    className="flex-1 px-2 py-1 text-sm bg-bg-primary border border-border rounded"
                    autoFocus
                    onClick={(e) => e.stopPropagation()}
                  />
                  <button
                    className="p-1 text-green-500 hover:bg-green-500/10 rounded"
                    onClick={(e) => handleSaveEdit(project.id, e)}
                  >
                    <Check size={14} />
                  </button>
                  <button
                    className="p-1 text-text-secondary hover:bg-bg-tertiary rounded"
                    onClick={(e) => {
                      e.stopPropagation();
                      setEditingProject(null);
                    }}
                  >
                    <X size={14} />
                  </button>
                </div>
              ) : (
                <>
                  <div className="flex items-center justify-between">
                    <span className="text-sm font-medium text-text-primary truncate">
                      {project.name}
                    </span>
                    <div className="opacity-0 group-hover:opacity-100 flex items-center gap-0.5">
                      <button
                        className="p-1 text-text-secondary hover:text-primary rounded"
                        onClick={(e) => handleStartEdit(project, e)}
                      >
                        <Edit2 size={12} />
                      </button>
                      <button
                        className="p-1 text-text-secondary hover:text-red-500 rounded"
                        onClick={(e) => handleDeleteProject(project.id, e)}
                      >
                        <Trash2 size={12} />
                      </button>
                    </div>
                  </div>
                  <div className="mt-1 flex items-center gap-2 text-xs text-text-secondary">
                    <span>{project.totalFiles} {t("projects.files")}</span>
                    <span className="text-border">|</span>
                    <span>{getProgressPercent(project)}%</span>
                  </div>
                  {project.totalSegments > 0 && (
                    <div className="mt-1.5 h-1 bg-bg-tertiary rounded-full overflow-hidden">
                      <div
                        className="h-full bg-primary rounded-full transition-all"
                        style={{ width: `${getProgressPercent(project)}%` }}
                      />
                    </div>
                  )}
                </>
              )}
            </div>
          ))}

          {projects.length === 0 && (
            <div className="text-center py-8 text-text-secondary text-sm">
              <FolderOpen size={32} className="mx-auto mb-2 opacity-50" />
              <p>{t("projects.noProjects")}</p>
              <button
                className="mt-2 text-primary hover:underline"
                onClick={() => setShowCreateDialog(true)}
              >
                {t("projects.createFirst")}
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {currentProject ? (
          <>
            {/* Project Header */}
            <div className="p-4 border-b border-border bg-bg-secondary">
              <div className="flex items-center justify-between">
                <div>
                  <h1 className="text-lg font-semibold text-text-primary">
                    {currentProject.name}
                  </h1>
                  {currentProject.description && (
                    <p className="text-sm text-text-secondary mt-0.5">
                      {currentProject.description}
                    </p>
                  )}
                </div>
                <div className="flex items-center gap-2">
                  <button
                    className="px-3 py-1.5 text-sm bg-bg-tertiary hover:bg-bg-primary text-text-primary rounded-lg flex items-center gap-1.5 transition-colors"
                    onClick={handleAddFile}
                  >
                    <Upload size={14} />
                    {t("projects.addFile")}
                  </button>
                  <button
                    className="px-3 py-1.5 text-sm bg-primary hover:bg-primary/90 text-white rounded-lg flex items-center gap-1.5 transition-colors"
                    onClick={handleExport}
                  >
                    <Download size={14} />
                    {t("projects.export")}
                  </button>
                </div>
              </div>

              {/* Stats */}
              <div className="mt-3 flex items-center gap-6 text-sm">
                <div className="flex items-center gap-1.5 text-text-secondary">
                  <Languages size={14} />
                  <span>
                    {currentProject.sourceLang} → {currentProject.targetLang}
                  </span>
                </div>
                <div className="flex items-center gap-1.5 text-text-secondary">
                  <FileText size={14} />
                  <span>{currentProject.totalFiles} {t("projects.files")}</span>
                </div>
                <div className="flex items-center gap-1.5 text-text-secondary">
                  <BarChart3 size={14} />
                  <span>
                    {currentProject.translatedSegments}/{currentProject.totalSegments} {t("projects.segments")}
                  </span>
                </div>
                <div className="flex items-center gap-1.5 text-text-secondary">
                  <Clock size={14} />
                  <span>
                    {new Date(currentProject.updatedAt * 1000).toLocaleDateString()}
                  </span>
                </div>
              </div>

              {/* Overall Progress */}
              {currentProject.totalSegments > 0 && (
                <div className="mt-3">
                  <div className="flex items-center justify-between text-xs text-text-secondary mb-1">
                    <span>{t("projects.overallProgress")}</span>
                    <span>{getProgressPercent(currentProject)}%</span>
                  </div>
                  <div className="h-2 bg-bg-tertiary rounded-full overflow-hidden">
                    <div
                      className="h-full bg-gradient-to-r from-primary to-accent rounded-full transition-all"
                      style={{ width: `${getProgressPercent(currentProject)}%` }}
                    />
                  </div>
                </div>
              )}
            </div>

            {/* Files List */}
            <div className="flex-1 overflow-y-auto p-4">
              <h3 className="text-sm font-medium text-text-secondary mb-3">{t("projects.projectFiles")}</h3>

              {currentFiles.length > 0 ? (
                <div className="space-y-2">
                  {currentFiles.map((file) => (
                    <div
                      key={file.id}
                      className="border border-border rounded-lg overflow-hidden"
                    >
                      <div
                        className={`flex items-center justify-between p-3 cursor-pointer transition-colors ${
                          selectedFileId === file.id
                            ? "bg-primary/5"
                            : "hover:bg-bg-tertiary"
                        }`}
                        onClick={() => {
                          setSelectedFile(file.id);
                          toggleFileExpanded(file.id);
                        }}
                      >
                        <div className="flex items-center gap-3">
                          <button className="text-text-secondary">
                            {expandedFiles.has(file.id) ? (
                              <ChevronDown size={16} />
                            ) : (
                              <ChevronRight size={16} />
                            )}
                          </button>
                          <FileText size={18} className="text-text-secondary" />
                          <div>
                            <div className="text-sm font-medium text-text-primary">
                              {file.fileName}
                            </div>
                            <div className="text-xs text-text-secondary flex items-center gap-2">
                              <span className="uppercase">{file.fileType}</span>
                              <span className="text-border">|</span>
                              <span className={getStatusColor(file.status)}>
                                {file.status}
                              </span>
                              <span className="text-border">|</span>
                              <span>
                                {file.translatedSegments}/{file.totalSegments} {t("projects.segments")}
                              </span>
                            </div>
                          </div>
                        </div>

                        <div className="flex items-center gap-2">
                          {file.totalSegments > 0 && (
                            <div className="w-24 h-1.5 bg-bg-tertiary rounded-full overflow-hidden">
                              <div
                                className="h-full bg-primary rounded-full"
                                style={{
                                  width: `${
                                    file.totalSegments > 0
                                      ? Math.round(
                                          (file.translatedSegments / file.totalSegments) * 100
                                        )
                                      : 0
                                  }%`,
                                }}
                              />
                            </div>
                          )}
                          <button
                            className="p-1.5 text-text-secondary hover:text-red-500 rounded-lg hover:bg-red-500/10"
                            onClick={(e) => handleDeleteFile(file.id, e)}
                          >
                            <Trash2 size={14} />
                          </button>
                        </div>
                      </div>

                      {/* Expanded Segments */}
                      {expandedFiles.has(file.id) && selectedFileId === file.id && (
                        <div className="border-t border-border bg-bg-primary">
                          {currentSegments.length > 0 ? (
                            <div className="max-h-64 overflow-y-auto">
                              {currentSegments.map((segment, idx) => (
                                <div
                                  key={segment.id}
                                  className={`p-3 border-b border-border last:border-b-0 ${
                                    idx % 2 === 0 ? "bg-bg-primary" : "bg-bg-secondary/50"
                                  }`}
                                >
                                  <div className="flex items-start gap-3">
                                    <span className="text-xs text-text-secondary w-6 text-right shrink-0 mt-0.5">
                                      {segment.segmentIndex + 1}
                                    </span>
                                    <div className="flex-1 min-w-0">
                                      <div className="text-sm text-text-primary mb-1">
                                        {segment.sourceText}
                                      </div>
                                      <div className="text-sm text-primary">
                                        {segment.translatedText || (
                                          <span className="text-text-secondary italic">
                                            {t("projects.notTranslated")}
                                          </span>
                                        )}
                                      </div>
                                    </div>
                                    <span
                                      className={`text-xs px-1.5 py-0.5 rounded ${
                                        segment.status === "translated"
                                          ? "bg-green-500/10 text-green-500"
                                          : segment.status === "reviewed"
                                          ? "bg-blue-500/10 text-blue-500"
                                          : "bg-bg-tertiary text-text-secondary"
                                      }`}
                                    >
                                      {segment.status}
                                    </span>
                                  </div>
                                </div>
                              ))}
                            </div>
                          ) : (
                            <div className="p-4 text-center text-text-secondary text-sm">
                              <AlertCircle size={20} className="mx-auto mb-1 opacity-50" />
                              <p>{t("projects.noSegments")}</p>
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="text-center py-12 text-text-secondary">
                  <Upload size={48} className="mx-auto mb-3 opacity-30" />
                  <p className="text-sm">{t("projects.noFiles")}</p>
                  <button
                    className="mt-2 text-sm text-primary hover:underline"
                    onClick={handleAddFile}
                  >
                    {t("projects.addFirstFile")}
                  </button>
                </div>
              )}
            </div>
          </>
        ) : (
          /* Empty State */
          <div className="flex-1 flex items-center justify-center text-text-secondary">
            <div className="text-center">
              <FolderOpen size={64} className="mx-auto mb-4 opacity-30" />
              <h2 className="text-lg font-medium text-text-primary mb-2">
                {t("projects.managerTitle")}
              </h2>
              <p className="text-sm mb-4">
                {t("projects.selectHint")}
              </p>
              <button
                className="px-4 py-2 bg-primary hover:bg-primary/90 text-white rounded-lg text-sm"
                onClick={() => setShowCreateDialog(true)}
              >
                {t("projects.createNew")}
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Create Project Dialog */}
      {showCreateDialog && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-bg-secondary rounded-xl shadow-xl w-96 p-6">
            <h3 className="text-lg font-semibold text-text-primary mb-4">
              {t("projects.createNew")}
            </h3>

            <div className="space-y-4">
              <div>
                <label className="block text-sm text-text-secondary mb-1">
                  {t("projects.projectName")}
                </label>
                <input
                  type="text"
                  value={newProjectName}
                  onChange={(e) => setNewProjectName(e.target.value)}
                  className="w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-text-primary focus:outline-none focus:border-primary"
                  placeholder={t("projects.projectNamePlaceholder")}
                  autoFocus
                />
              </div>

              <div>
                <label className="block text-sm text-text-secondary mb-1">
                  {t("projects.description")}
                </label>
                <textarea
                  value={newProjectDesc}
                  onChange={(e) => setNewProjectDesc(e.target.value)}
                  className="w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-text-primary focus:outline-none focus:border-primary resize-none"
                  rows={3}
                  placeholder={t("projects.descriptionPlaceholder")}
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-sm text-text-secondary mb-1">
                    {t("projects.sourceLanguage")}
                  </label>
                  <select
                    value={newSourceLang}
                    onChange={(e) => setNewSourceLang(e.target.value)}
                    className="w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-text-primary focus:outline-none focus:border-primary"
                  >
                    <option value="auto">{t("projects.autoDetect")}</option>
                    <option value="en">{t("projects.lang.en")}</option>
                    <option value="zh">{t("projects.lang.zh")}</option>
                    <option value="ja">{t("projects.lang.ja")}</option>
                    <option value="ko">{t("projects.lang.ko")}</option>
                    <option value="fr">{t("projects.lang.fr")}</option>
                    <option value="de">{t("projects.lang.de")}</option>
                    <option value="es">{t("projects.lang.es")}</option>
                  </select>
                </div>
                <div>
                  <label className="block text-sm text-text-secondary mb-1">
                    {t("projects.targetLanguage")}
                  </label>
                  <select
                    value={newTargetLang}
                    onChange={(e) => setNewTargetLang(e.target.value)}
                    className="w-full px-3 py-2 bg-bg-primary border border-border rounded-lg text-text-primary focus:outline-none focus:border-primary"
                  >
                    <option value="zh">{t("projects.lang.zh")}</option>
                    <option value="en">{t("projects.lang.en")}</option>
                    <option value="ja">{t("projects.lang.ja")}</option>
                    <option value="ko">{t("projects.lang.ko")}</option>
                    <option value="fr">{t("projects.lang.fr")}</option>
                    <option value="de">{t("projects.lang.de")}</option>
                    <option value="es">{t("projects.lang.es")}</option>
                  </select>
                </div>
              </div>
            </div>

            <div className="flex justify-end gap-2 mt-6">
              <button
                className="px-4 py-2 text-sm text-text-secondary hover:text-text-primary hover:bg-bg-tertiary rounded-lg transition-colors"
                onClick={() => {
                  setShowCreateDialog(false);
                  setNewProjectName("");
                  setNewProjectDesc("");
                }}
              >
                {t("common.cancel")}
              </button>
              <button
                className="px-4 py-2 text-sm bg-primary hover:bg-primary/90 text-white rounded-lg transition-colors"
                onClick={handleCreateProject}
                disabled={isLoading}
              >
                {isLoading ? t("projects.creating") : t("projects.createProject")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default ProjectManager;
