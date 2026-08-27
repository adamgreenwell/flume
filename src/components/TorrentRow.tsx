"use client";

import { formatBytes, formatSpeed } from "@/lib/format";
import type { TorrentState, TorrentSummary } from "@/lib/ipc/types";

import { HealthChip } from "./HealthChip";
import { Icon, type IconName } from "./Icon";

/**
 * Glyph per lifecycle state, drawn inside the status ring.
 *
 * The icon is the state. The row's meta line never repeats it in words — that
 * line is the only place a row can say something the user does not already
 * know, so spending it on "Downloading" wastes it.
 */
const STATE_ICON: Record<TorrentState, IconName> = {
  downloading: "arrow-down",
  seeding: "arrow-up",
  paused: "pause",
  checking: "check",
  error: "alert-circle",
};

/** Ring and glyph colour per state. */
const STATE_TONE: Record<TorrentState, string> = {
  downloading: "text-acc",
  seeding: "text-ok",
  paused: "text-fg-3",
  checking: "text-warn",
  error: "text-err",
};

/** Progress fill per state. */
const FILL_BY_STATE: Record<TorrentState, string> = {
  checking: "bg-warn",
  downloading: "bg-acc",
  seeding: "bg-ok",
  paused: "bg-fg-3",
  error: "bg-err",
};

/** Accessible wording for each state, since the glyph itself is hidden. */
const STATE_LABEL: Record<TorrentState, string> = {
  downloading: "Downloading",
  seeding: "Seeding",
  paused: "Paused",
  checking: "Checking",
  error: "Error",
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

/**
 * A rate cell.
 *
 * A zero rate drops to `fg-3` rather than being blanked. An empty cell reads
 * as "no data"; a dimmed zero reads as "measured, and it is zero", which is a
 * different and more useful fact when a torrent has stalled.
 */
function Rate({ bps, tone }: { bps: number; tone: string }) {
  return (
    <span
      role="gridcell"
      className={`flume-num w-[86px] shrink-0 text-right text-xs ${
        bps === 0 ? "text-fg-3" : tone
      }`}
    >
      {formatSpeed(bps)}
    </span>
  );
}

/** Props for {@link TorrentRow}. */
export interface TorrentRowProps {
  /** The torrent to render. */
  torrent: TorrentSummary;
  /** Whether this row is the selected one. */
  selected: boolean;
  /** Select or deselect this row. */
  onSelect: (t: TorrentSummary) => void;
  /**
   * Open the inspector for this torrent.
   *
   * Bound to double-click and to Enter-with-modifier. The design puts a row's
   * actions in the region that opens when it is clicked, which does not exist
   * yet — until it does, this and the context menu are how a torrent is
   * reached, and a list you can only act on by right-clicking is a list half
   * the users cannot act on at all.
   */
  onOpen: (t: TorrentSummary) => void;
  /** Right-clicked, with the pointer position in viewport coordinates. */
  onContextMenu: (t: TorrentSummary, at: { x: number; y: number }) => void;
}

/**
 * One torrent in the library list.
 *
 * Seven columns at fixed widths so the numbers line up down the list rather
 * than wandering with content — the reason every figure here is tabular mono.
 *
 * Rendered as a `row` in a real grid rather than a list item: the columns carry
 * meaning, and a screen reader user should be able to move across them and hear
 * the header for each rather than one run-on sentence.
 *
 * @param props - See {@link TorrentRowProps}.
 * @returns The rendered row.
 */
export function TorrentRow({
  torrent,
  selected,
  onSelect,
  onOpen,
  onContextMenu,
}: TorrentRowProps) {
  const fraction = progressFraction(torrent);
  const percent = Math.round(fraction * 100);

  return (
    <div
      role="row"
      aria-selected={selected}
      tabIndex={0}
      onClick={() => onSelect(torrent)}
      onDoubleClick={() => onOpen(torrent)}
      onKeyDown={(event) => {
        // A row is an interactive control, so it answers to the keys a control
        // answers to. The static mockups only imply this.
        if (event.key === "Enter") {
          event.preventDefault();
          if (event.metaKey || event.ctrlKey) onOpen(torrent);
          else onSelect(torrent);
        } else if (event.key === " ") {
          event.preventDefault();
          onSelect(torrent);
        }
      }}
      onContextMenu={(event) => {
        event.preventDefault();
        onContextMenu(torrent, { x: event.clientX, y: event.clientY });
      }}
      className={`border-line flex h-[var(--flume-row-h)] shrink-0 cursor-pointer items-center gap-4 border-b px-[18px] transition-colors ${
        selected
          ? "bg-bg-2 shadow-[inset_2px_0_0_var(--flume-acc)]"
          : "hover:bg-bg-1"
      }`}
    >
      <span
        role="gridcell"
        className={`relative flex h-[18px] w-[18px] shrink-0 items-center justify-center ${STATE_TONE[torrent.state]}`}
      >
        <span
          className="absolute inset-0 rounded-full border border-current opacity-[0.36]"
          aria-hidden="true"
        />
        <Icon name={STATE_ICON[torrent.state]} size={11} />
        <span className="sr-only">{STATE_LABEL[torrent.state]}</span>
      </span>

      <span role="gridcell" className="flex min-w-0 grow flex-col gap-0.5">
        <span
          className="truncate text-[13px] font-medium tracking-[-0.005em]"
          title={torrent.name}
        >
          {torrent.name}
        </span>
        <span className="text-fg-2 flex h-[var(--flume-meta-h)] items-center gap-[7px] overflow-hidden text-[11px] opacity-[var(--flume-meta-op)]">
          <span className="flume-num">{formatBytes(torrent.totalBytes)}</span>
          <span className="text-fg-3">·</span>
          <span className="truncate">{torrent.detail}</span>
        </span>
      </span>

      <span
        role="gridcell"
        className="flex w-[180px] shrink-0 items-center gap-[9px]"
      >
        <span
          className="bg-bg-3 flex h-[5px] grow overflow-hidden rounded-[3px]"
          role="progressbar"
          aria-label={`${torrent.name} progress`}
          aria-valuenow={percent}
          aria-valuemin={0}
          aria-valuemax={100}
        >
          <span
            className={`block h-full rounded-[3px] ${FILL_BY_STATE[torrent.state]}`}
            style={{ width: `${percent}%` }}
          />
        </span>
        <span
          className="flume-num text-fg-1 w-[34px] text-right text-[11px]"
          aria-hidden="true"
        >
          {percent}%
        </span>
      </span>

      <Rate bps={torrent.downloadBps} tone="text-fg-0" />
      <Rate bps={torrent.uploadBps} tone="text-fg-1" />

      <span
        role="gridcell"
        className="flume-num text-fg-2 w-[78px] shrink-0 text-right text-[11.5px]"
      >
        {torrent.knownPeers === 0
          ? "—"
          : `${torrent.livePeers} / ${torrent.knownPeers}`}
      </span>

      <span role="gridcell" className="w-[124px] shrink-0 pl-[14px]">
        <HealthChip health={torrent.health} detail={torrent.detail} />
      </span>
    </div>
  );
}
