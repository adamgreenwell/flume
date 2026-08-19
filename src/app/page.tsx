"use client";

import { StatCard } from "@/components/StatCard";
import { StatusPill } from "@/components/StatusPill";
import { useCoreStatus } from "@/hooks/useCoreStatus";
import { formatDuration, formatSpeed } from "@/lib/format";

/**
 * Phase 0 landing page.
 *
 * Its job is to prove the full IPC path: the Rust backend starts a real
 * librqbit session, and these numbers are live counters read out of it over
 * Tauri `invoke`. Phase 1 replaces this with the torrent list.
 *
 * @returns The rendered page.
 */
export default function Home() {
  const { status, error, isLoading } = useCoreStatus();

  return (
    <main className="mx-auto flex min-h-full w-full max-w-3xl flex-col gap-8 px-8 py-12">
      <header className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-text text-3xl font-semibold tracking-tight">
            Flume
          </h1>
          <p className="text-muted mt-1 text-sm">
            A beautiful, cross-platform BitTorrent client.
          </p>
        </div>
        <StatusPill
          health={status?.health ?? "starting"}
          pulse={isLoading || status?.health === "connecting"}
        />
      </header>

      {error ? (
        <div
          className="border-warn/30 bg-warn/10 text-warn rounded-lg border px-4 py-3 text-sm"
          role="alert"
        >
          {error}
        </div>
      ) : null}

      <section aria-label="Engine status">
        <h2 className="text-faint mb-3 text-[11px] font-medium tracking-wider uppercase">
          Torrent engine
        </h2>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
          <StatCard
            label="Download"
            value={formatSpeed(status?.downloadBps ?? 0)}
          />
          <StatCard
            label="Upload"
            value={formatSpeed(status?.uploadBps ?? 0)}
          />
          <StatCard label="Peers" value={status?.livePeers ?? 0} />
          <StatCard
            label="DHT nodes"
            value={
              status?.dht.enabled
                ? status.dht.nodesV4 + status.dht.nodesV6
                : "Off"
            }
            hint={
              status?.dht.enabled
                ? `${status.dht.nodesV4} IPv4 · ${status.dht.nodesV6} IPv6`
                : "Magnet links need DHT"
            }
          />
          <StatCard
            label="Listen port"
            value={status?.listenPort ?? "—"}
            hint={
              status?.announcePort
                ? `announcing ${status.announcePort}`
                : undefined
            }
          />
          <StatCard
            label="Uptime"
            value={formatDuration(status?.uptimeSeconds ?? 0)}
          />
        </div>
      </section>

      <section aria-label="Session details" className="mt-auto">
        <dl className="border-border-subtle bg-surface divide-border-subtle divide-y rounded-lg border text-sm">
          <div className="flex items-baseline justify-between gap-4 px-4 py-3">
            <dt className="text-muted shrink-0">Download folder</dt>
            <dd
              className="text-text selectable truncate font-mono text-xs"
              title={status?.downloadDir}
            >
              {status?.downloadDir ?? "—"}
            </dd>
          </div>
          <div className="flex items-baseline justify-between gap-4 px-4 py-3">
            <dt className="text-muted shrink-0">Engine</dt>
            <dd className="text-text selectable font-mono text-xs">
              {status?.clientVersion ?? "—"}
            </dd>
          </div>
        </dl>
        <p className="text-faint mt-3 text-xs">
          Phase 0 — these figures are live from librqbit over the Tauri IPC
          bridge. Torrent management arrives in Phase 1.
        </p>
      </section>
    </main>
  );
}
