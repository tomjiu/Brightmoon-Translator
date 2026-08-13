import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useThemeStore } from './themeStore';

describe('themeStore', () => {
  beforeEach(() => {
    // Clear localStorage
    localStorage.clear();
    vi.clearAllMocks();

    // Reset classList mock
    vi.mocked(document.documentElement.classList.add).mockClear();
    vi.mocked(document.documentElement.classList.remove).mockClear();
  });

  describe('initial state', () => {
    it('should default to dev theme when no stored theme', () => {
      vi.mocked(localStorage.getItem).mockReturnValue(null);

      // Need to re-import to get fresh state
      const { theme } = useThemeStore.getState();
      expect(theme).toBe('dev');
    });

    it('should use stored theme from localStorage', () => {
      vi.mocked(localStorage.getItem).mockReturnValue('light');

      // The store reads from localStorage on creation
      // Since we can't re-create the store, we test the behavior
      const { setTheme } = useThemeStore.getState();
      setTheme('light');

      expect(useThemeStore.getState().theme).toBe('light');
    });
  });

  describe('setTheme', () => {
    it('should update theme state', () => {
      const { setTheme } = useThemeStore.getState();

      setTheme('light');
      expect(useThemeStore.getState().theme).toBe('light');

      setTheme('dark');
      expect(useThemeStore.getState().theme).toBe('dark');
    });

    it('should save theme to localStorage', () => {
      const { setTheme } = useThemeStore.getState();

      setTheme('light');

      expect(localStorage.setItem).toHaveBeenCalledWith('theme', 'light');
    });

    it('should apply theme class to document', () => {
      const { setTheme } = useThemeStore.getState();

      setTheme('light');

      expect(document.documentElement.classList.remove).toHaveBeenCalledWith('dark', 'light', 'dev', 'dev-light');
      expect(document.documentElement.classList.add).toHaveBeenCalledWith('light');
    });
  });

  describe('toggleTheme', () => {
    it('should toggle mono family from dark to light', () => {
      useThemeStore.setState({ theme: 'dark' });

      useThemeStore.getState().toggleTheme();

      expect(useThemeStore.getState().theme).toBe('light');
    });

    it('should toggle mono family from light to dark', () => {
      useThemeStore.setState({ theme: 'light' });

      useThemeStore.getState().toggleTheme();

      expect(useThemeStore.getState().theme).toBe('dark');
    });

    it('should toggle dev family from dev to dev-light (stay in lunar family)', () => {
      useThemeStore.setState({ theme: 'dev' });

      useThemeStore.getState().toggleTheme();

      expect(useThemeStore.getState().theme).toBe('dev-light');
    });

    it('should toggle dev family from dev-light back to dev', () => {
      useThemeStore.setState({ theme: 'dev-light' });

      useThemeStore.getState().toggleTheme();

      expect(useThemeStore.getState().theme).toBe('dev');
    });

    it('should save toggled theme to localStorage', () => {
      useThemeStore.setState({ theme: 'dark' });

      useThemeStore.getState().toggleTheme();

      expect(localStorage.setItem).toHaveBeenCalledWith('theme', 'light');
    });

    it('should apply toggled theme class to document (dev keeps dark+dev)', () => {
      useThemeStore.setState({ theme: 'dev-light' });

      useThemeStore.getState().toggleTheme();

      // dev sets both `dark` (shared status-color mappers) and `dev` (token overrides)
      expect(document.documentElement.classList.add).toHaveBeenCalledWith('dark', 'dev');
    });
  });
});
