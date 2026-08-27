import type { StorybookConfig } from "@storybook/nextjs-vite";

/**
 * Storybook configuration.
 *
 * Uses the Vite-based Next.js framework rather than the webpack one so the
 * harness shares a bundler with Vitest, which is already in the tree. The
 * webpack variant would add a second, differently-configured build of the same
 * source — a place for "works in tests, breaks in Storybook" to hide.
 */
const config: StorybookConfig = {
  stories: ["../src/**/*.stories.@(ts|tsx)"],
  addons: [
    "@storybook/addon-docs",
    // The design's accessibility rules are non-negotiable and easy to break by
    // accident — contrast floors, focus visibility, status never by colour
    // alone. Running axe beside every story catches the regressions that a
    // screenshot does not.
    "@storybook/addon-a11y",
  ],
  framework: {
    name: "@storybook/nextjs-vite",
    options: {},
  },
  // Flume is a BitTorrent client. Shipping a dev tool that phones home by
  // default sits badly beside that, and CI has no business making the call.
  core: { disableTelemetry: true },
};

export default config;
