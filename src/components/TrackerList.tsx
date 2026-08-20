"use client";

/** Props for {@link TrackerList}. */
export interface TrackerListProps {
  /** Tracker announce URLs. */
  trackers: string[];
}

/**
 * Tracker announce URLs.
 *
 * URLs only, deliberately. librqbit v9 exposes the configured tracker list but
 * not per-tracker announce status, so there is no last-announce time, next
 * announce, or peer count available. Showing empty columns for those would
 * imply the data is merely missing rather than unavailable.
 *
 * @param props - See {@link TrackerListProps}.
 * @returns The rendered list.
 */
export function TrackerList({ trackers }: TrackerListProps) {
  if (trackers.length === 0) {
    return (
      <p className="text-faint py-6 text-center text-xs">
        No trackers. This torrent relies on the DHT to find peers.
      </p>
    );
  }

  return (
    <>
      <ul className="flex flex-col gap-1">
        {trackers.map((url) => (
          <li
            key={url}
            className="border-border-subtle bg-bg text-text selectable truncate rounded border px-2.5 py-1.5 font-mono text-xs"
            title={url}
          >
            {url}
          </li>
        ))}
      </ul>
      <p className="text-faint mt-2 text-xs">
        Per-tracker announce status is not available from the torrent engine.
      </p>
    </>
  );
}
