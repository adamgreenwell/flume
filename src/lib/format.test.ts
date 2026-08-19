import { describe, expect, it } from "vitest";

import { formatBytes, formatDuration, formatSpeed } from "./format";

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

  it("uses one decimal below 10 in a unit and none above", () => {
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(20 * 1024)).toBe("20 KB");
  });

  it("scales through binary units", () => {
    expect(formatBytes(1024 ** 2)).toBe("1.0 MB");
    expect(formatBytes(1024 ** 3)).toBe("1.0 GB");
    expect(formatBytes(1024 ** 4)).toBe("1.0 TB");
  });

  it("clamps at the largest known unit instead of producing undefined", () => {
    expect(formatBytes(1024 ** 8)).toContain("PB");
  });
});

describe("formatSpeed", () => {
  it("appends a per-second suffix", () => {
    expect(formatSpeed(1024 ** 2)).toBe("1.0 MB/s");
    expect(formatSpeed(0)).toBe("0 B/s");
  });
});

describe("formatDuration", () => {
  it("renders zero and invalid input as 0s", () => {
    expect(formatDuration(0)).toBe("0s");
    expect(formatDuration(-1)).toBe("0s");
    expect(formatDuration(Number.NaN)).toBe("0s");
  });

  it("renders seconds only under a minute", () => {
    expect(formatDuration(45)).toBe("45s");
  });

  it("zero-pads seconds within a minute", () => {
    expect(formatDuration(185)).toBe("3m 05s");
  });

  it("switches to hours and pads minutes", () => {
    expect(formatDuration(7620)).toBe("2h 07m");
  });
});
