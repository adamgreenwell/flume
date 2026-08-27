import type { IconName } from "@/components/Icon";
import type { TorrentSummary } from "@/lib/ipc/types";

/** The sidebar's filters. */
export type ViewId =
  "all" | "downloading" | "seeding" | "attention" | "completed" | "paused";

/** One entry in the sidebar. */
export interface ViewDef {
  id: ViewId;
  name: string;
  icon: IconName;
}

/**
 * The views, in the order the rail lists them.
 *
 * "Needs attention" is computed, not a tag the user applies. A torrent the user
 * had to remember to label would be a torrent nobody labels.
 */
export const VIEWS: readonly ViewDef[] = [
  { id: "all", name: "All torrents", icon: "files" },
  { id: "downloading", name: "Downloading", icon: "arrow-down" },
  { id: "seeding", name: "Seeding", icon: "arrow-up" },
  { id: "attention", name: "Needs attention", icon: "alert-triangle" },
  { id: "completed", name: "Completed", icon: "check-circle" },
  { id: "paused", name: "Paused & queued", icon: "pause" },
];

/**
 * Whether a torrent belongs in a view.
 *
 * `attention` is the only interesting one: it means "this will not finish
 * unless you do something", which is a narrower claim than "not currently
 * downloading". A paused torrent is fine. A torrent with nobody to talk to is
 * not, and neither is one that stopped on a failure.
 *
 * Torrents whose health is `unknown` are deliberately excluded — Flume cannot
 * yet tell a thin swarm from a healthy one, and sweeping every active download
 * into "needs attention" would make the view useless.
 *
 * @param t - The torrent to test.
 * @param view - The view to test against.
 * @returns Whether the torrent should appear.
 */
export function matchesView(t: TorrentSummary, view: ViewId): boolean {
  switch (view) {
    case "all":
      return true;
    case "downloading":
      return t.state === "downloading" || t.state === "checking";
    case "seeding":
      return t.state === "seeding";
    case "attention":
      return t.state === "error" || t.health === "none";
    case "completed":
      return t.finished;
    case "paused":
      return t.state === "paused";
  }
}

/**
 * Whether a torrent matches a search query.
 *
 * Case-insensitive substring over the name. An empty query matches everything
 * rather than nothing, so clearing the field restores the list.
 *
 * @param t - The torrent to test.
 * @param query - The raw query, untrimmed.
 * @returns Whether the torrent should appear.
 */
export function matchesQuery(t: TorrentSummary, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (needle === "") return true;
  return t.name.toLowerCase().includes(needle);
}

/**
 * Counts per view, for the rail's badges.
 *
 * Counted before the search filter is applied: the badge answers "how many are
 * there", and a count that shrank as you typed would be answering a question
 * nobody asked.
 *
 * @param torrents - Every torrent in the session.
 * @returns A count per view id.
 */
export function viewCounts(
  torrents: readonly TorrentSummary[],
): Record<ViewId, number> {
  const counts = {} as Record<ViewId, number>;
  for (const view of VIEWS) {
    counts[view.id] = torrents.filter((t) => matchesView(t, view.id)).length;
  }
  return counts;
}
