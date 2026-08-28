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

/**
 * Resamples copy counts to the same cell count as the completion strip.
 *
 * Takes the *minimum* across the pieces a cell covers, not the mean, for the
 * same reason the engine buckets by minimum: this strip is here to show where
 * a torrent is about to stall, and a cell averaging eight copies while
 * covering one piece nobody holds is precisely what must not be smoothed away.
 *
 * @param counts - Per-bucket copy counts from the engine.
 * @param cells - How many cells to produce.
 * @returns One copy count per cell.
 */
export function resampleRarest(
  counts: readonly number[],
  cells: number = STRIP_CELLS,
): number[] {
  if (counts.length === 0) return [];
  if (counts.length <= cells) return [...counts];

  const out: number[] = [];
  for (let cell = 0; cell < cells; cell++) {
    const start = Math.floor((cell * counts.length) / cells);
    const end = Math.max(
      start + 1,
      Math.floor(((cell + 1) * counts.length) / cells),
    );

    let lowest = Infinity;
    for (let i = start; i < end; i++) lowest = Math.min(lowest, counts[i]);
    out.push(lowest === Infinity ? 0 : lowest);
  }
  return out;
}

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
  const rarest = pieces.availability
    ? resampleRarest(pieces.availability)
    : null;
  // The tallest bar is the best-held region, so every other bar is read
  // against it. A floor of 1 keeps a swarm where nothing is held from
  // dividing by zero.
  const peak = rarest ? Math.max(1, ...rarest) : 1;
  const low = rarest ? Math.min(...rarest) : 0;
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

      {rarest ? (
        <div
          className="flex h-[16px] items-end gap-px"
          role="img"
          aria-label={
            low === 0
              ? `Availability: some regions are held by no connected peer, the best by ${peak}`
              : `Availability: every region is held by between ${low} and ${peak} peers`
          }
        >
          {rarest.map((copies, index) => (
            <span
              key={index}
              className={`grow rounded-[1px] ${copies === 0 ? "bg-err" : "bg-acc-dim"}`}
              // A region nobody holds draws the *tallest* bar, in the error
              // colour. It is the most important thing on the strip, so a
              // zero-height bar would be the least visible thing on it.
              //
              // Held regions are capped at 80% so a zero always stands taller
              // than any of them. Without that cap the strip distinguishes
              // zero by colour alone whenever availability is flat: every bar
              // reaches the top, and the one region that cannot be finished
              // looks exactly like the rest.
              style={{
                height:
                  copies === 0
                    ? "100%"
                    : `${Math.max(12, (copies / peak) * 80)}%`,
              }}
            />
          ))}
        </div>
      ) : null}

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
        <span className="text-fg-3 ml-auto">
          {rarest === null ? (
            <span className="flume-num">
              {pieces.piecesComplete.toLocaleString()} of{" "}
              {pieces.totalPieces.toLocaleString()} pieces
            </span>
          ) : low === 0 ? (
            <>
              Bars below: no peer holds some regions ·{" "}
              <span className="flume-num">
                {pieces.piecesComplete.toLocaleString()} of{" "}
                {pieces.totalPieces.toLocaleString()}
              </span>
            </>
          ) : (
            <>
              Bars below: peers holding each region{" "}
              <span className="flume-num">
                ({low} to {peak})
              </span>
            </>
          )}
        </span>
      </div>
    </div>
  );
}
