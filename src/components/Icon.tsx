import type { SVGProps } from "react";

/** Icons available to {@link Icon}. */
export type IconName =
  "pause" | "play" | "folder" | "files" | "trash" | "plus" | "settings";

/**
 * Inline SVG paths, drawn on a 24×24 grid with a 2px stroke.
 *
 * Hand-inlined rather than pulled from an icon package: seven glyphs do not
 * justify a dependency, and inlining keeps them themeable via `currentColor`
 * and free of any network or bundle cost.
 */
const PATHS: Record<IconName, string> = {
  pause: "M10 4H6v16h4V4zm8 0h-4v16h4V4z",
  play: "M6 4l14 8-14 8V4z",
  folder:
    "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z",
  files: "M4 4h9l4 4v12H4V4zm9 0v4h4",
  trash: "M4 7h16M9 7V5h6v2m-8 0v13h10V7M10 11v5m4-5v5",
  plus: "M12 5v14M5 12h14",
  settings:
    "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1.08-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z",
};

/** Glyphs that read better filled than stroked. */
const FILLED: ReadonlySet<IconName> = new Set(["pause", "play"]);

/** Props for {@link Icon}. */
export interface IconProps extends Omit<SVGProps<SVGSVGElement>, "children"> {
  /** Which glyph to draw. */
  name: IconName;
  /** Edge length in pixels. Defaults to 16. */
  size?: number;
}

/**
 * Renders an inline icon that inherits the current text colour.
 *
 * Always `aria-hidden`: every icon in Flume sits inside a control that carries
 * its own accessible name, so announcing the glyph as well would be noise.
 *
 * @param props - See {@link IconProps}.
 * @returns The rendered SVG.
 */
export function Icon({ name, size = 16, ...rest }: IconProps) {
  const filled = FILLED.has(name);
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill={filled ? "currentColor" : "none"}
      stroke={filled ? "none" : "currentColor"}
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...rest}
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
