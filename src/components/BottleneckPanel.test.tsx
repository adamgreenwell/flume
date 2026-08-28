import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { Bottleneck } from "@/lib/ipc/types";

import { BottleneckPanel } from "./BottleneckPanel";

/**
 * Tests for the panel's one promise: it never claims more than it knows.
 *
 * The design says a wrong answer here is worse than no panel, so what is
 * asserted is the shape of the honesty — an unmeasured factor is not drawn as
 * a measured one, and a torrent that is not being limited gets no panel at
 * all. Exact classes are left alone; they are not the promise.
 */
const SWARM_BOUND: Bottleneck = {
  factors: [
    {
      name: "Peer upload",
      utilisation: 100,
      value: "6.6 MB/s",
      binding: true,
    },
    {
      name: "Your download cap",
      utilisation: 41,
      value: "16.0 MB/s",
      binding: false,
    },
  ],
  explanation: "The peers are supplying all they have.",
};

describe("BottleneckPanel", () => {
  it("renders nothing when the torrent is not being limited", () => {
    const { container } = render(<BottleneckPanel bottleneck={null} />);
    expect(container.firstChild).toBeNull();
  });

  it("marks exactly one factor as limiting", () => {
    render(<BottleneckPanel bottleneck={SWARM_BOUND} />);
    expect(screen.getAllByText("Limiting now")).toHaveLength(1);
    expect(screen.getAllByText("Headroom")).toHaveLength(1);
  });

  it("says what is limiting and whether a setting would help", () => {
    render(<BottleneckPanel bottleneck={SWARM_BOUND} />);
    expect(
      screen.getByText("The peers are supplying all they have."),
    ).toBeDefined();
  });

  /**
   * The reason `utilisation` is nullable at all. An empty bar reads as
   * "plenty of headroom", which is a claim Flume cannot make for a factor it
   * has no ceiling for.
   */
  it("does not draw a bar for a factor it cannot measure", () => {
    const unmeasured: Bottleneck = {
      factors: [
        {
          name: "Peer upload",
          utilisation: null,
          value: "6.6 MB/s",
          binding: false,
        },
      ],
      explanation: "Your cap is the limit.",
    };
    render(<BottleneckPanel bottleneck={unmeasured} />);

    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(screen.getByText("Not measured")).toBeDefined();
    expect(screen.queryByText("Headroom")).toBeNull();
  });

  it("exposes a measured factor as a real progressbar", () => {
    render(<BottleneckPanel bottleneck={SWARM_BOUND} />);
    const bars = screen.getAllByRole("progressbar");
    expect(bars).toHaveLength(2);
    expect(bars[1].getAttribute("aria-valuenow")).toBe("41");
  });

  /** Status is never colour alone — the verdict is a word, not just a tint. */
  it("names the verdict in words", () => {
    render(<BottleneckPanel bottleneck={SWARM_BOUND} />);
    expect(screen.getByText("Peer upload")).toBeDefined();
    expect(screen.getByText("Limiting now")).toBeDefined();
  });
});
