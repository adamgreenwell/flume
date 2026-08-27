import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { EngineHealth, TorrentState } from "@/lib/ipc/types";

import { Button } from "./Button";
import { Icon, type IconName } from "./Icon";
import { IconButton } from "./IconButton";
import { ProgressBar } from "./ProgressBar";
import { StatCard } from "./StatCard";
import { StatusPill } from "./StatusPill";

/**
 * Tests for the design-system invariants of the shared primitives.
 *
 * These assert the rules the design calls non-negotiable rather than exact
 * pixel values. A class list is a brittle thing to assert on: pinning one
 * fails on any legitimate refactor while still not proving the rule holds.
 * What is checked here is what a redesign must not break — status is never
 * colour alone, a bar always carries its number, an icon-only control always
 * has a name.
 *
 * Plain matchers rather than `@testing-library/jest-dom`, matching the rest of
 * the suite; the extra dependency buys readability the project has so far
 * chosen not to pay for.
 */

const NAMES: IconName[] = [
  "pause",
  "play",
  "folder",
  "files",
  "trash",
  "plus",
  "settings",
  "search",
  "chevron-right",
  "chevron-down",
  "arrow-down",
  "arrow-up",
  "check",
  "dash",
  "clock",
  "check-circle",
  "alert-circle",
  "alert-triangle",
];

describe("Icon", () => {
  it.each(NAMES)("draws %s on the 16 grid with a path", (name) => {
    const { container } = render(<Icon name={name} />);
    const svg = container.querySelector("svg");

    expect(svg?.getAttribute("viewBox")).toBe("0 0 16 16");
    expect(svg?.querySelector("path")?.getAttribute("d")).toBeTruthy();
  });

  it("is hidden from assistive tech, because its control carries the name", () => {
    const { container } = render(<Icon name="folder" />);
    expect(container.querySelector("svg")?.getAttribute("aria-hidden")).toBe(
      "true",
    );
  });

  it("holds a constant optical stroke weight as the size changes", () => {
    // 1.5 user units on a 16 grid rendered at 16px is 1.5 device pixels. The
    // same 1.5 at 32px would be 3 — twice the weight — so stroke-width has to
    // scale inversely for a large icon to sit beside a small one.
    const width = (root: HTMLElement) =>
      Number(root.querySelector("svg")?.getAttribute("stroke-width"));

    expect(
      width(render(<Icon name="folder" size={16} />).container),
    ).toBeCloseTo(1.5);
    expect(
      width(render(<Icon name="folder" size={32} />).container),
    ).toBeCloseTo(0.75);
  });

  it("strokes every glyph rather than filling any", () => {
    // A filled glyph beside stroked ones reads as a different weight class.
    for (const name of NAMES) {
      const { container } = render(<Icon name={name} />);
      const svg = container.querySelector("svg");

      expect(svg?.getAttribute("fill")).toBe("none");
      expect(svg?.getAttribute("stroke")).toBe("currentColor");
    }
  });
});

describe("Button", () => {
  it("defaults to type=button so it cannot submit a surrounding form", () => {
    render(<Button>Add torrent</Button>);
    expect(screen.getByRole("button").getAttribute("type")).toBe("button");
  });

  it("keeps an explicit type when one is given", () => {
    render(<Button type="submit">Add torrent</Button>);
    expect(screen.getByRole("button").getAttribute("type")).toBe("submit");
  });

  it("renders a disabled primary without fading its label away", () => {
    // The disabled label usually names the thing you cannot yet do, so it has
    // to stay readable. `fg-3` is the 4.5:1 floor; `fg-dis` would not be.
    render(
      <Button variant="primary" disabled>
        Add 0 files
      </Button>,
    );
    const button = screen.getByRole("button") as HTMLButtonElement;

    expect(button.disabled).toBe(true);
    expect(button.className).toContain("disabled:text-fg-3");
    expect(button.className).not.toContain("disabled:opacity");
  });
});

describe("IconButton", () => {
  it("always exposes an accessible name", () => {
    render(<IconButton icon="trash" label="Remove" />);
    expect(screen.getByRole("button", { name: "Remove" })).toBeTruthy();
  });

  it("stays reachable rather than appearing on hover", () => {
    // Hiding actions until hover strands keyboard and touch users. The control
    // dims by colour and is always in the tree and always hit-testable.
    render(<IconButton icon="pause" label="Pause" />);
    const button = screen.getByRole("button", { name: "Pause" });

    expect(button.className).not.toContain("hidden");
    expect(button.className).not.toContain("opacity-0");
  });
});

describe("ProgressBar", () => {
  it("reports its value to assistive tech", () => {
    render(
      <ProgressBar value={0.427} state="downloading" label="Sprite Fright" />,
    );
    const bar = screen.getByRole("progressbar", { name: "Sprite Fright" });

    expect(bar.getAttribute("aria-valuenow")).toBe("43");
    expect(bar.getAttribute("aria-valuemin")).toBe("0");
    expect(bar.getAttribute("aria-valuemax")).toBe("100");
  });

  it("always shows the percentage beside the bar", () => {
    // At 5px tall a 3% fill and a 0% fill are the same two pixels, so the
    // number is the only thing carrying the value at the low end.
    render(<ProgressBar value={0.03} state="downloading" label="Debian" />);
    expect(screen.getByText("3%")).toBeTruthy();
  });

  it("hides the visible percentage from assistive tech to avoid saying it twice", () => {
    render(<ProgressBar value={0.68} state="downloading" label="Debian" />);
    expect(screen.getByText("68%").getAttribute("aria-hidden")).toBe("true");
  });

  it("clamps values outside 0..1", () => {
    const { rerender } = render(
      <ProgressBar value={-4} state="error" label="Debian" />,
    );
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe(
      "0",
    );

    rerender(<ProgressBar value={9} state="seeding" label="Debian" />);
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe(
      "100",
    );
  });

  it("gives every lifecycle state its own fill", () => {
    const states: TorrentState[] = [
      "checking",
      "downloading",
      "seeding",
      "paused",
      "error",
    ];
    const fills = states.map((state) => {
      const { container } = render(
        <ProgressBar value={0.5} state={state} label={state} />,
      );
      return container.querySelector('[role="progressbar"] > div')?.className;
    });

    expect(new Set(fills).size).toBe(states.length);
  });
});

describe("StatusPill", () => {
  it.each<[EngineHealth, string]>([
    ["starting", "Starting"],
    ["connecting", "Connecting"],
    ["ready", "Ready"],
    ["degraded", "Degraded"],
  ])("labels %s in words, never colour alone", (health, label) => {
    render(<StatusPill health={health} />);
    expect(screen.getByRole("status").textContent).toContain(label);
  });

  it("announces changes politely", () => {
    render(<StatusPill health="ready" />);
    expect(screen.getByRole("status").getAttribute("aria-live")).toBe("polite");
  });

  it("hides the indicator dot, which only repeats the word", () => {
    const { container } = render(<StatusPill health="ready" />);
    expect(container.querySelector("[aria-hidden='true']")).not.toBeNull();
  });
});

describe("StatCard", () => {
  it("renders label, value and caption at strip size", () => {
    render(
      <StatCard
        size="strip"
        label="Availability"
        value="4.2×"
        hint="rarest piece on 4 peers"
      />,
    );

    expect(screen.getByText("Availability")).toBeTruthy();
    expect(screen.getByText("4.2×")).toBeTruthy();
    expect(screen.getByText("rarest piece on 4 peers")).toBeTruthy();
  });

  it("drops the caption at dock size, which has no room for one", () => {
    render(<StatCard label="Uptime" value="4 d 07 h" hint="since 08:12" />);

    expect(screen.getByText("4 d 07 h")).toBeTruthy();
    expect(screen.queryByText("since 08:12")).toBeNull();
  });

  it("renders values in the tabular mono face so columns cannot jitter", () => {
    render(<StatCard label="Share ratio" value="0.22" />);
    expect(screen.getByText("0.22").className).toContain("flume-num");
  });
});
