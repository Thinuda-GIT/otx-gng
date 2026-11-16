/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tauri expects a fixed dev port and must not clear its own console output.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: "chrome110", // WebView2 is Chromium-based and evergreen
    sourcemap: false,
    // Loaded from local disk inside WebView2, so a ~500 kB vendor chunk is fine.
    chunkSizeWarningLimit: 700,
  },
  test: { include: ["src/**/*.test.ts"], environment: "node" },
});
