import { describe, expect, it } from "vitest";

import { STRIP_CELLS, resample } from "./PieceStrip";

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
