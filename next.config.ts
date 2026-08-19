import type { NextConfig } from "next";

/**
 * Flume runs entirely inside a Tauri v2 WebView, which serves the frontend as
 * static files from disk (`tauri://` / `http://tauri.localhost`). There is no
 * Node.js server at runtime, so the app MUST be a pure static export.
 *
 * Consequences enforced here and in review:
 * - `output: "export"` emits plain HTML/CSS/JS into `out/`, which
 *   `tauri.conf.json` points at via `build.frontendDist`.
 * - `images.unoptimized` is required because `next/image` optimization needs a
 *   server; without this the export fails.
 * - `trailingSlash` makes every route emit `<route>/index.html`, which resolves
 *   correctly under the WebView's custom protocol handler.
 *
 * Anything requiring a server runtime (route handlers under `app/api/*`,
 * `cookies()`, `headers()`, ISR, middleware) is unavailable by construction.
 * All backend work goes through Tauri `invoke` instead.
 */
const nextConfig: NextConfig = {
  output: "export",
  images: { unoptimized: true },
  trailingSlash: true,
  reactStrictMode: true,
};

export default nextConfig;
