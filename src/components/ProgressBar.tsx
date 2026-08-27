import type { TorrentState } from "@/lib/ipc/types";

/**
 * Bar colour per lifecycle state.
 *
 * Status colours are reserved: `ok` means the thing finished, `warn` means it
 * is re-hashing, `err` means it stopped on a failure. Paused and queued get
 * `fg-3` rather than a status colour because neither is a condition — the user
 * asked for one and the scheduler caused the other, and colouring them would
 * spend a status colour on a non-event.
 */
const FILL_BY_STATE: Record<TorrentState, string> = {
  checking: "bg-warn",
  downloading: "bg-acc",
  seeding: "bg-ok",
  paused: "bg-fg-3",
  error: "bg-err",
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
 * A thin completion bar with its percentage beside it.
 *
 * The numeric label is part of the component rather than something call sites
 * remember to add. At 5px tall the bar alone is unreadable below about 10% —
 * a 3% fill and a 0% fill are the same two pixels — so a bar without a number
 * is a bar that lies at exactly the moment the user cares most.
 *
 * Exposed as a real `progressbar` role with `aria-valuenow`, and the visible
 * percentage is hidden from assistive tech so the value is announced once
 * rather than twice.
 *
 * @param props - See {@link ProgressBarProps}.
 * @returns The rendered bar.
 */
export function ProgressBar({ value, state, label }: ProgressBarProps) {
  const percent = Math.round(Math.min(Math.max(value, 0), 1) * 100);

  return (
    <div className="flex items-center gap-2.5">
      <div
        className="bg-bg-3 h-[5px] w-full overflow-hidden rounded-[3px]"
        role="progressbar"
        aria-label={label}
        aria-valuenow={percent}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <div
          className={`h-full rounded-[3px] transition-[width] duration-500 ease-out ${FILL_BY_STATE[state]}`}
          style={{ width: `${percent}%` }}
        />
      </div>
      <span
        className="flume-num text-fg-1 w-[38px] shrink-0 text-right text-[11.5px]"
        aria-hidden="true"
      >
        {percent}%
      </span>
    </div>
  );
}
