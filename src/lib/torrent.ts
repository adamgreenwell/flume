/**
 * Pure derivations over a {@link TorrentSummary}.
 *
 * Here rather than beside the component that happened to need them first,
 * because each of these is shown *and* sorted on, and a column that orders
 * rows by a different number than the one printed in them is indefensible.
 * That is not hypothetical: the Progress key was written against its own
 * fraction and disagreed with the bar beside it for a finished torrent whose
 * metadata had not resolved.
 */

import type { TorrentSummary } from "@/lib/ipc/types";

/**
 * Completion fraction, from 0 to 1.
 *
 * **Must stay in step with `TorrentSummary::progress_fraction` in
 * `src-tauri/src/engine/torrent.rs`.** Both treat a finished torrent as
 * complete even when `totalBytes` is zero, which it briefly is before
 * metadata resolves, and both clamp — a counter can read over-complete after
 * a re-check.
 *
 * @param t - The torrent summary.
 * @returns A value in `0..=1`.
 */
export function progressFraction(t: TorrentSummary): number {
  if (t.finished) return 1;
  if (t.totalBytes === 0) return 0;
  return Math.min(Math.max(t.progressBytes / t.totalBytes, 0), 1);
}

/**
 * Uploaded ÷ downloaded, or `null` when nothing has been downloaded.
 *
 * `null` rather than zero, because the two are different claims and the
 * callers want different things from them: a sort puts "no ratio" last, and
 * the detail panel renders it as `0.00`. Matches `policy::rules::ratio`,
 * which also returns nothing rather than zero — a seed limit must not fire
 * on a torrent that has downloaded nothing.
 *
 * The denominator is `progressBytes` in all three places. For a torrent
 * added on top of existing data that counts bytes never actually
 * transferred, but a limit or a column that disagreed with the figure on
 * screen would be worse than being theoretically precise.
 *
 * @param t - The torrent summary.
 * @returns The ratio, or `null` when there is no denominator.
 */
export function ratio(t: TorrentSummary): number | null {
  return t.progressBytes === 0 ? null : t.uploadedBytes / t.progressBytes;
}
