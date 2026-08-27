"use client";

import type { CoreStatus } from "@/lib/ipc/types";

import { Button } from "./Button";

/** Props for {@link EmptyState}. */
export interface EmptyStateProps {
  /** Session status, used to tailor the guidance. May be `null` while starting. */
  status: CoreStatus | null;
  /** Opens the add-torrent dialog. */
  onAdd: () => void;
}

/**
 * Shown when there are no torrents.
 *
 * Doubles as the first-run onboarding: rather than a dismissible banner that
 * appears once and is gone forever, the guidance lives exactly where a new
 * user is already looking, and stays available until they have a torrent.
 *
 * It also reflects live session state — where files will land, and whether
 * peer discovery is ready yet — because "why is nothing happening?" is the
 * first question a new user has, and DHT bootstrap takes a few seconds.
 *
 * @param props - See {@link EmptyStateProps}.
 * @returns The rendered empty state.
 */
export function EmptyState({ status, onAdd }: EmptyStateProps) {
  const dhtReady = status?.health === "ready";

  return (
    <div className="border-line flex flex-1 flex-col items-center justify-center gap-4 rounded-xl border border-dashed px-6 py-16 text-center">
      <div className="max-w-sm">
        <h2 className="text-fg-0 text-sm font-medium">No torrents yet</h2>
        <p className="text-fg-2 mt-1.5 text-xs leading-relaxed">
          Paste a magnet link, choose a <code>.torrent</code> file, or drop one
          onto this window. Flume shows you the file list first, so you only
          download what you actually want.
        </p>
      </div>

      <Button variant="primary" onClick={onAdd}>
        Add your first torrent
      </Button>

      {status ? (
        <dl className="text-fg-3 mt-2 flex flex-col gap-1 text-[11px]">
          <div className="flex items-center justify-center gap-1.5">
            <dt>Saving to</dt>
            <dd
              className="text-fg-2 selectable font-mono"
              title={status.downloadDir}
            >
              {status.downloadDir}
            </dd>
          </div>
          <div className="flex items-center justify-center gap-1.5">
            <dt>Peer discovery</dt>
            <dd className={dhtReady ? "text-ok" : "text-warn"}>
              {dhtReady
                ? "ready"
                : "still connecting — magnet links need a moment"}
            </dd>
          </div>
        </dl>
      ) : null}
    </div>
  );
}
