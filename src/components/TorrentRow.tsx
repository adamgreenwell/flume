"use client";

import { formatBytes, formatDuration, formatSpeed } from "@/lib/format";
import type { TorrentState, TorrentSummary } from "@/lib/ipc/types";

import { IconButton } from "./IconButton";
import { ProgressBar } from "./ProgressBar";

/** Short label per lifecycle state. */
const STATE_LABEL: Record<TorrentState, string> = {
  checking: "Checking",
  downloading: "Downloading",
  seeding: "Seeding",
  paused: "Paused",
  error: "Error",
};

/** Indicator colour per lifecycle state. */
const STATE_DOT: Record<TorrentState, string> = {
  checking: "bg-muted",
  downloading: "bg-accent",
  seeding: "bg-ok",
  paused: "bg-faint",
  error: "bg-error",
};

/**
 * Completion fraction for a summary.
 *
 * Mirrors the Rust `progress_fraction`, including treating a finished torrent
 * as complete before its metadata resolves.
 *
 * @param t - The torrent summary.
 * @returns A value in `0..=1`.
 */
export function progressFraction(t: TorrentSummary): number {
  if (t.finished) return 1;
  if (t.totalBytes === 0) return 0;
  return Math.min(Math.max(t.progressBytes / t.totalBytes, 0), 1);
}

/** One statistic in the row's footer. */
function Stat({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="flex items-baseline gap-1.5">
      <dt className="text-faint">{label}</dt>
      <dd className="text-muted font-mono tabular-nums">{value}</dd>
    </div>
  );
}

/** Props for {@link TorrentRow}. */
export interface TorrentRowProps {
  /** The torrent to render. */
  torrent: TorrentSummary;
  /** Pause or resume, depending on current state. */
  onToggle: (t: TorrentSummary) => void;
  /** Begin removal; the caller confirms. */
  onRemove: (t: TorrentSummary) => void;
  /** Reveal the download in the OS file manager. */
  onReveal: (t: TorrentSummary) => void;
  /** Open the per-torrent detail panel. */
  onOpenDetail: (t: TorrentSummary) => void;
  /** Right-clicked, with the pointer position in viewport coordinates. */
  onContextMenu: (t: TorrentSummary, at: { x: number; y: number }) => void;
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
  onOpenDetail,
  onContextMenu,
}: TorrentRowProps) {
  const fraction = progressFraction(torrent);
  const isPaused = torrent.state === "paused";
  const percent = Math.round(fraction * 100);
  const ratio =
    torrent.progressBytes > 0
      ? (torrent.uploadedBytes / torrent.progressBytes).toFixed(2)
      : "0.00";

  return (
    <li
      className="group border-border-subtle bg-surface hover:border-muted/40 rounded-lg border px-4 py-3.5 transition-colors"
      onContextMenu={(event) => {
        event.preventDefault();
        onContextMenu(torrent, { x: event.clientX, y: event.clientY });
      }}
    >
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <p
            className="text-text truncate text-[0.9375rem] leading-snug font-medium"
            title={torrent.name}
          >
            {torrent.name}
          </p>
          <div className="text-muted mt-1 flex items-center gap-1.5 text-xs">
            <span
              className={`h-1.5 w-1.5 shrink-0 rounded-full ${STATE_DOT[torrent.state]}`}
              aria-hidden="true"
            />
            <span>{STATE_LABEL[torrent.state]}</span>
            <span className="text-faint">·</span>
            <span className="font-mono tabular-nums">
              {formatBytes(torrent.progressBytes)} of{" "}
              {formatBytes(torrent.totalBytes)}
            </span>
            <span className="text-faint">·</span>
            <span className="font-mono tabular-nums">{percent}%</span>
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-0.5">
          <IconButton
            icon={isPaused ? "play" : "pause"}
            label={isPaused ? "Resume" : "Pause"}
            onClick={() => onToggle(torrent)}
          />
          <IconButton
            icon="files"
            label="Files and details"
            onClick={() => onOpenDetail(torrent)}
          />
          <IconButton
            icon="folder"
            label="Open containing folder"
            onClick={() => onReveal(torrent)}
          />
          <IconButton
            icon="trash"
            label="Remove"
            destructive
            onClick={() => onRemove(torrent)}
          />
        </div>
      </div>

      <div className="mt-2.5">
        <ProgressBar
          value={fraction}
          state={torrent.state}
          label={`${torrent.name} download progress`}
        />
      </div>

      <dl className="mt-2.5 flex flex-wrap gap-x-5 gap-y-1 text-xs">
        <Stat label="Down" value={formatSpeed(torrent.downloadBps)} />
        <Stat label="Up" value={formatSpeed(torrent.uploadBps)} />
        <Stat label="Peers" value={torrent.livePeers} />
        {torrent.etaSeconds !== null ? (
          <Stat label="ETA" value={formatDuration(torrent.etaSeconds)} />
        ) : null}
        <Stat label="Ratio" value={ratio} />
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
