import type { Theme } from "@/lib/ipc/types";

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
