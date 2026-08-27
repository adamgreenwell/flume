import { describe, expect, it } from "vitest";

import { formatBytes, formatDuration, formatSpeed } from "./format";

/** 1000^8, well past the largest unit the formatter knows. */
const STEP_TO_THE_EIGHTH = 1000 ** 8;

describe("formatBytes", () => {
  it("renders zero and negatives as 0 B", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-5)).toBe("0 B");
  });

  it("renders non-finite input as 0 B rather than NaN", () => {
    expect(formatBytes(Number.NaN)).toBe("0 B");
    expect(formatBytes(Number.POSITIVE_INFINITY)).toBe("0 B");
  });

  it("uses whole numbers for bytes", () => {
    expect(formatBytes(512)).toBe("512 B");
  });

  it("scales decimally, not binarily", () => {
    // The distinction is the whole point: a 2 TB disk holds 2e12 bytes, and
    // rendering that as 1.82 TB tells the user their disk is smaller than the
    // label on it.
    expect(formatBytes(1_000)).toBe("1.00 KB");
    expect(formatBytes(1_000_000)).toBe("1.00 MB");
    expect(formatBytes(1_000_000_000)).toBe("1.00 GB");
    expect(formatBytes(1_000_000_000_000)).toBe("1.00 TB");
  });

  it("holds three significant figures across the range", () => {
    // The design's own fixture sizes, which is what these have to render as.
    expect(formatBytes(46_100_000_000)).toBe("46.1 GB");
    expect(formatBytes(1_190_000_000)).toBe("1.19 GB");
    expect(formatBytes(84_900_000_000)).toBe("84.9 GB");
    expect(formatBytes(231_000_000_000)).toBe("231 GB");
  });

  it("clamps at the largest known unit instead of producing undefined", () => {
    expect(formatBytes(STEP_TO_THE_EIGHTH)).toContain("PB");
  });
});

describe("formatSpeed", () => {
  it("renders zero and non-finite input as 0 B/s", () => {
    expect(formatSpeed(0)).toBe("0 B/s");
    expect(formatSpeed(-1)).toBe("0 B/s");
    expect(formatSpeed(Number.NaN)).toBe("0 B/s");
  });

  it("uses one decimal, not three significant figures", () => {
    // A rate is redrawn every second. Two decimals on a number that changes
    // every tick is motion the eye tracks and cannot use.
    expect(formatSpeed(6_600_000)).toBe("6.6 MB/s");
    expect(formatSpeed(17_900_000)).toBe("17.9 MB/s");
  });

  it("holds MB/s down to 0.1 so a rate column reads as one unit", () => {
    expect(formatSpeed(900_000)).toBe("0.9 MB/s");
    expect(formatSpeed(400_000)).toBe("0.4 MB/s");
    expect(formatSpeed(100_000)).toBe("0.1 MB/s");
  });

  it("drops below MB/s rather than rendering a real rate as 0.0", () => {
    expect(formatSpeed(99_000)).toBe("99.0 KB/s");
    expect(formatSpeed(512)).toBe("512 B/s");
  });
});

describe("formatDuration", () => {
  it("renders zero and invalid input as 0 s", () => {
    expect(formatDuration(0)).toBe("0 s");
    expect(formatDuration(-1)).toBe("0 s");
    expect(formatDuration(Number.NaN)).toBe("0 s");
  });

  it("renders seconds only under a minute", () => {
    expect(formatDuration(45)).toBe("45 s");
  });

  it("zero-pads seconds within a minute", () => {
    expect(formatDuration(185)).toBe("3 min 05 s");
  });

  it("switches to hours and pads minutes", () => {
    expect(formatDuration(7620)).toBe("2 h 07 min");
  });

  it("agrees with the Rust formatter on the engine's own examples", () => {
    // The same four assertions exist in `format_duration`'s tests in
    // `src-tauri/src/engine/torrent.rs`. Both sides render durations the user
    // sees side by side, so they cannot be allowed to drift.
    expect(formatDuration(4020)).toBe("1 h 07 min");
    expect(formatDuration(150)).toBe("2 min 30 s");
    expect(formatDuration(45)).toBe("45 s");
    expect(formatDuration(0)).toBe("0 s");
  });
});
