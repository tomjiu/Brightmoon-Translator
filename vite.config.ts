import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

const ignoreGlobs = ["**/src-tauri/**", "**/tmp/**", "**/extension/**", "**/docs/**"];

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  // Only app frontend — never scan OSS reference clones under tmp/
  optimizeDeps: {
    entries: ["index.html", "src/**/*.{ts,tsx}"],
    exclude: ["src-tauri"],
  },
  server: {
    port: 5173,
    strictPort: true,
    // Force IPv4 — Windows often binds only [::1]; WebView uses 127.0.0.1 → blank window
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 5173,
        }
      : undefined,
    watch: {
      ignored: ignoreGlobs,
    },
    fs: {
      deny: ["**/src-tauri/**", "**/tmp/**"],
      allow: ["."],
    },
  },
}));
