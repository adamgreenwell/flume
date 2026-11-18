import { describe, expect, it } from "vitest";

import {
  COLUMNS,
  MAX_COLUMN_WIDTH,
  clampWidth,
  columnStyle,
  minRowWidth,
  resolveWidths,
  toSaved,
} from "./columns";

describe("resolveWidths", () => {
  it("uses each column's default when nothing was saved", () => {
    const widths = resolveWidths(null);
    for (const c of COLUMNS) expect(widths[c.id]).toBe(c.defaultWidth);
  });

  it("keeps a saved width and clamps one out of range", () => {
    const widths = resolveWidths({
      progress: 300,
      down: 1,
      up: 5000,
      peers: null,
      health: null,
    });
    expect(widths.progress).toBe(300);
    expect(widths.down).toBe(COLUMNS.find((c) => c.id === "down")!.minWidth);
    expect(widths.up).toBe(MAX_COLUMN_WIDTH);
    expect(widths.peers).toBe(78);
  });
});

describe("toSaved", () => {
  it("stores a column at its default as null, so it follows later defaults", () => {
    const widths = resolveWidths(null);
    widths.down = 120;
    expect(toSaved(widths)).toEqual({
      progress: null,
      down: 120,
      up: null,
      peers: null,
      health: null,
    });
  });
});

describe("clampWidth", () => {
  it("rounds to a whole pixel", () => {
    expect(clampWidth("progress", 200.6)).toBe(201);
  });

  it("falls back to the default for a width that is not a number", () => {
    expect(clampWidth("progress", Number.NaN)).toBe(180);
  });
});

describe("columnStyle", () => {
  it("publishes every width and the row minimum", () => {
    const widths = resolveWidths(null);
    const style = columnStyle(widths);
    expect(style["--flume-col-progress"]).toBe("180px");
    expect(style["--flume-list-min-w"]).toBe(`${minRowWidth(widths)}px`);
  });

  it("grows the row minimum with a widened column", () => {
    const narrow = minRowWidth(resolveWidths(null));
    const wide = minRowWidth(resolveWidths({ progress: 280 } as never));
    expect(wide - narrow).toBe(100);
  });
});
