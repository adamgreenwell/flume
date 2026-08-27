"use client";

import { formatBytes, formatDuration } from "@/lib/format";
import type { CoreStatus, TorrentSummary } from "@/lib/ipc/types";

import { StatCard } from "./StatCard";

/** Props for {@link Dock}. */
export interface DockProps {
  /** Session status, or `null` before the engine answers. */
  status: CoreStatus | null;
  /** Every torrent in the session, not just the filtered view. */
  torrents: readonly TorrentSummary[];
}

/**
 * The 116px footer: what the session as a whole is doing.
 *
 * Aggregates over every torrent rather than the filtered view. These numbers
 * answer "what is this machine doing right now", which does not change because
 * the user typed in the search box.
 *
 * The throughput chart that belongs on the left of this dock is not here yet;
 * it needs a rolling history the telemetry hook does not keep.
 *
 * @param props - See {@link DockProps}.
 * @returns The rendered dock.
 */
export function Dock({ status, torrents }: DockProps) {
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
    <div className="bg-bg-1 border-line flex h-[116px] shrink-0 items-stretch border-t">
      <div className="grid grow grid-cols-4 content-center gap-x-[18px] gap-y-2.5 px-[22px] py-3.5">
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
