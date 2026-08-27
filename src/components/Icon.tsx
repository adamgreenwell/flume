import type { SVGProps } from "react";

/** Icons available to {@link Icon}. */
export type IconName =
  | "pause"
  | "play"
  | "folder"
  | "files"
  | "trash"
  | "plus"
  | "settings"
  | "search"
  | "chevron-right"
  | "chevron-down"
  | "arrow-down"
  | "arrow-up"
  | "check"
  | "dash"
  | "clock"
  | "check-circle"
  | "alert-circle"
  | "alert-triangle";

/**
 * Inline SVG paths on the design's 16×16 grid.
 *
 * Everything is stroked — the design has no filled glyphs, including pause,
 * which other clients usually draw as two solid bars. That is deliberate:
 * "generous hairlines instead of heavy borders" applies to icons too, and a
 * filled glyph sitting beside stroked ones reads as a different weight class.
 *
 * Hand-inlined rather than pulled from an icon package. These are the design's
 * own glyphs at its own optical weight, which no general-purpose set matches,
 * and inlining keeps them themeable via `currentColor` with no bundle cost.
 *
 * Marked `[undesigned]` below: three glyphs the design never draws. Built in
 * the same idiom at the same metrics and flagged for review rather than
 * borrowed from another vocabulary.
 */
const PATHS: Record<IconName, string> = {
  pause: "M6.2 3.6v8.8M9.8 3.6v8.8",
  // [undesigned] Matches pause's 3.4–12.6 vertical extent so the two glyphs
  // swap in place without the row shifting.
  play: "M5.8 3.4 12.4 8l-6.6 4.6z",
  folder:
    "M1.8 4.2a1.4 1.4 0 0 1 1.4-1.4h2.6l1.5 1.7h5.5a1.4 1.4 0 0 1 1.4 1.4v6.2a1.4 1.4 0 0 1-1.4 1.4H3.2a1.4 1.4 0 0 1-1.4-1.4z",
  files: "M4 2.4h5l3 3v8.2H4zM9 2.4v3.2h3",
  // [undesigned]
  trash:
    "M2.8 4.4h10.4M6.4 4.4V2.8h3.2v1.6M4.2 4.4v8.2a1 1 0 0 0 1 1h5.6a1 1 0 0 0 1-1V4.4M6.6 6.8v4.4M9.4 6.8v4.4",
  plus: "M8 3.5v9M3.5 8h9",
  // [undesigned] Sliders rather than a gear: a 16px gear with a 1.5 stroke
  // turns to mush, and the design's idiom is plain geometry.
  settings:
    "M2.4 5.2h3.2M9.2 5.2h4.4M2.4 10.8h4.4M10 10.8h3.6M8.6 5.2a1.4 1.4 0 1 1-2.8 0 1.4 1.4 0 0 1 2.8 0M9.6 10.8a1.4 1.4 0 1 1-2.8 0 1.4 1.4 0 0 1 2.8 0",
  search: "M11.3 7a4.3 4.3 0 1 1-8.6 0 4.3 4.3 0 0 1 8.6 0m-1.1 3.2 3 3",
  "chevron-right": "m6.2 4 4 4-4 4",
  "chevron-down": "m4 6.2 4 4 4-4",
  "arrow-down": "M8 2.6v8.4M4.6 7.6 8 11l3.4-3.4M3 13.4h10",
  "arrow-up": "M8 13.4V5M4.6 8.4 8 5l3.4 3.4M3 2.6h10",
  check: "m3.2 8.4 2.7 2.7 6.9-6.9",
  dash: "M4 8h8",
  clock: "M8 2.2a5.8 5.8 0 1 0 0 11.6 5.8 5.8 0 0 0 0-11.6zM8 5v3.4l2.2 1.4",
  "check-circle":
    "M8 2.2a5.8 5.8 0 1 0 0 11.6 5.8 5.8 0 0 0 0-11.6zM5.4 8.2l1.9 1.9 3.5-4",
  "alert-circle": "M8 2a6 6 0 1 0 0 12A6 6 0 0 0 8 2zM8 4.8v3.6M8 11v.1",
  "alert-triangle": "M8 2.2 14.8 13.8H1.2zM8 6.4v3.2M8 11.8v.1",
};

/**
 * Optical stroke weight in device pixels, held constant across sizes.
 *
 * The design draws at 1.5 on a 16 grid. Because `stroke-width` is in user
 * units, a fixed 1.5 would render heavier as the glyph shrinks and lighter as
 * it grows — so it is scaled by the grid-to-size ratio instead, and a 20px
 * icon sits beside a 14px one at the same weight.
 */
const STROKE_PX = 1.5;

/** The grid every path above is drawn on. */
const GRID = 16;

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
  return (
    <svg
      width={size}
      height={size}
      viewBox={`0 0 ${GRID} ${GRID}`}
      fill="none"
      stroke="currentColor"
      strokeWidth={(GRID / size) * STROKE_PX}
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
