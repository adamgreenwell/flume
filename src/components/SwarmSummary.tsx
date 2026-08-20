"use client";

import type { SwarmStats } from "@/lib/ipc/types";

/** Props for {@link SwarmSummary}. */
export interface SwarmSummaryProps {
  /** Peer pool counts for the torrent. */
  swarm: SwarmStats;
}

/** One labelled figure. */
function Figure({
  label,
  value,
  hint,
  tone = "text-text",
}: {
  label: string;
  value: number;
  hint: string;
  tone?: string;
}) {
  return (
    <div className="flex flex-col gap-0.5" title={hint}>
      <dt className="text-faint text-[11px] tracking-wide uppercase">
        {label}
      </dt>
      <dd className={`font-mono text-sm tabular-nums ${tone}`}>
        {value.toLocaleString()}
      </dd>
    </div>
  );
}

/**
 * Peer pool health for one torrent.
 *
 * These are *pool* counts, not seeds versus leechers. librqbit v9 knows
 * whether a peer holds the whole torrent but does not expose it through the
 * public per-peer snapshot, so that split is unavailable — and the footnote
 * says so rather than letting the omission look like an oversight.
 *
 * @param props - See {@link SwarmSummaryProps}.
 * @returns The rendered summary.
 */
export function SwarmSummary({ swarm }: SwarmSummaryProps) {
  return (
    <div className="border-border-subtle bg-bg rounded-md border p-3">
      <dl className="grid grid-cols-5 gap-3">
        <Figure
          label="Live"
          value={swarm.live}
          hint="Peers with an established connection right now"
          tone="text-accent"
        />
        <Figure
          label="Connecting"
          value={swarm.connecting}
          hint="Peers currently being connected to"
        />
        <Figure
          label="Queued"
          value={swarm.queued}
          hint="Known peers waiting for a connection slot"
        />
        <Figure
          label="Seen"
          value={swarm.seen}
          hint="Distinct peers discovered for this torrent, ever"
        />
        <Figure
          label="Dead"
          value={swarm.dead}
          hint="Peers that failed and were dropped"
          tone="text-faint"
        />
      </dl>
      <p className="text-faint mt-2.5 text-[11px]">
        {swarm.liveTcp} over TCP · {swarm.liveUtp} over uTP. The torrent engine
        does not report which peers are seeds, so there is no seeds/leechers
        split.
      </p>
    </div>
  );
}
