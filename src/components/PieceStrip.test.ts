import { describe, expect, it } from "vitest";

import { STRIP_CELLS, resample, resampleRarest } from "./PieceStrip";

describe("resample", () => {
  it("returns nothing for an empty map", () => {
    expect(resample([])).toEqual([]);
  });

  it("passes short input through untouched", () => {
    // A torrent with fewer pieces than cells should not have its map
    // stretched, invented, or padded.
    expect(resample([255, 0, 128])).toEqual([255, 0, 128]);
  });

  it("compresses a long map to the strip's cell count", () => {
    const buckets = Array.from({ length: 1600 }, () => 255);
    expect(resample(buckets)).toHaveLength(STRIP_CELLS);
  });

  it("averages rather than sampling, so a gap cannot vanish", () => {
    // Every other bucket empty. Picking one bucket per cell could land
    // entirely on the full ones and draw a solid strip over a half-empty
    // torrent.
    const buckets = Array.from({ length: 960 }, (_, i) =>
      i % 2 === 0 ? 255 : 0,
    );
    const cells = resample(buckets);

    expect(cells).toHaveLength(STRIP_CELLS);
    for (const level of cells) {
      expect(level).toBeGreaterThan(100);
      expect(level).toBeLessThan(160);
    }
  });

  it("keeps a solid head solid and an empty tail empty", () => {
    const buckets = [
      ...Array.from({ length: 800 }, () => 255),
      ...Array.from({ length: 800 }, () => 0),
    ];
    const cells = resample(buckets);

    expect(cells[0]).toBe(255);
    expect(cells[cells.length - 1]).toBe(0);
  });

  it("produces the requested count for any cell size", () => {
    const buckets = Array.from({ length: 1600 }, (_, i) => i % 256);
    expect(resample(buckets, 10)).toHaveLength(10);
    expect(resample(buckets, 200)).toHaveLength(200);
  });
});

describe("resampleRarest", () => {
  it("returns nothing for an empty map", () => {
    expect(resampleRarest([])).toEqual([]);
  });

  it("passes short input through untouched", () => {
    expect(resampleRarest([4, 9, 2])).toEqual([4, 9, 2]);
  });

  it("compresses a long map to the strip's cell count", () => {
    const counts = Array.from({ length: 1600 }, () => 5);
    expect(resampleRarest(counts)).toHaveLength(STRIP_CELLS);
  });

  /**
   * The reason this exists rather than reusing `resample`.
   *
   * A region where one piece is held by nobody is the thing the strip is for.
   * Averaging would smooth it into its well-served neighbours and hide exactly
   * the case that stalls a download.
   */
  it("keeps a region nobody holds rather than averaging it away", () => {
    // 1600 buckets, all well held except one.
    const counts = Array.from({ length: 1600 }, () => 9);
    counts[800] = 0;

    const cells = resampleRarest(counts);
    expect(cells).toContain(0);

    // Averaging would have buried it: the same input through `resample`
    // rounds back up to 9.
    expect(resample(counts)).not.toContain(0);
  });

  it("takes the minimum of the buckets a cell covers", () => {
    // 192 buckets into 96 cells: two buckets per cell.
    const counts = Array.from({ length: STRIP_CELLS * 2 }, (_, i) =>
      i === 1 ? 2 : 7,
    );
    const cells = resampleRarest(counts);
    expect(cells[0]).toBe(2);
    expect(cells[1]).toBe(7);
  });
});
