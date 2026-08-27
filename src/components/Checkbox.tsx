import type { CheckState } from "@/lib/filetree";

import { Icon } from "./Icon";

/** Props for {@link Checkbox}. */
export interface CheckboxProps {
  /** Full, empty, or some-but-not-all. */
  state: CheckState;
  /** Accessible label for the thing being checked. */
  label: string;
  /** Called when the box is clicked. */
  onChange: () => void;
}

/**
 * A tri-state checkbox.
 *
 * `partial` is the state that makes folder checkboxes usable at all: without a
 * third mark, a folder with one file deselected looks identical to one with
 * everything deselected, and the user has to open it to find out.
 *
 * Exposed as `aria-checked="mixed"`, which is the real ARIA value for this and
 * is announced properly — not as a checked box with a different picture in it.
 *
 * The border is `line-2` when empty, which is the token that clears 3:1. An
 * unchecked box that cannot be seen is a control that does not exist.
 *
 * @param props - See {@link CheckboxProps}.
 * @returns The rendered checkbox.
 */
export function Checkbox({ state, label, onChange }: CheckboxProps) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={state === "partial" ? "mixed" : state === "on"}
      aria-label={label}
      onClick={(event) => {
        // The row behind this is clickable too; without this the click would
        // toggle twice and land back where it started.
        event.stopPropagation();
        onChange();
      }}
      className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-[4px] border-[1.5px] transition-colors ${
        state === "off"
          ? "border-line-2 bg-transparent"
          : "border-acc bg-acc text-on-acc"
      }`}
    >
      {state === "on" ? <Icon name="check" size={11} /> : null}
      {state === "partial" ? <Icon name="dash" size={11} /> : null}
    </button>
  );
}
