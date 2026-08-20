"use client";

import type { PieceMap } from "@/lib/ipc/types";

/** Props for {@link PieceHeatmap}. */
export interface PieceHeatmapProps {
  /** Downsampled piece completion from the backend. */
  pieces: PieceMap;
}

/**
 * A strip showing which parts of a torrent are on disk.
 *
 * Rendered as a CSS gradient over a fixed set of buckets rather than one
 * element per piece: a large torrent has hundreds of thousands of pieces, and
 * a DOM node each would be unusable. The backend has already downsampled to at
 * most 1600 buckets, each a 0–255 completion level.
 *
 * Uses opacity over the accent colour rather than a hue ramp, so it reads
 * correctly in both themes and does not rely on colour discrimination to
 * convey "more" versus "less".
 *
 * @param props - See {@link PieceHeatmapProps}.
 * @returns The rendered strip.
 */
export function PieceHeatmap({ pieces }: PieceHeatmapProps) {
  const { buckets, totalPieces, piecesPerBucket } = pieces;

  if (buckets.length === 0) {
    return (
      <p className="text-faint text-xs">
        No piece information yet — this appears once the torrent is running.
      </p>
    );
  }

  const complete = buckets.filter((b) => b === 255).length;
  const percentComplete = Math.round((complete / buckets.length) * 100);

  return (
    <div className="flex flex-col gap-2">
      <div
        className="border-border-subtle bg-surface-raised flex h-10 w-full overflow-hidden rounded-md border"
        role="img"
        aria-label={`Piece map: roughly ${percentComplete}% of ${totalPieces} pieces downloaded`}
      >
        {buckets.map((level, index) => (
          <div
            key={index}
            className="bg-accent h-full flex-1"
            // Opacity rather than a colour ramp: legible in both themes, and
            // it degrades to a plain intensity scale for colour-blind users.
            style={{ opacity: level / 255 }}
          />
        ))}
      </div>
      <p className="text-faint text-xs">
        {totalPieces.toLocaleString()} pieces
        {piecesPerBucket > 1
          ? ` · ${piecesPerBucket} per column (downsampled to fit)`
          : ""}
      </p>
    </div>
  );
}
