import type { ButtonHTMLAttributes, ReactNode } from "react";

/** Visual weight of a button. */
export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

/**
 * Which of the design's two button scales to use.
 *
 * `control` is the 30px chrome button that appears in toolbars and panels.
 * `dialog` is the 34px one a sheet uses for its single primary action — it is
 * larger because it is the decision the whole screen was built to ask.
 */
export type ButtonSize = "control" | "dialog";

const SIZE_CLASSES: Record<ButtonSize, string> = {
  control: "h-[var(--flume-h-control)] gap-[7px] px-[13px]",
  dialog: "h-[var(--flume-h-primary)] gap-2 px-4",
};

/**
 * Per-variant colour, including the disabled treatment.
 *
 * Disabled is an explicit recolour rather than a blanket opacity knock-down.
 * Fading the primary variant leaves accent-tinted text on an accent-tinted
 * fill, which reads as "still clickable, just dim"; the design instead drops
 * the control to a flat surface so it stops looking like an action at all.
 *
 * The disabled ink is `fg-3`, not `fg-dis`, because a disabled primary button
 * still has to be readable — it usually names the thing you cannot yet do
 * ("Add 6 files · 46.1 GB"). `fg-dis` is reserved for controls whose label
 * carries nothing the user needs.
 */
const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary:
    "bg-acc border-acc text-on-acc hover:bg-acc-hi hover:border-acc-hi " +
    "disabled:bg-bg-3 disabled:border-line disabled:text-fg-3",
  secondary:
    "bg-bg-2 border-line text-fg-0 hover:border-line-2 " +
    "disabled:bg-bg-3 disabled:border-line disabled:text-fg-dis",
  ghost:
    "border-transparent bg-transparent text-fg-1 hover:bg-bg-2 hover:text-fg-0 " +
    "disabled:bg-transparent disabled:text-fg-dis",
  // [undesigned] The system sheet has no destructive button. Built from its
  // vocabulary — status colour, same anatomy, tint only on hover so a remove
  // action does not shout from across the window. Flagged for review.
  danger:
    "border-err/40 bg-transparent text-err hover:bg-err/15 hover:border-err " +
    "disabled:border-line disabled:bg-transparent disabled:text-fg-dis",
};

/** Props for {@link Button}. */
export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  /** Visual weight. Defaults to `secondary`. */
  variant?: ButtonVariant;
  /** Which scale to draw at. Defaults to `control`. */
  size?: ButtonSize;
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
  size = "control",
  className = "",
  type = "button",
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={`inline-flex items-center justify-center rounded-md border text-[12.5px] font-semibold whitespace-nowrap transition-colors disabled:pointer-events-none ${SIZE_CLASSES[size]} ${VARIANT_CLASSES[variant]} ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
