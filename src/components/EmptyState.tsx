"use client";

import type { CoreStatus, Note } from "@/lib/ipc/types";

import { Button } from "./Button";
import { Chip } from "./Chip";

/** Props for {@link EmptyState}. */
export interface EmptyStateProps {
  /** Session status, used to tailor the guidance. May be `null` while starting. */
  status: CoreStatus | null;
  /** Opens the add-torrent dialog. */
  onAdd: () => void;
  /**
   * Whether the library has torrents that the current view or search hid.
   *
   * An empty list means two entirely different things — "you have not added
   * anything" and "nothing here matches what you asked for" — and the onboarding
   * copy is actively wrong for the second. Telling a user with 12 torrents that
   * they have none reads as data loss.
   */
  filtered?: boolean;
  /**
   * Why the guard is holding transfer, if it is.
   *
   * Takes priority over every other branch. Without it, launching while held
   * shows the first-run onboarding screen — "No torrents yet", "Add your first
   * torrent" — to someone with a full library, which reads as data loss. It is
   * also the one branch whose primary action cannot be "add a torrent", since
   * adding is exactly what a stopped engine cannot do.
   */
  guardNote?: Note | null;
  /** Opens settings, the only action that can lift a hold from inside Flume. */
  onOpenSettings?: () => void;
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
export function EmptyState({
  status,
  onAdd,
  filtered = false,
  guardNote = null,
  onOpenSettings,
}: EmptyStateProps) {
  const dhtReady = status?.health === "ready";

  if (guardNote) {
    return (
      <div className="flex max-w-md flex-col items-center gap-3 px-6 py-16 text-center">
        <h2 className="text-fg-0 text-sm font-medium">{guardNote.title}</h2>
        <p className="text-fg-2 text-xs leading-relaxed">{guardNote.body}</p>
        <p className="text-fg-2 text-xs leading-relaxed">
          Your library is on disk exactly as you left it, and every torrent
          comes back in the state it was in.
        </p>
        {onOpenSettings ? (
          <Chip onClick={onOpenSettings}>Open network settings</Chip>
        ) : null}
      </div>
    );
  }

  if (filtered) {
    return (
      <div className="flex flex-col items-center gap-1.5 px-6 py-16 text-center">
        <h2 className="text-fg-0 text-sm font-medium">Nothing matches</h2>
        <p className="text-fg-2 max-w-sm text-xs leading-relaxed">
          No torrent in this view matches your search. Everything you have added
          is still here — clear the search or pick another view to see it.
        </p>
      </div>
    );
  }

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
