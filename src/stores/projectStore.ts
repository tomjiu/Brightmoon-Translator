import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

// Types matching the Rust backend
export interface TranslationProject {
  id: string;
  name: string;
  description: string;
  sourceLang: string;
  targetLang: string;
  createdAt: number;
  updatedAt: number;
  status: string;
  totalFiles: number;
  completedFiles: number;
  totalSegments: number;
  translatedSegments: number;
}

export interface ProjectFile {
  id: string;
  projectId: string;
  fileName: string;
  filePath: string;
  fileType: string;
  fileSize: number;
  status: string;
  totalSegments: number;
  translatedSegments: number;
  createdAt: number;
  updatedAt: number;
}

export interface TranslationSegment {
  id: string;
  fileId: string;
  segmentIndex: number;
  sourceText: string;
  translatedText: string;
  status: string;
  createdAt: number;
  updatedAt: number;
}

export interface ProjectExportData {
  project: TranslationProject;
  files: Array<{
    fileName: string;
    fileType: string;
    segments: TranslationSegment[];
  }>;
  exportedAt: number;
}

interface ProjectState {
  projects: TranslationProject[];
  currentProject: TranslationProject | null;
  currentFiles: ProjectFile[];
  currentSegments: TranslationSegment[];
  selectedFileId: string | null;
  isLoading: boolean;
  error: string | null;

  // Project actions
  loadProjects: () => Promise<void>;
  createProject: (name: string, description?: string, sourceLang?: string, targetLang?: string) => Promise<TranslationProject>;
  updateProject: (id: string, updates: Partial<TranslationProject>) => Promise<void>;
  deleteProject: (id: string) => Promise<void>;
  setCurrentProject: (project: TranslationProject | null) => void;

  // File actions
  loadProjectFiles: (projectId: string) => Promise<void>;
  addFile: (projectId: string, fileName: string, filePath: string, fileType: string, fileSize: number) => Promise<ProjectFile>;
  deleteFile: (fileId: string) => Promise<void>;
  setSelectedFile: (fileId: string | null) => void;

  // Segment actions
  loadFileSegments: (fileId: string) => Promise<void>;
  addSegments: (fileId: string, segments: Array<{ index: string; sourceText: string }>) => Promise<void>;
  updateSegment: (segmentId: string, translatedText: string, status?: string) => Promise<void>;

  // Export
  exportProject: (projectId: string) => Promise<ProjectExportData>;
  exportProjectJson: (projectId: string) => Promise<string>;

  // Utility
  getProgress: () => number;
  clearError: () => void;
}

export const useProjectStore = create<ProjectState>((set, get) => ({
  projects: [],
  currentProject: null,
  currentFiles: [],
  currentSegments: [],
  selectedFileId: null,
  isLoading: false,
  error: null,

  // Project actions
  loadProjects: async () => {
    set({ isLoading: true, error: null });
    try {
      const projects = await invoke<TranslationProject[]>("get_all_projects");
      set({ projects, isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  createProject: async (name, description = "", sourceLang = "auto", targetLang = "zh") => {
    set({ isLoading: true, error: null });
    try {
      const project = await invoke<TranslationProject>("create_project", {
        input: { name, description, sourceLang, targetLang },
      });
      set((state) => ({
        projects: [project, ...state.projects],
        isLoading: false,
      }));
      return project;
    } catch (err) {
      set({ error: String(err), isLoading: false });
      throw err;
    }
  },

  updateProject: async (id, updates) => {
    set({ isLoading: true, error: null });
    try {
      const updated = await invoke<TranslationProject>("update_project", {
        id,
        input: updates,
      });
      set((state) => ({
        projects: state.projects.map((p) => (p.id === id ? updated : p)),
        currentProject: state.currentProject?.id === id ? updated : state.currentProject,
        isLoading: false,
      }));
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  deleteProject: async (id) => {
    set({ isLoading: true, error: null });
    try {
      await invoke("delete_project", { id });
      set((state) => ({
        projects: state.projects.filter((p) => p.id !== id),
        currentProject: state.currentProject?.id === id ? null : state.currentProject,
        isLoading: false,
      }));
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  setCurrentProject: (project) => {
    set({ currentProject: project, currentFiles: [], currentSegments: [], selectedFileId: null });
    if (project) {
      get().loadProjectFiles(project.id);
    }
  },

  // File actions
  loadProjectFiles: async (projectId) => {
    set({ isLoading: true, error: null });
    try {
      const files = await invoke<ProjectFile[]>("get_project_files", { projectId });
      set({ currentFiles: files, isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  addFile: async (projectId, fileName, filePath, fileType, fileSize) => {
    set({ isLoading: true, error: null });
    try {
      const file = await invoke<ProjectFile>("add_file_to_project", {
        projectId,
        input: { fileName, filePath, fileType, fileSize },
      });
      set((state) => ({
        currentFiles: [...state.currentFiles, file],
        isLoading: false,
      }));
      // Reload project to update counts
      get().loadProjects();
      return file;
    } catch (err) {
      set({ error: String(err), isLoading: false });
      throw err;
    }
  },

  deleteFile: async (fileId) => {
    set({ isLoading: true, error: null });
    try {
      await invoke("delete_file", { fileId });
      set((state) => ({
        currentFiles: state.currentFiles.filter((f) => f.id !== fileId),
        selectedFileId: state.selectedFileId === fileId ? null : state.selectedFileId,
        isLoading: false,
      }));
      // Reload project to update counts
      get().loadProjects();
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  setSelectedFile: (fileId) => {
    set({ selectedFileId: fileId, currentSegments: [] });
    if (fileId) {
      get().loadFileSegments(fileId);
    }
  },

  // Segment actions
  loadFileSegments: async (fileId) => {
    set({ isLoading: true, error: null });
    try {
      const segments = await invoke<TranslationSegment[]>("get_file_segments", { fileId });
      set({ currentSegments: segments, isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  addSegments: async (fileId, segments) => {
    set({ isLoading: true, error: null });
    try {
      await invoke("add_segments", {
        fileId,
        input: { segments },
      });
      // Reload segments
      await get().loadFileSegments(fileId);
      // Reload files to update counts
      const currentProject = get().currentProject;
      if (currentProject) {
        await get().loadProjectFiles(currentProject.id);
      }
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  updateSegment: async (segmentId, translatedText, status = "translated") => {
    try {
      const updated = await invoke<TranslationSegment>("update_segment", {
        segmentId,
        input: { translatedText, status },
      });
      set((state) => ({
        currentSegments: state.currentSegments.map((s) =>
          s.id === segmentId ? updated : s
        ),
      }));
      // Reload files and project to update progress
      const currentProject = get().currentProject;
      if (currentProject) {
        await get().loadProjectFiles(currentProject.id);
        // Reload project from backend to get updated progress
        const project = await invoke<TranslationProject>("get_project", { id: currentProject.id });
        set({ currentProject: project });
      }
    } catch (err) {
      set({ error: String(err) });
    }
  },

  // Export
  exportProject: async (projectId) => {
    set({ isLoading: true, error: null });
    try {
      const data = await invoke<ProjectExportData>("export_project", { projectId });
      set({ isLoading: false });
      return data;
    } catch (err) {
      set({ error: String(err), isLoading: false });
      throw err;
    }
  },

  exportProjectJson: async (projectId) => {
    set({ isLoading: true, error: null });
    try {
      const json = await invoke<string>("export_project_json", { projectId });
      set({ isLoading: false });
      return json;
    } catch (err) {
      set({ error: String(err), isLoading: false });
      throw err;
    }
  },

  // Utility
  getProgress: () => {
    const { currentProject } = get();
    if (!currentProject || currentProject.totalSegments === 0) return 0;
    return Math.round((currentProject.translatedSegments / currentProject.totalSegments) * 100);
  },

  clearError: () => set({ error: null }),
}));
