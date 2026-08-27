import type { PieceMap } from "@/lib/ipc/types";

/** How many cells the compact in-row strip draws. */
export const STRIP_CELLS = 96;

/**
 * Resamples the backend's buckets to a fixed cell count.
 *
 * The engine downsamples to up to 1600 buckets, which is right for the
 * inspector's full-width map and far too many for a 96-cell strip inside a
 * row. Averaging here rather than asking the engine for a second resolution
 * keeps one piece query serving both.
 *
 * @param buckets - Completion levels from the engine, `0..=255`.
 * @param cells - How many cells to produce.
 * @returns One level per cell, `0..=255`.
 */
export function resample(
  buckets: readonly number[],
  cells: number = STRIP_CELLS,
): number[] {
  if (buckets.length === 0) return [];
  if (buckets.length <= cells) return [...buckets];

  const out: number[] = [];
  for (let cell = 0; cell < cells; cell++) {
    const start = Math.floor((cell * buckets.length) / cells);
    const end = Math.max(
      start + 1,
      Math.floor(((cell + 1) * buckets.length) / cells),
    );

    let total = 0;
    for (let i = start; i < end; i++) total += buckets[i];
    out.push(Math.round(total / (end - start)));
  }
  return out;
}

/**
 * A level counts as verified above this.
 *
 * 250 rather than 255: a bucket covering dozens of pieces averages to just
 * under full when all but a rounding error are present, and drawing that as
 * "in flight" would leave a permanently speckled strip on a torrent that is
 * effectively complete.
 */
const VERIFIED = 250;

/** Below this a cell reads as untouched rather than partially filled. */
const REQUESTED = 8;

/** Props for {@link PieceStrip}. */
export interface PieceStripProps {
  /** Piece completion from the engine. */
  pieces: PieceMap;
}

/**
 * The compact piece map inside an expanded row.
 *
 * Answers "which parts do I have", which overall progress cannot: 60% with a
 * solid head and an empty tail is a torrent downloading in order, and 60%
 * scattered evenly is one pulling rarest-first. They behave differently when
 * the swarm thins out.
 *
 * Cells are proportional rather than fixed-width, so the strip always spans
 * the row whatever the piece count.
 *
 * @param props - See {@link PieceStripProps}.
 * @returns The rendered strip.
 */
export function PieceStrip({ pieces }: PieceStripProps) {
  const cells = resample(pieces.buckets);
  const percent =
    pieces.totalPieces === 0
      ? 0
      : Math.round((pieces.piecesComplete / pieces.totalPieces) * 100);

  return (
    <div className="flex flex-col gap-[7px]">
      <div
        className="flex h-[22px] gap-px"
        role="img"
        aria-label={`Piece map: ${pieces.piecesComplete.toLocaleString()} of ${pieces.totalPieces.toLocaleString()} pieces verified, ${percent}%`}
      >
        {cells.map((level, index) => (
          <span
            key={index}
            className={`grow rounded-[1px] ${
              level >= VERIFIED
                ? "bg-acc-dim"
                : level >= REQUESTED
                  ? "bg-acc"
                  : "bg-bg-3"
            }`}
          />
        ))}
      </div>

      <div className="text-fg-2 flex items-center gap-3.5 text-[11px]">
        <span className="flex items-center gap-1.5">
          <span className="bg-acc-dim block h-[9px] w-[9px] rounded-[2px]" />
          Verified
        </span>
        <span className="flex items-center gap-1.5">
          <span className="bg-acc block h-[9px] w-[9px] rounded-[2px]" />
          In flight
        </span>
        <span className="flex items-center gap-1.5">
          <span className="bg-bg-3 block h-[9px] w-[9px] rounded-[2px]" />
          Not yet requested
        </span>
        <span className="text-fg-3 flume-num ml-auto">
          {pieces.piecesComplete.toLocaleString()} of{" "}
          {pieces.totalPieces.toLocaleString()} pieces
        </span>
      </div>
    </div>
  );
}
