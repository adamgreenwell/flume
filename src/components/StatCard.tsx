import type { ReactNode } from "react";

/**
 * Which of the design's two stat scales to draw at.
 *
 * `dock` is the compact aggregate readout along the bottom of the library —
 * label and value, no caption, no chrome. `strip` is the seven-across band at
 * the top of the inspector, where each stat is bigger and separated from its
 * neighbour by a hairline.
 */
export type StatCardSize = "dock" | "strip";

const SIZE_CLASSES: Record<StatCardSize, string> = {
  dock: "gap-[3px]",
  strip: "border-line gap-[5px] border-r px-5 py-4 last:border-r-0",
};

const VALUE_CLASSES: Record<StatCardSize, string> = {
  dock: "text-base tracking-[-0.015em]",
  strip: "text-xl leading-[1.1] tracking-[-0.028em]",
};

/** Props for {@link StatCard}. */
export interface StatCardProps {
  /** Short label describing the metric. */
  label: string;
  /** The primary value, rendered in a monospace face for digit alignment. */
  value: ReactNode;
  /** Optional caption beneath the value. Ignored at `dock` size. */
  hint?: ReactNode;
  /** Which scale to draw at. Defaults to `dock`. */
  size?: StatCardSize;
}

/**
 * A single labelled metric: label above, mono value, caption below.
 *
 * Values are mono with tabular figures so a number updating at 1 Hz does not
 * shove the ones beside it sideways as digit widths change — the reason every
 * figure in the app is mono, not a stylistic preference.
 *
 * The label sits at `fg-3`, which is the floor that still clears 4.5:1. It is
 * 10px and uppercase, so it has no contrast headroom to give away.
 *
 * @param props - See {@link StatCardProps}.
 * @returns The rendered stat.
 */
export function StatCard({ label, value, hint, size = "dock" }: StatCardProps) {
  return (
    <div className={`flex flex-col ${SIZE_CLASSES[size]}`}>
      <span className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
        {label}
      </span>
      <span
        className={`flume-num text-fg-0 font-medium ${VALUE_CLASSES[size]}`}
      >
        {value}
      </span>
      {hint && size === "strip" ? (
        <span className="text-fg-2 text-[11px]">{hint}</span>
      ) : null}
    </div>
  );
}
