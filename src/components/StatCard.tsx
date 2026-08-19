import type { ReactNode } from "react";

/** Props for {@link StatCard}. */
export interface StatCardProps {
  /** Short label describing the metric. */
  label: string;
  /** The primary value, rendered in a monospace face for digit alignment. */
  value: ReactNode;
  /** Optional secondary line beneath the value. */
  hint?: ReactNode;
}

/**
 * A single labelled metric in the status grid.
 *
 * Values use a monospace face so that rapidly changing numbers do not cause
 * the surrounding layout to jitter as digit widths change.
 *
 * @param props - See {@link StatCardProps}.
 * @returns The rendered card.
 */
export function StatCard({ label, value, hint }: StatCardProps) {
  return (
    <div className="border-border-subtle bg-surface rounded-lg border p-4">
      <div className="text-faint text-[11px] font-medium tracking-wider uppercase">
        {label}
      </div>
      <div className="text-text mt-1.5 font-mono text-lg tabular-nums">
        {value}
      </div>
      {hint ? <div className="text-muted mt-0.5 text-xs">{hint}</div> : null}
    </div>
  );
}
