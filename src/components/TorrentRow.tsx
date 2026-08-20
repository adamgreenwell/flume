"use client";

import { formatBytes, formatDuration, formatSpeed } from "@/lib/format";
import type { TorrentSummary, TorrentState } from "@/lib/ipc/types";

import { Button } from "./Button";
import { ProgressBar } from "./ProgressBar";

/** Short label per lifecycle state. */
const STATE_LABEL: Record<TorrentState, string> = {
  checking: "Checking",
  downloading: "Downloading",
  seeding: "Seeding",
  paused: "Paused",
  error: "Error",
};

const STATE_COLOR: Record<TorrentState, string> = {
  checking: "text-muted",
  downloading: "text-accent",
  seeding: "text-ok",
  paused: "text-faint",
  error: "text-error",
};

/**
 * Completion fraction for a summary.
 *
 * Mirrors the Rust `progress_fraction`, including treating a finished torrent
 * as complete before metadata resolves.
 *
 * @param t - The torrent summary.
 * @returns A value in `0..=1`.
 */
export function progressFraction(t: TorrentSummary): number {
  if (t.finished) return 1;
  if (t.totalBytes === 0) return 0;
  return Math.min(Math.max(t.progressBytes / t.totalBytes, 0), 1);
}

/** Props for {@link TorrentRow}. */
export interface TorrentRowProps {
  /** The torrent to render. */
  torrent: TorrentSummary;
  /** Pause or resume, depending on current state. */
  onToggle: (t: TorrentSummary) => void;
  /** Begin removal; the caller is responsible for confirming. */
  onRemove: (t: TorrentSummary) => void;
  /** Reveal the download in the OS file manager. */
  onReveal: (t: TorrentSummary) => void;
}

/**
 * One torrent in the list.
 *
 * @param props - See {@link TorrentRowProps}.
 * @returns The rendered row.
 */
export function TorrentRow({
  torrent,
  onToggle,
  onRemove,
  onReveal,
}: TorrentRowProps) {
  const fraction = progressFraction(torrent);
  const isPaused = torrent.state === "paused";
  const percent = Math.round(fraction * 100);

  return (
    <li className="border-border-subtle bg-surface hover:border-muted/40 rounded-lg border p-4 transition-colors">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <p
            className="text-text truncate text-sm font-medium"
            title={torrent.name}
          >
            {torrent.name}
          </p>
          <p className="text-muted mt-0.5 text-xs">
            <span className={STATE_COLOR[torrent.state]}>
              {STATE_LABEL[torrent.state]}
            </span>
            {" · "}
            <span className="font-mono tabular-nums">
              {formatBytes(torrent.progressBytes)} /{" "}
              {formatBytes(torrent.totalBytes)}
            </span>
            {" · "}
            <span className="font-mono tabular-nums">{percent}%</span>
          </p>
        </div>

        <div className="flex shrink-0 gap-1">
          <Button variant="ghost" onClick={() => onToggle(torrent)}>
            {isPaused ? "Resume" : "Pause"}
          </Button>
          <Button variant="ghost" onClick={() => onReveal(torrent)}>
            Show
          </Button>
          <Button variant="ghost" onClick={() => onRemove(torrent)}>
            Remove
          </Button>
        </div>
      </div>

      <div className="mt-3">
        <ProgressBar
          value={fraction}
          state={torrent.state}
          label={`${torrent.name} download progress`}
        />
      </div>

      <dl className="text-muted mt-2.5 flex flex-wrap gap-x-5 gap-y-1 text-xs">
        <div className="flex gap-1.5">
          <dt className="text-faint">Down</dt>
          <dd className="font-mono tabular-nums">
            {formatSpeed(torrent.downloadBps)}
          </dd>
        </div>
        <div className="flex gap-1.5">
          <dt className="text-faint">Up</dt>
          <dd className="font-mono tabular-nums">
            {formatSpeed(torrent.uploadBps)}
          </dd>
        </div>
        <div className="flex gap-1.5">
          <dt className="text-faint">Peers</dt>
          <dd className="font-mono tabular-nums">{torrent.livePeers}</dd>
        </div>
        {torrent.etaSeconds !== null ? (
          <div className="flex gap-1.5">
            <dt className="text-faint">ETA</dt>
            <dd className="font-mono tabular-nums">
              {formatDuration(torrent.etaSeconds)}
            </dd>
          </div>
        ) : null}
        <div className="flex gap-1.5">
          <dt className="text-faint">Ratio</dt>
          <dd className="font-mono tabular-nums">
            {torrent.progressBytes > 0
              ? (torrent.uploadedBytes / torrent.progressBytes).toFixed(2)
              : "0.00"}
          </dd>
        </div>
      </dl>

      {torrent.error ? (
        <p
          className="border-error/30 bg-error/10 text-error mt-2.5 rounded-md border px-2.5 py-1.5 text-xs"
          role="alert"
        >
          {torrent.error}
        </p>
      ) : null}
    </li>
  );
}
