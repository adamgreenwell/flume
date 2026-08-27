import type { Density, Theme } from "@/lib/ipc/types";

/**
 * Applies a theme by setting `data-theme` on the document root.
 *
 * `"system"` removes the attribute entirely rather than resolving the
 * preference in JavaScript, which lets the CSS `prefers-color-scheme` media
 * query stay authoritative — so the app follows the OS live if the user
 * switches appearance while it is open.
 *
 * @param theme - The preference to apply.
 */
export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}

/**
 * Applies a row density by setting `data-density` on the document root.
 *
 * Comfortable removes the attribute rather than setting it, so the default
 * lives in one place — the `:root` block in `globals.css` — instead of being
 * asserted from both CSS and JavaScript.
 *
 * @param density - The density to apply.
 */
export function applyDensity(density: Density): void {
  const root = document.documentElement;
  if (density === "compact") root.setAttribute("data-density", "compact");
  else root.removeAttribute("data-density");
}
