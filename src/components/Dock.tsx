"use client";

import type { ThroughputSample } from "@/lib/chart";
import { formatBytes, formatDuration } from "@/lib/format";
import type { CoreStatus, TorrentSummary } from "@/lib/ipc/types";

import { StatCard } from "./StatCard";
import { ThroughputChart } from "./ThroughputChart";

/** Props for {@link Dock}. */
export interface DockProps {
  /** Session status, or `null` before the engine answers. */
  status: CoreStatus | null;
  /** Every torrent in the session, not just the filtered view. */
  torrents: readonly TorrentSummary[];
  /** The last minute of session throughput, oldest first. */
  history: readonly ThroughputSample[];
  /** Configured download limit in bytes/sec, or `null` for unlimited. */
  limitBps: number | null;
}

/**
 * The 116px footer: what the session as a whole is doing.
 *
 * Aggregates over every torrent rather than the filtered view. These numbers
 * answer "what is this machine doing right now", which does not change because
 * the user typed in the search box.
 *
 * The chart takes the left; the aggregate readout takes the rest. The chart is
 * what makes "it dropped about forty seconds ago" answerable, which is a
 * question the numbers beside it cannot answer at any size.
 *
 * @param props - See {@link DockProps}.
 * @returns The rendered dock.
 */
export function Dock({ status, torrents, history, limitBps }: DockProps) {
  const active = torrents.filter(
    (t) => t.state === "downloading" || t.state === "seeding",
  ).length;

  const uploaded = torrents.reduce((sum, t) => sum + t.uploadedBytes, 0);
  const downloaded = torrents.reduce((sum, t) => sum + t.progressBytes, 0);
  // Guarded rather than shown as infinity: a fresh session has uploaded
  // something and downloaded nothing often enough to matter.
  const ratio = downloaded === 0 ? 0 : uploaded / downloaded;

  const livePeers = torrents.reduce((sum, t) => sum + t.livePeers, 0);
  const knownPeers = torrents.reduce((sum, t) => sum + t.knownPeers, 0);

  return (
    <div className="bg-bg-1 border-line flex min-h-[116px] shrink-0 items-stretch border-t">
      {/*
        The chart is the first thing to go when the window is narrow. Below
        about 1024px the rail plus a readable stat grid already fills the row,
        and a chart squeezed into what is left stops being readable before the
        numbers do — the numbers are the part you cannot reconstruct by
        looking.
      */}
      <div className="hidden min-w-0 flex-1 lg:flex">
        <ThroughputChart history={history} limitBps={limitBps} />
      </div>

      {/*
        `auto-fit` with an 88px floor rather than a fixed four columns, so a
        narrow window drops to three or two rather than shrinking each stat
        past legibility.

        The basis is 460px because that is what four columns need: 4 × 88 plus
        three 18px gaps is 406, plus 44px of padding. At 420 only three fitted,
        which turned eight stats into three rows and pushed the last one out of
        the bottom — the fixed height had been sized for two.

        `min-h` on the dock rather than `h` is the backstop. Row count depends
        on width, so any fixed height is a guess that some window size will
        falsify; growing is always better than clipping.
      */}
      <div className="border-line grid grow grid-cols-[repeat(auto-fit,minmax(88px,1fr))] content-center gap-x-[18px] gap-y-2.5 border-l px-[22px] py-3.5 lg:grow-0 lg:basis-[460px]">
        <StatCard label="Active" value={`${active} of ${torrents.length}`} />
        <StatCard label="Session down" value={formatBytes(downloaded)} />
        <StatCard label="Session up" value={formatBytes(uploaded)} />
        <StatCard label="Share ratio" value={ratio.toFixed(2)} />
        <StatCard label="Connected peers" value={livePeers} />
        <StatCard label="Known peers" value={knownPeers} />
        <StatCard
          label="DHT nodes"
          value={
            status ? (status.dht.nodesV4 + status.dht.nodesV6).toString() : "—"
          }
        />
        <StatCard
          label="Uptime"
          value={status ? formatDuration(status.uptimeSeconds) : "—"}
        />
      </div>
    </div>
  );
}
