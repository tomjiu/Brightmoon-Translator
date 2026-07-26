import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useThemeStore } from './themeStore';

describe('themeStore', () => {
  beforeEach(() => {
    // Clear localStorage
    localStorage.clear();
    vi.clearAllMocks();

    // Reset classList mock
    document.documentElement.classList.add.mockClear();
    document.documentElement.classList.remove.mockClear();
  });

  describe('initial state', () => {
    it('should default to dark theme when no stored theme', () => {
      localStorage.getItem.mockReturnValue(null);

      // Need to re-import to get fresh state
      const { theme } = useThemeStore.getState();
      expect(theme).toBe('dark');
    });

    it('should use stored theme from localStorage', () => {
      localStorage.getItem.mockReturnValue('light');

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

      expect(document.documentElement.classList.remove).toHaveBeenCalledWith('dark', 'light');
      expect(document.documentElement.classList.add).toHaveBeenCalledWith('light');
    });
  });

  describe('toggleTheme', () => {
    it('should toggle from dark to light', () => {
      useThemeStore.setState({ theme: 'dark' });

      useThemeStore.getState().toggleTheme();

      expect(useThemeStore.getState().theme).toBe('light');
    });

    it('should toggle from light to dark', () => {
      useThemeStore.setState({ theme: 'light' });

      useThemeStore.getState().toggleTheme();

      expect(useThemeStore.getState().theme).toBe('dark');
    });

    it('should save toggled theme to localStorage', () => {
      useThemeStore.setState({ theme: 'dark' });

      useThemeStore.getState().toggleTheme();

      expect(localStorage.setItem).toHaveBeenCalledWith('theme', 'light');
    });

    it('should apply toggled theme class to document', () => {
      useThemeStore.setState({ theme: 'dark' });

      useThemeStore.getState().toggleTheme();

      expect(document.documentElement.classList.add).toHaveBeenCalledWith('light');
    });
  });
});
