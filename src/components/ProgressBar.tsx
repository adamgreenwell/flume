import type { TorrentState } from "@/lib/ipc/types";

/** Bar colour per lifecycle state. */
const FILL_BY_STATE: Record<TorrentState, string> = {
  checking: "bg-muted",
  downloading: "bg-accent",
  seeding: "bg-ok",
  paused: "bg-faint",
  error: "bg-error",
};

/** Props for {@link ProgressBar}. */
export interface ProgressBarProps {
  /** Completion in `0.0..=1.0`. */
  value: number;
  /** Drives the fill colour. */
  state: TorrentState;
  /** Accessible label describing what is progressing. */
  label: string;
}

/**
 * A thin completion bar.
 *
 * Exposed as a real `progressbar` role with `aria-valuenow`, so the percentage
 * is available to a screen reader rather than only being implied by width.
 *
 * @param props - See {@link ProgressBarProps}.
 * @returns The rendered bar.
 */
export function ProgressBar({ value, state, label }: ProgressBarProps) {
  const percent = Math.round(Math.min(Math.max(value, 0), 1) * 100);

  return (
    <div
      className="bg-surface-raised h-1.5 w-full overflow-hidden rounded-full"
      role="progressbar"
      aria-label={label}
      aria-valuenow={percent}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div
        className={`h-full rounded-full transition-[width] duration-500 ease-out ${FILL_BY_STATE[state]}`}
        style={{ width: `${percent}%` }}
      />
    </div>
  );
}
