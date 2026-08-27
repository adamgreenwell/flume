import type { ButtonHTMLAttributes } from "react";

import { Icon, type IconName } from "./Icon";

/** Props for {@link IconButton}. */
export interface IconButtonProps extends Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  "children"
> {
  /** Which glyph to show. */
  icon: IconName;
  /**
   * Accessible name, also used as the tooltip.
   *
   * Required: an icon-only control is unusable without one.
   */
  label: string;
  /** Tints the control as destructive. */
  destructive?: boolean;
}

/**
 * A compact icon-only control, drawn at the design's 28px chip height.
 *
 * Sits at `fg-2` until its row is hovered or it takes focus, so a list of
 * torrents reads as content rather than as a wall of buttons. Colour only —
 * the control stays present, hit-testable, and keyboard reachable at all
 * times, unlike the common pattern of hiding actions until hover, which
 * strands keyboard and touch users.
 *
 * 28px is a pointer target, not a touch target. A remote web UI would have to
 * re-scale this to a 44px minimum rather than ship the desktop size to a phone.
 *
 * @param props - See {@link IconButtonProps}.
 * @returns The rendered button.
 */
export function IconButton({
  icon,
  label,
  destructive = false,
  className = "",
  type = "button",
  ...rest
}: IconButtonProps) {
  return (
    <button
      type={type}
      title={label}
      aria-label={label}
      className={`text-fg-2 hover:bg-bg-2 disabled:text-fg-dis inline-flex h-[var(--flume-h-chip)] w-[var(--flume-h-chip)] items-center justify-center rounded-md transition-colors disabled:pointer-events-none ${
        destructive ? "hover:text-err" : "hover:text-fg-0"
      } ${className}`}
      {...rest}
    >
      <Icon name={icon} size={15} />
    </button>
  );
}
