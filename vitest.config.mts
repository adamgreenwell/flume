import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

/**
 * Vitest configuration for Flume's frontend logic.
 *
 * Tauri's `invoke` is mocked with `mockIPC` from `@tauri-apps/api/mocks`, so
 * these tests never need a running Rust backend or a WebView.
 *
 * The `.mts` extension keeps this file unambiguously ESM, which avoids Vite's
 * CommonJS config-loader deprecation warning.
 */
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["./vitest.setup.ts"],
  },
});
