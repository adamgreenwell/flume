import type { ButtonHTMLAttributes, ReactNode } from "react";

/** Visual weight of a button. */
export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary: "bg-accent text-bg hover:bg-accent-dim font-medium",
  secondary:
    "bg-surface-raised text-text border border-border-subtle hover:border-muted",
  ghost: "text-muted hover:text-text hover:bg-surface-raised",
  danger: "bg-error/15 text-error border border-error/30 hover:bg-error/25",
};

/** Props for {@link Button}. */
export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** Visual weight. Defaults to `secondary`. */
  variant?: ButtonVariant;
  /** Button content. */
  children: ReactNode;
}

/**
 * A styled button.
 *
 * Always sets an explicit `type`, defaulting to `"button"` — an unset type
 * inside a form defaults to `submit`, which is a classic accidental-submit bug.
 *
 * @param props - See {@link ButtonProps}.
 * @returns The rendered button.
 */
export function Button({
  variant = "secondary",
  className = "",
  type = "button",
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={`inline-flex items-center justify-center gap-1.5 rounded-md px-3 py-1.5 text-sm transition-colors disabled:pointer-events-none disabled:opacity-40 ${VARIANT_CLASSES[variant]} ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
