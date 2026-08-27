"use client";

import { formatBytes, formatDuration, formatSpeed } from "@/lib/format";

/** How much of the bar a tile's incoming segment may claim. */
const MAX_INCOMING = 42;

/** One tile in the pre-flight strip. */
function Tile({
  label,
  value,
  bar,
  note,
  noteTone = "text-fg-3",
}: {
  label: string;
  value: React.ReactNode;
  bar: React.ReactNode;
  note: string;
  noteTone?: string;
}) {
  return (
    <div className="border-line flex flex-col gap-[7px] border-r px-5 py-3.5 last:border-r-0">
      <span className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
        {label}
      </span>
      <span className="flume-num text-[19px] leading-[1.15] font-medium tracking-[-0.025em]">
        {value}
      </span>
      <span className="bg-bg-3 flex h-1 overflow-hidden rounded-sm">{bar}</span>
      <span className={`text-[10.5px] ${noteTone}`}>{note}</span>
    </div>
  );
}

/** A smaller unit suffix beside a tile's number. */
function Unit({ children }: { children: React.ReactNode }) {
  return <span className="text-fg-2 text-xs font-normal">{children}</span>;
}

/** Props for {@link PreflightTiles}. */
export interface PreflightTilesProps {
  /** Peers that answered while the metadata was fetched. */
  seenPeers: number;
  /** Bytes the user has selected. */
  selectedBytes: number;
  /** Bytes in the whole torrent. */
  totalBytes: number;
  /** How many files are selected. */
  selectedCount: number;
  /** How many files the torrent has. */
  totalCount: number;
  /** Free space on the save volume, or `null` when unknown. */
  freeBytes: number | null;
  /** Recent download rate to estimate against, or `null` when unmeasured. */
  rateBps: number | null;
}

/**
 * The four facts worth knowing before agreeing to a download.
 *
 * Three recalculate as files are toggled. The swarm tile does not — it reports
 * what was measured while the metadata was fetched, and re-measuring it on
 * every checkbox click would be both wrong and pointless.
 *
 * Every tile states its own basis in the line underneath. A number whose
 * derivation is invisible is a number the user has to take on trust, and the
 * whole point of a review sheet is that nothing needs to be taken on trust.
 *
 * @param props - See {@link PreflightTilesProps}.
 * @returns The rendered strip.
 */
export function PreflightTiles({
  seenPeers,
  selectedBytes,
  totalBytes,
  selectedCount,
  totalCount,
  freeBytes,
  rateBps,
}: PreflightTilesProps) {
  const share = totalBytes === 0 ? 0 : (selectedBytes / totalBytes) * 100;

  // "Nothing skipped" is a claim about files, so it is decided by counting
  // them. Deciding it by comparing byte sums makes the tile contradict itself
  // — "11 of 11 files, 20 MB skipped" — the moment the reported total and the
  // sum of the file lengths disagree by a rounding step.
  const skippedCount = totalCount - selectedCount;
  const skippedBytes = Math.max(totalBytes - selectedBytes, 0);

  const after = freeBytes === null ? null : freeBytes - selectedBytes;
  const overFull = after !== null && after < 0;
  // A quarter of what was free, floored at 20 GB. Below that a download is
  // technically fine and practically a problem the user should see coming.
  const tight =
    freeBytes !== null &&
    after !== null &&
    !overFull &&
    after < Math.max(freeBytes * 0.25, 20_000_000_000);

  const seconds =
    rateBps === null || rateBps === 0 || selectedBytes === 0
      ? null
      : selectedBytes / rateBps;

  return (
    <div className="border-line bg-bg-0 grid shrink-0 grid-cols-4 border-b">
      <Tile
        label="Swarm when checked"
        value={
          <>
            {seenPeers} <Unit>{seenPeers === 1 ? "peer" : "peers"}</Unit>
          </>
        }
        bar={
          <span
            className="bg-acc-dim block h-full"
            style={{ width: seenPeers === 0 ? "0%" : "100%" }}
          />
        }
        note={
          seenPeers === 0
            ? "Metadata came from a file, not from peers."
            : "Answered while the file list was fetched."
        }
      />

      <Tile
        label="You are downloading"
        value={formatBytes(selectedBytes)}
        bar={
          <span
            className="bg-acc block h-full"
            style={{ width: `${share}%` }}
          />
        }
        note={`${selectedCount} of ${totalCount} files · ${
          skippedCount > 0
            ? `${formatBytes(skippedBytes)} skipped`
            : "nothing skipped"
        }`}
      />

      <Tile
        label="Volume after"
        value={after === null ? "Unknown" : formatBytes(Math.max(after, 0))}
        bar={
          freeBytes === null || freeBytes === 0 ? (
            <span className="bg-bg-3 block h-full w-full" />
          ) : (
            <span
              className={`block h-full ${overFull ? "bg-err" : "bg-acc"}`}
              style={{
                width: `${Math.min(MAX_INCOMING, (selectedBytes / freeBytes) * 100)}%`,
              }}
            />
          )
        }
        note={
          after === null
            ? "This volume did not report its free space."
            : overFull
              ? `${formatBytes(-after)} short — this will not fit.`
              : tight
                ? "Leaves the volume tight."
                : `Down from ${formatBytes(freeBytes ?? 0)}.`
        }
        noteTone={overFull ? "text-err" : tight ? "text-warn" : "text-fg-3"}
      />

      <Tile
        label="Estimated finish"
        value={seconds === null ? "—" : formatDuration(seconds)}
        bar={
          <span
            className="bg-acc-dim block h-full"
            style={{ width: seconds === null ? "0%" : "100%" }}
          />
        }
        note={
          rateBps === null || rateBps === 0
            ? "No recent transfer to estimate from."
            : `At this session's ${formatSpeed(rateBps)} average.`
        }
      />
    </div>
  );
}
