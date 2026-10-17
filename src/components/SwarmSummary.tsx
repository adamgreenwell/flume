"use client";

import type { SwarmStats } from "@/lib/ipc/types";

/** Props for {@link SwarmSummary}. */
export interface SwarmSummaryProps {
  /** Peer pool counts for the torrent. */
  swarm: SwarmStats;
}

/** One labelled figure, optionally shown against a ceiling. */
function Figure({
  label,
  value,
  of,
  hint,
  tone = "text-fg-0",
}: {
  label: string;
  value: number;
  /** The ceiling this figure counts towards, drawn as `3 / 128`. */
  of?: number;
  hint: string;
  tone?: string;
}) {
  return (
    <div className="flex flex-col gap-0.5" title={hint}>
      <dt className="text-fg-3 text-[11px] tracking-wide uppercase">{label}</dt>
      <dd className={`font-mono text-sm tabular-nums ${tone}`}>
        {value.toLocaleString()}
        {of === undefined ? null : (
          <span className="text-fg-3"> / {of.toLocaleString()}</span>
        )}
      </dd>
    </div>
  );
}

/**
 * Peer pool health for one torrent.
 *
 * The first five are *pool* counts — how many peers, in what connection state.
 * Seeds and availability are a different question, derived from the peers'
 * bitfields: what the swarm actually holds.
 *
 * Availability is shown beside the rarest-piece count, never instead of it. A
 * mean of 4.0 reads reassuring and can still hide a piece nobody has, which is
 * the one case that stops a torrent finishing.
 *
 * @param props - See {@link SwarmSummaryProps}.
 * @returns The rendered summary.
 */
export function SwarmSummary({ swarm }: SwarmSummaryProps) {
  return (
    <div className="border-line bg-bg-0 rounded-md border p-3">
      <dl className="grid grid-cols-5 gap-3">
        <Figure
          label="Live"
          value={swarm.live}
          of={swarm.peerLimit}
          hint={`Peers with an established connection right now, against the ${swarm.peerLimit}-peer limit for this torrent`}
          tone="text-acc"
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
          tone="text-fg-3"
        />
      </dl>
      {swarm.seeds === null || swarm.availability === null ? null : (
        <dl className="border-line mt-3 grid grid-cols-5 gap-3 border-t pt-3">
          <Figure
            label="Seeds"
            value={swarm.seeds}
            hint="Connected peers holding every piece"
          />
          <div
            className="col-span-2 flex flex-col gap-0.5"
            title="Mean copies of each piece across the connected peers"
          >
            <dt className="text-fg-3 text-[11px] tracking-wide uppercase">
              Availability
            </dt>
            <dd className="font-mono text-sm tabular-nums">
              {swarm.availability.toFixed(2)}×
            </dd>
          </div>
        </dl>
      )}
      {swarm.rarest === null ? (
        <p className="text-fg-3 mt-2.5 text-[11px]">
          {swarm.liveTcp} over TCP · {swarm.liveUtp} over uTP. No peer bitfields
          yet, so what the swarm holds is not known.
        </p>
      ) : (
        <p className="text-fg-3 mt-2.5 text-[11px]">
          {swarm.liveTcp} over TCP · {swarm.liveUtp} over uTP ·{" "}
          <span className={swarm.rarest === 0 ? "text-err" : undefined}>
            {swarm.rarest === 0
              ? "no peer holds every piece — this cannot finish as it stands"
              : `rarest piece on ${swarm.rarest} ${swarm.rarest === 1 ? "peer" : "peers"}`}
          </span>
          .
        </p>
      )}
      {swarm.live >= swarm.peerLimit ? (
        <p className="text-fg-3 mt-1 text-[11px]">
          At the {swarm.peerLimit}-peer limit for this torrent. Whether that is
          holding the transfer back depends on what the peers it is not
          connecting to would have supplied, which is not knowable — raising it
          in Settings takes effect after a restart.
        </p>
      ) : null}
    </div>
  );
}
