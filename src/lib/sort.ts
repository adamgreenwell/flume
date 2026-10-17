/**
 * Ordering for the torrent list.
 *
 * Pure, and separate from the list that renders it, because the properties
 * that matter here are testable and easy to get subtly wrong: rows update
 * every second, so an order that is not *total* reshuffles equal rows on
 * every tick and makes clicking a moving target.
 *
 * The key names are column names on purpose. #147 moves sorting out of the
 * toolbar and into the column headers, and a persisted sort keyed to three
 * chip labels would have to be migrated when it does. The one exception is
 * `activity`, which is a composite of two columns rather than a column —
 * noted rather than removed, because it is the default and it is the order
 * someone glancing at a running client actually wants.
 */

import type {
  SortBy,
  SortDirection,
  SortState,
  TorrentSummary,
} from "@/lib/ipc/types";
import { progressFraction, ratio } from "@/lib/torrent";

export type { SortBy, SortDirection, SortState };

/**
 * How each key is labelled, in the order the picker offers them.
 *
 * `activity` leads because it is the default.
 */
export const SORTS: ReadonlyArray<{ by: SortBy; name: string }> = [
  { by: "activity", name: "Activity" },
  { by: "name", name: "Name" },
  { by: "size", name: "Size" },
  { by: "progress", name: "Progress" },
  { by: "downloadBps", name: "Download speed" },
  { by: "uploadBps", name: "Upload speed" },
  { by: "ratio", name: "Ratio" },
  { by: "eta", name: "Time left" },
  { by: "added", name: "Date added" },
  { by: "state", name: "State" },
];

/**
 * States in the order `asc` puts them.
 *
 * A categorical field has no natural direction, so this is a decision rather
 * than a fact: active first, then waiting, then finished, then stopped, then
 * broken. Errors sort last ascending and first descending, which is the one
 * part worth arguing about — they are the rows a user most wants to see.
 * They stay here because the rail already has a "Needs attention" view that
 * answers that question better than a sort can, and because a sort whose
 * ascending order is not the reverse of its descending order is worse than
 * either arrangement.
 */
const STATE_ORDER: Record<TorrentSummary["state"], number> = {
  downloading: 0,
  checking: 1,
  queued: 2,
  seeding: 3,
  paused: 4,
  error: 5,
};

/**
 * The direction a key is most useful in when it is first chosen.
 *
 * Picking "Size" and getting the smallest torrent first is a small, constant
 * annoyance. Numbers start largest-first; names start A to Z.
 */
export function defaultDirectionFor(by: SortBy): SortDirection {
  return by === "name" || by === "eta" || by === "state" ? "asc" : "desc";
}

/**
 * The value a key sorts on: a number, or `null` where there is none.
 *
 * `null` is not zero, and the difference decides where the row lands. A
 * torrent with no estimable time left has not finished instantly.
 */
function valueOf(t: TorrentSummary, by: SortBy): number | null {
  switch (by) {
    case "activity":
      return t.downloadBps + t.uploadBps;
    case "name":
      return null; // compared as text, below
    case "size":
      return t.totalBytes;
    case "progress":
      // The displayed fraction, not a second opinion on it. Sorting by a
      // fraction rather than by bytes is deliberate -- a nearly-finished
      // small torrent belongs above a barely-started large one, and bytes
      // downloaded is what the Size column is for.
      return progressFraction(t);
    case "downloadBps":
      return t.downloadBps;
    case "uploadBps":
      return t.uploadBps;
    case "ratio":
      return ratio(t);
    case "eta":
      return t.etaSeconds;
    case "added":
      return t.addedAt;
    case "state":
      return STATE_ORDER[t.state];
  }
}

/**
 * Orders `torrents` without mutating them.
 *
 * Three properties hold whatever the key:
 *
 * - **Total.** Every comparison ends on the session id, so no two rows ever
 *   compare equal. Rows update every second and the list re-sorts on each
 *   tick; without a final tie-break, equal rows swap places continuously.
 *   The engine already returns torrents id-ordered, which would make a stable
 *   sort enough — but relying on that makes this correct only by someone
 *   else's promise.
 * - **Nulls last, in both directions.** Reversing the direction must not
 *   march the rows that have no value to the top. "No estimate" is not
 *   "finishes soonest", and a user who flips the arrow is asking about the
 *   rows that have a value.
 * - **Direction applies to the key only.** The tie-breaks are always
 *   ascending, so flipping the arrow does not reorder rows that tie.
 *
 * @param torrents - The rows to order; not mutated.
 * @param sort - Which key, and which way.
 * @returns A new array in the requested order.
 */
export function sortTorrents(
  torrents: readonly TorrentSummary[],
  sort: SortState,
): TorrentSummary[] {
  const sign = sort.direction === "asc" ? 1 : -1;

  return [...torrents].sort((a, b) => {
    if (sort.by === "name") {
      const byName = a.name.localeCompare(b.name);
      if (byName !== 0) return sign * byName;
      return a.id - b.id;
    }

    const left = valueOf(a, sort.by);
    const right = valueOf(b, sort.by);

    // Before the direction is applied, so it cannot be reversed by it.
    if (left === null && right !== null) return 1;
    if (left !== null && right === null) return -1;

    if (left !== null && right !== null && left !== right) {
      return sign * (left < right ? -1 : 1);
    }

    // Equal on the key. `activity` keeps its name tie-break: an idle list
    // has every row at zero, and alphabetical reads better there than
    // arrival order does.
    if (sort.by === "activity") {
      const byName = a.name.localeCompare(b.name);
      if (byName !== 0) return byName;
    }
    return a.id - b.id;
  });
}
