import type { Decorator, Preview } from "@storybook/nextjs-vite";
import { useEffect } from "react";

import "../src/app/globals.css";

/**
 * Applies the selected theme the same way the app does.
 *
 * Flips `data-theme` on the document element rather than wrapping the story in
 * a themed container. The tokens are declared on `:root`, and "system" works by
 * removing the attribute so the `prefers-color-scheme` media query stays
 * authoritative — a wrapper would test a mechanism the app does not use and
 * quietly stop catching bugs in the one it does.
 */
function ThemeRoot({
  theme,
  children,
}: {
  theme: "system" | "dark" | "light";
  children: React.ReactNode;
}) {
  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", theme);
  }, [theme]);

  return <>{children}</>;
}

const withTheme: Decorator = (Story, context) => (
  <ThemeRoot theme={context.globals.theme as "system" | "dark" | "light"}>
    <Story />
  </ThemeRoot>
);

const preview: Preview = {
  decorators: [withTheme],
  globalTypes: {
    theme: {
      description: "Flume theme",
      toolbar: {
        title: "Theme",
        icon: "circlehollow",
        items: [
          { value: "dark", title: "Dark" },
          { value: "light", title: "Light" },
          { value: "system", title: "System" },
        ],
        dynamicTitle: true,
      },
    },
  },
  initialGlobals: {
    // Dark is the design's primary theme, so it is what a story shows first.
    theme: "dark",
  },
  parameters: {
    // The palette comes from the tokens. Storybook's own background swatches
    // would let a story sit on a colour the app can never produce.
    backgrounds: { disable: true },
    controls: { matchers: { color: /(background|color)$/i } },
    a11y: { test: "error" },
  },
};

export default preview;
