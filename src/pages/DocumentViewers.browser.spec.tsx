import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { listen } from "@tauri-apps/api/event";
import PdfViewer from "./PdfViewer";
import SubtitleViewer from "./SubtitleViewer";

describe("document viewers browser runtime", () => {
  beforeEach(() => {
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
    vi.mocked(listen).mockReset();
  });

  test("does not register PDF OCR progress listener outside Tauri", async () => {
    render(<PdfViewer />);

    await waitFor(() => {
      expect(listen).not.toHaveBeenCalled();
    });
  });

  test("does not register subtitle progress listener outside Tauri", async () => {
    render(<SubtitleViewer />);

    await waitFor(() => {
      expect(listen).not.toHaveBeenCalled();
    });
  });
});
