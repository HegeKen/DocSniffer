import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Standard Tauri v2 + Vite + React configuration.
// https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react()],
  // Use relative asset paths so the production bundle loads correctly under
  // Tauri's custom protocol (absolute `/assets/...` can cause a blank window).
  base: "./",
  clearScreen: false,
  build: {
    // Tauri loads the bundle in the OS webview: WKWebView on macOS tracks the
    // system Safari version, which can lag well behind the "widely available"
    // baseline Vite now targets by default. Pin a conservative target (matching
    // tsconfig's `target: ES2020`) so older macOS builds don't white-screen.
    target: "es2020",
  },
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
