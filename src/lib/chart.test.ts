import { describe, expect, it } from "vitest";

import {
  WINDOW_SIZE,
  areaPath,
  chartCeiling,
  linePath,
  sampleAt,
  toPoints,
} from "./chart";

describe("chartCeiling", () => {
  it("uses the configured limit when there is one", () => {
    // With a limit set, the question is how close to it you are, and an axis
    // that rescaled with traffic could not answer that.
    expect(chartCeiling(3_000_000, 18_000_000)).toBe(18_000_000);
    expect(chartCeiling(50_000_000, 18_000_000)).toBe(18_000_000);
  });

  it("ignores a zero or absent limit", () => {
    expect(chartCeiling(4_000_000, null)).toBeGreaterThan(0);
    expect(chartCeiling(4_000_000, 0)).toBeGreaterThan(0);
  });

  it("rounds to 1, 2 or 5 times a power of ten", () => {
    expect(chartCeiling(4_000_000, null)).toBe(5_000_000);
    expect(chartCeiling(6_000_000, null)).toBe(10_000_000);
    expect(chartCeiling(17_900_000, null)).toBe(20_000_000);
    expect(chartCeiling(1_500_000, null)).toBe(2_000_000);
  });

  it("never scales below 1 MB/s", () => {
    // An idle session would otherwise scale to a few hundred bytes and render
    // background chatter as dramatic peaks — most alarming when nothing is
    // happening.
    expect(chartCeiling(0, null)).toBe(1_000_000);
    expect(chartCeiling(2_000, null)).toBe(1_000_000);
  });

  it("never returns zero, which would divide by zero downstream", () => {
    expect(chartCeiling(0, null)).toBeGreaterThan(0);
  });
});

describe("toPoints", () => {
  it("returns nothing for no samples", () => {
    expect(toPoints([], 620, 88, 1_000_000)).toEqual([]);
  });

  it("puts a full window across the whole width", () => {
    const values = Array.from({ length: WINDOW_SIZE }, () => 0);
    const points = toPoints(values, 620, 88, 1_000_000);

    expect(points[0].x).toBeCloseTo(0);
    expect(points[points.length - 1].x).toBeCloseTo(620);
  });

  it("right-aligns a partial window rather than stretching it", () => {
    // Ten seconds of history drawn across a full minute of width would claim
    // history the chart does not have.
    const points = toPoints([0, 0, 0], 620, 88, 1_000_000);

    expect(points[points.length - 1].x).toBeCloseTo(620);
    expect(points[0].x).toBeGreaterThan(500);
  });

  it("puts zero at the bottom and the ceiling at the top", () => {
    const points = toPoints([0, 1_000_000], 620, 88, 1_000_000);

    expect(points[0].y).toBeCloseTo(88);
    expect(points[1].y).toBeCloseTo(0);
  });

  it("clamps a sample above the ceiling to the top", () => {
    // Happens whenever a rate limit is momentarily overshot; the line should
    // flatten against the ceiling, not escape the box.
    const points = toPoints([5_000_000], 620, 88, 1_000_000);
    expect(points[0].y).toBe(0);
  });

  it("scales both series against the same ceiling", () => {
    const down = toPoints([2_000_000], 620, 88, 4_000_000);
    const up = toPoints([1_000_000], 620, 88, 4_000_000);

    // Half the ceiling is halfway up; a quarter is a quarter. If upload had
    // its own axis, a trickle and a torrent would draw the same height.
    expect(down[0].y).toBeCloseTo(44);
    expect(up[0].y).toBeCloseTo(66);
  });
});

describe("linePath", () => {
  it("returns nothing for fewer than two points", () => {
    expect(linePath([])).toBe("");
    expect(linePath([{ x: 0, y: 0 }])).toBe("");
  });

  it("steps between samples rather than sloping through them", () => {
    // A straight line between two readings claims the rate moved evenly
    // between them, which is a measurement nobody took.
    const d = linePath([
      { x: 0, y: 10 },
      { x: 10, y: 20 },
    ]);

    // Control points sit at the midpoint, holding each sample's own y.
    expect(d).toBe("M0.0 10.0 C5.0 10.0 5.0 20.0 10.0 20.0");
  });

  it("chains a segment per sample pair", () => {
    const d = linePath([
      { x: 0, y: 0 },
      { x: 10, y: 5 },
      { x: 20, y: 5 },
    ]);
    expect(d.match(/C/g)).toHaveLength(2);
  });
});

describe("areaPath", () => {
  it("returns nothing for fewer than two points", () => {
    expect(areaPath([{ x: 0, y: 0 }], 88)).toBe("");
  });

  it("closes the line down to the baseline", () => {
    const d = areaPath(
      [
        { x: 0, y: 10 },
        { x: 10, y: 20 },
      ],
      88,
    );

    expect(d.startsWith("M0.0 10.0")).toBe(true);
    expect(d.endsWith("L10.0 88 L0.0 88 Z")).toBe(true);
  });
});

describe("sampleAt", () => {
  it("returns nothing when there are no samples", () => {
    expect(sampleAt(300, 620, 0)).toBeNull();
  });

  it("finds the sample under the pointer", () => {
    expect(sampleAt(0, 620, WINDOW_SIZE)).toBe(0);
    expect(sampleAt(620, 620, WINDOW_SIZE)).toBe(WINDOW_SIZE - 1);
  });

  it("returns nothing for a non-finite pointer", () => {
    // What a caller gets when it divides by the width of an element that has
    // not been laid out. Better no tooltip than an index of NaN.
    expect(sampleAt(Number.NaN, 620, 10)).toBeNull();
    expect(sampleAt(0, 0, 10)).toBeNull();
  });

  it("clamps a pointer outside the drawn range", () => {
    // With a partial window the line does not reach the left edge, but the
    // hover target still spans the whole box.
    expect(sampleAt(0, 620, 3)).toBe(0);
    expect(sampleAt(9_999, 620, 3)).toBe(2);
  });
});
