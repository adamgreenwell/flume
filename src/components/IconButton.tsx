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
 * A compact icon-only control.
 *
 * Sits at reduced opacity until its row is hovered or it takes focus, so a
 * list of torrents reads as content rather than as a wall of buttons. Opacity
 * only — the control stays present, hit-testable, and keyboard reachable at
 * all times, unlike the common pattern of hiding actions until hover, which
 * strands keyboard and touch users.
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
      className={`text-muted hover:bg-surface-raised inline-flex h-7 w-7 items-center justify-center rounded-md opacity-70 transition-[opacity,color,background-color] group-hover:opacity-100 focus-visible:opacity-100 disabled:pointer-events-none disabled:opacity-30 ${
        destructive ? "hover:text-error" : "hover:text-text"
      } ${className}`}
      {...rest}
    >
      <Icon name={icon} />
    </button>
  );
}
