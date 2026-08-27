import type { ButtonHTMLAttributes, ReactNode } from "react";

/** Props for {@link Chip}. */
export interface ChipProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** Whether this chip is the active choice. */
  selected?: boolean;
  /** Chip content. */
  children: ReactNode;
}

/**
 * A 28px toggle in the app chrome — sort order, density, filters.
 *
 * Selection is carried by `aria-pressed` as well as by fill, so a screen
 * reader hears which sort is active rather than only seeing it.
 *
 * @param props - See {@link ChipProps}.
 * @returns The rendered chip.
 */
export function Chip({
  selected = false,
  className = "",
  type = "button",
  children,
  ...rest
}: ChipProps) {
  return (
    <button
      type={type}
      aria-pressed={selected}
      className={`inline-flex h-[var(--flume-h-chip)] items-center gap-1.5 rounded-sm border px-2.5 text-xs transition-colors ${
        selected
          ? "bg-bg-3 border-line-2 text-fg-0"
          : "border-line text-fg-1 hover:bg-bg-2 hover:text-fg-0 bg-transparent"
      } ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
