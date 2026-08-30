"use client";

import { formatBytes } from "@/lib/format";
import type { PeerInfo, TorrentDetail, TorrentSummary } from "@/lib/ipc/types";

import { Chip } from "./Chip";
import { Icon } from "./Icon";
import { NoteCard } from "./NoteCard";
import { PieceStrip } from "./PieceStrip";
import { Skeleton } from "./Skeleton";

/** How many contributors the row lists. */
const TOP_CONTRIBUTORS = 4;

/** One statistic in the expanded head. */
function Stat({
  label,
  value,
  mono = true,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    // Figures never shrink; the one prose stat does. Without this the strip is
    // a row of flex children that all give way equally, and "11,863 of 23,280"
    // breaks at its spaces into three stacked lines — unreadable, and taller
    // than the row it sits in. A truncated path is recoverable from the
    // tooltip; a wrapped number is just wrong.
    <div className={`flex flex-col gap-[3px] ${mono ? "shrink-0" : "min-w-0"}`}>
      <span className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
        {label}
      </span>
      <span
        className={`font-medium tracking-[-0.01em] ${
          mono
            ? "flume-num text-[15px] whitespace-nowrap"
            : "truncate text-[12.5px]"
        }`}
        title={mono ? undefined : value}
      >
        {value}
      </span>
    </div>
  );
}

/**
 * The peers that have actually given you something, busiest first.
 *
 * Ranked by bytes that passed verification, not by connection order or
 * announced rate. A peer can be connected, chatty and useless; this is the
 * list of the ones that are not.
 *
 * The design shows an instantaneous rate per peer. librqbit's per-peer
 * counters are cumulative totals, with no rate among them, so this shows the
 * total each peer has actually supplied — which is arguably the better answer
 * to "who is contributing" anyway, since it does not swing with the tick.
 */
function Contributors({ peers }: { peers: readonly PeerInfo[] }) {
  const ranked = [...peers]
    .filter((p) => p.downloadedBytes > 0)
    .sort((a, b) => b.downloadedBytes - a.downloadedBytes)
    .slice(0, TOP_CONTRIBUTORS);

  if (ranked.length === 0) {
    return (
      <div className="flex min-w-[340px] flex-col gap-1.5">
        <span className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
          Top contributors
        </span>
        <p className="text-fg-2 text-[11.5px]">
          {peers.length === 0
            ? "No peers connected."
            : peers.length === 1
              ? "1 peer connected, but it has not sent a verified piece yet."
              : `${peers.length} peers connected, none has sent a verified piece yet.`}
        </p>
      </div>
    );
  }

  const best = ranked[0].downloadedBytes;

  return (
    <div className="flex min-w-[340px] flex-col gap-1.5">
      <span className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
        Top contributors
      </span>
      {ranked.map((peer) => (
        <div
          key={peer.address}
          className="text-fg-1 flex items-center gap-2.5 text-[11.5px]"
        >
          <span className="flume-num w-[132px] truncate" title={peer.address}>
            {peer.address}
          </span>
          <span className="text-fg-2 grow truncate">
            {peer.client ?? "unidentified client"}
          </span>
          <span className="bg-bg-3 flex h-1 w-[70px] shrink-0 overflow-hidden rounded-sm">
            <span
              className={
                peer.downloadedBytes / best > 0.6 ? "bg-acc" : "bg-acc-dim"
              }
              style={{ width: `${(peer.downloadedBytes / best) * 100}%` }}
            />
          </span>
          <span className="flume-num w-[62px] shrink-0 text-right">
            {formatBytes(peer.downloadedBytes)}
          </span>
        </div>
      ))}
    </div>
  );
}

/** Props for {@link ExpandedRow}. */
export interface ExpandedRowProps {
  /** The torrent this panel belongs to. */
  torrent: TorrentSummary;
  /** Its detail, or `null` while the first response is outstanding. */
  detail: TorrentDetail | null;
  /** A failure reading the detail, or `null`. */
  error: string | null;
  /** Pause or resume. */
  onToggle: (t: TorrentSummary) => void;
  /** Reveal the download in the OS file manager. */
  onReveal: (t: TorrentSummary) => void;
  /** Open the full inspector. */
  onOpen: (t: TorrentSummary) => void;
}

/**
 * The panel that opens under a clicked row.
 *
 * Indented to clear the row's status column above it, so the panel reads as
 * belonging to that row rather than floating between two.
 *
 * @param props - See {@link ExpandedRowProps}.
 * @returns The rendered panel.
 */
export function ExpandedRow({
  torrent,
  detail,
  error,
  onToggle,
  onReveal,
  onOpen,
}: ExpandedRowProps) {
  const remaining = Math.max(torrent.totalBytes - torrent.progressBytes, 0);
  const ratio =
    torrent.progressBytes === 0
      ? 0
      : torrent.uploadedBytes / torrent.progressBytes;

  return (
    <div className="border-line bg-bg-1 flex flex-col gap-3.5 border-b py-4 pr-[18px] pl-[52px]">
      <div className="flex items-start gap-7">
        <div className="flex gap-7">
          <Stat label="Downloaded" value={formatBytes(torrent.progressBytes)} />
          <Stat
            label="Remaining"
            value={torrent.finished ? "—" : formatBytes(remaining)}
          />
          <Stat label="Ratio" value={ratio.toFixed(2)} />
          <Stat
            label="Pieces"
            value={
              detail?.pieces
                ? `${detail.pieces.piecesComplete.toLocaleString()} of ${detail.pieces.totalPieces.toLocaleString()}`
                : "—"
            }
          />
          <Stat label="Saving to" value={torrent.outputFolder} mono={false} />
        </div>

        <div className="ml-auto flex shrink-0 gap-1.5">
          <Chip onClick={() => onToggle(torrent)}>
            {torrent.state === "paused" ? "Resume" : "Pause"}
          </Chip>
          <Chip onClick={() => onReveal(torrent)}>Reveal in folder</Chip>
          <Chip selected onClick={() => onOpen(torrent)}>
            Open details
            <Icon name="chevron-right" size={12} />
          </Chip>
        </div>
      </div>

      {error ? (
        <p className="text-err text-[11.5px]" role="alert">
          {error}
        </p>
      ) : null}

      {detail === null && error === null ? (
        <Skeleton label={`Loading detail for ${torrent.name}`} rows={2} />
      ) : null}

      {detail?.pieces ? (
        <PieceStrip pieces={detail.pieces} />
      ) : detail !== null ? (
        <p className="text-fg-3 text-[11.5px]">
          Piece detail appears once this torrent is running — a torrent that is
          still starting up or has errored has no piece state to read.
        </p>
      ) : null}

      {detail ? (
        <div className="flex flex-wrap gap-9">
          <Contributors peers={detail.peers} />
          <NoteCard note={detail.note} />
        </div>
      ) : null}
    </div>
  );
}
