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
    // The collector's tests run here too, so `npm run check`, CI and
    // preflight all cover the Worker without a fourth list to keep in step.
    // They opt into the node environment per file.
    include: [
      "src/**/*.test.{ts,tsx}",
      "collector/test/**/*.test.ts",
      "scripts/**/*.test.mjs",
    ],
    setupFiles: ["./vitest.setup.ts"],
  },
});
