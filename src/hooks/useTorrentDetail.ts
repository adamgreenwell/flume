"use client";

import { useEffect, useState } from "react";

import { getTorrentDetail } from "@/lib/ipc/client";
import { isCommandError, type TorrentDetail } from "@/lib/ipc/types";

/** How often an open panel refreshes, matching the telemetry tick. */
const REFRESH_MS = 1000;

/** What {@link useTorrentDetail} returns. */
export interface UseTorrentDetailResult {
  /** The latest detail, or `null` before the first response. */
  detail: TorrentDetail | null;
  /** Human-readable failure, or `null`. */
  error: string | null;
}

/**
 * Polls one torrent's detail while its row is open.
 *
 * Polled rather than pushed, unlike telemetry. Detail is per-torrent and
 * several times the size of a summary; broadcasting it for every torrent every
 * second so that one expanded row can read it would be the opposite trade. At
 * most one row is open at a time, so this is one extra call per second in
 * total.
 *
 * Stops entirely when `id` is `null`, so a collapsed list makes no IPC calls
 * beyond the telemetry stream.
 *
 * Results carry the id they were fetched for, so a stale response for a row
 * the user has already closed is discarded rather than rendered.
 *
 * @param id - The torrent to watch, or `null` for none.
 * @returns The latest {@link UseTorrentDetailResult}.
 */
export function useTorrentDetail(id: number | null): UseTorrentDetailResult {
  // The id the stored result belongs to is kept alongside it, so switching
  // rows is handled by *deriving* a null rather than by clearing state inside
  // an effect. Clearing it there would set state during render's commit and
  // cascade an extra render every time a row opened.
  const [state, setState] = useState<{
    id: number;
    detail: TorrentDetail | null;
    error: string | null;
  } | null>(null);

  useEffect(() => {
    if (id === null) return;

    let active = true;

    const poll = async () => {
      try {
        const next = await getTorrentDetail(id);
        if (!active) return;
        setState({ id, detail: next, error: null });
      } catch (caught: unknown) {
        if (!active) return;
        setState({
          id,
          detail: null,
          error: isCommandError(caught)
            ? caught.message
            : "Could not read this torrent's detail.",
        });
      }
    };

    void poll();
    const timer = setInterval(() => void poll(), REFRESH_MS);

    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [id]);

  // A result for a different torrent is not this torrent's result. Without
  // this the panel would show the previous row's pieces under the new row's
  // name for one tick.
  if (id === null || state === null || state.id !== id) {
    return { detail: null, error: null };
  }
  return { detail: state.detail, error: state.error };
}
