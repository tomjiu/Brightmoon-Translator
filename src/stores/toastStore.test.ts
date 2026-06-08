import { describe, it, expect, vi, beforeEach } from "vitest";
import { useToastStore } from "./toastStore";

describe("toastStore", () => {
  beforeEach(() => {
    // Reset store state
    useToastStore.setState({ toasts: [] });
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("addToast", () => {
    it("should add a toast with generated id", () => {
      const { addToast } = useToastStore.getState();

      addToast({
        type: "info",
        message: "Test message",
        duration: 3000,
      });

      const { toasts } = useToastStore.getState();
      expect(toasts).toHaveLength(1);
      expect(toasts[0].message).toBe("Test message");
      expect(toasts[0].type).toBe("info");
      expect(toasts[0].duration).toBe(3000);
      expect(toasts[0].id).toMatch(/^toast-\d+$/);
    });

    it("should add multiple toasts", () => {
      const { addToast } = useToastStore.getState();

      addToast({ type: "info", message: "First", duration: 3000 });
      addToast({ type: "error", message: "Second", duration: 5000 });

      const { toasts } = useToastStore.getState();
      expect(toasts).toHaveLength(2);
      expect(toasts[0].message).toBe("First");
      expect(toasts[1].message).toBe("Second");
    });

    it("should auto-remove toast after duration", () => {
      const { addToast } = useToastStore.getState();

      addToast({ type: "info", message: "Auto remove", duration: 1000 });

      expect(useToastStore.getState().toasts).toHaveLength(1);

      // Fast-forward time
      vi.advanceTimersByTime(1000);

      expect(useToastStore.getState().toasts).toHaveLength(0);
    });

    it("should support optional detail field", () => {
      const { addToast } = useToastStore.getState();

      addToast({
        type: "warning",
        message: "Warning",
        detail: "Detailed info",
        duration: 3000,
      });

      const { toasts } = useToastStore.getState();
      expect(toasts[0].detail).toBe("Detailed info");
    });
  });

  describe("removeToast", () => {
    it("should remove a specific toast by id", () => {
      const { addToast } = useToastStore.getState();

      addToast({ type: "info", message: "First", duration: 3000 });
      addToast({ type: "error", message: "Second", duration: 3000 });

      const { toasts } = useToastStore.getState();
      const firstId = toasts[0].id;

      useToastStore.getState().removeToast(firstId);

      const remaining = useToastStore.getState().toasts;
      expect(remaining).toHaveLength(1);
      expect(remaining[0].message).toBe("Second");
    });

    it("should do nothing if toast id does not exist", () => {
      const { addToast } = useToastStore.getState();

      addToast({ type: "info", message: "Test", duration: 3000 });

      useToastStore.getState().removeToast("non-existent-id");

      expect(useToastStore.getState().toasts).toHaveLength(1);
    });
  });

  describe("clearAll", () => {
    it("should remove all toasts", () => {
      const { addToast } = useToastStore.getState();

      addToast({ type: "info", message: "First", duration: 3000 });
      addToast({ type: "error", message: "Second", duration: 3000 });
      addToast({ type: "success", message: "Third", duration: 3000 });

      expect(useToastStore.getState().toasts).toHaveLength(3);

      useToastStore.getState().clearAll();

      expect(useToastStore.getState().toasts).toHaveLength(0);
    });
  });

  describe("Toast types", () => {
    it("should support all toast types", () => {
      const { addToast } = useToastStore.getState();
      const types = ["info", "success", "warning", "error"] as const;

      types.forEach((type) => {
        addToast({ type, message: `${type} message`, duration: 3000 });
      });

      const { toasts } = useToastStore.getState();
      expect(toasts).toHaveLength(4);
      expect(toasts.map((t) => t.type)).toEqual(types);
    });
  });
});
