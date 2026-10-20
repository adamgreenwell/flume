import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ThroughputChart } from "./ThroughputChart";

describe("the throughput chart's header", () => {
  /**
   * Just above `lg` the dock gives the chart about 270px, and the header's
   * four items do not fit in one row there. Without a wrap they squeezed to
   * min-content — the title over three lines, "1.0 / MB/s" over two — and
   * then overflowed onto the stat grid beside the chart (#232).
   *
   * jsdom has no layout, so this pins the two classes that make the row
   * behave: the row may wrap, and no item may wrap inside itself.
   */
  it.each([
    ["without a limit", null, "Peak"],
    ["against a limit", 18_000_000, "Limit"],
  ])("wraps between items, never within one, %s", (_, limitBps, ceiling) => {
    render(<ThroughputChart history={[]} limitBps={limitBps} />);

    const title = screen.getByText(/Session throughput · last/);
    const row = title.parentElement;
    expect(row?.className).toContain("flex-wrap");

    const items = Array.from(row?.children ?? []);
    expect(items.map((item) => item.textContent)).toEqual([
      expect.stringContaining("Session throughput"),
      "Download",
      "Upload",
      expect.stringContaining(ceiling),
    ]);
    for (const item of items) {
      expect(
        item.className,
        `"${item.textContent}" must not wrap inside itself`,
      ).toContain("whitespace-nowrap");
    }
  });
});
