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
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
