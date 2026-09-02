import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { CoreStatus, GuardStatus } from "@/lib/ipc/types";
import { VIEWS, type ViewId } from "@/lib/views";

import { Rail } from "./Rail";

const STATUS: CoreStatus = {
  clientVersion: "Flume 1.1.0",
  listenPort: 42221,
  announcePort: 42221,
  dht: { enabled: true, nodesV4: 1400, nodesV6: 56, outstandingRequests: 0 },
  downloadDir: "/Users/test/Downloads",
  uptimeSeconds: 90,
  downloadBps: 0,
  uploadBps: 0,
  livePeers: 2,
  health: "ready",
};

const HELD: GuardStatus = {
  guard: "hold",
  report: {
    path: { v4: { interface: "en7", kind: "ordinary" }, v6: null },
    verdict: { verdict: "direct", interface: "en7" },
  },
  held: true,
  resumesInSeconds: null,
};

const TUNNELLED: GuardStatus = {
  guard: "hold",
  report: {
    path: { v4: { interface: "utun12", kind: "tunnel" }, v6: null },
    verdict: {
      verdict: "tunnelled",
      interface: "utun12",
      otherFamilyOutside: false,
    },
  },
  held: false,
  resumesInSeconds: null,
};

const COUNTS: Record<ViewId, number> = {
  all: 7,
  downloading: 2,
  seeding: 4,
  attention: 1,
  completed: 5,
  paused: 0,
};

function renderRail(over: Partial<Parameters<typeof Rail>[0]> = {}) {
  const props = {
    view: "all" as ViewId,
    onViewChange: vi.fn(),
    counts: COUNTS,
    query: "",
    onQueryChange: vi.fn(),
    status: STATUS,
    loading: false,
    onToggleCollapsed: vi.fn(),
    ...over,
  };
  const utils = render(<Rail {...props} />);
  return { ...utils, props };
}

describe("Rail, collapsed", () => {
  it("keeps every view reachable and named once the text is gone", () => {
    // A `title` is a tooltip, not an accessible name. Each icon button has to
    // answer to its view's name, and the count was part of what the row said.
    renderRail({ collapsed: true });

    for (const v of VIEWS) {
      const button = screen.getByRole("button", {
        name: new RegExp(`^${v.name}, ${COUNTS[v.id]}$`),
      });
      expect(button).toBeDefined();
    }
  });

  it("announces its state on the toggle", () => {
    const { rerender, props } = renderRail({ collapsed: true });
    expect(
      screen
        .getByRole("button", { name: "Expand sidebar" })
        .getAttribute("aria-expanded"),
    ).toBe("false");

    rerender(<Rail {...props} collapsed={false} />);
    expect(
      screen
        .getByRole("button", { name: "Collapse sidebar" })
        .getAttribute("aria-expanded"),
    ).toBe("true");
  });

  it("expands on / rather than swallowing the key", () => {
    // The one thing a shortcut must never do is nothing. With no search field
    // to focus, `/` has to produce the field.
    const { props } = renderRail({ collapsed: true });
    expect(screen.queryByRole("textbox")).toBeNull();

    fireEvent.keyDown(window, { key: "/" });

    expect(props.onToggleCollapsed).toHaveBeenCalledTimes(1);
  });

  it("focuses search once the / it triggered has expanded the rail", () => {
    const { rerender, props } = renderRail({ collapsed: true });
    fireEvent.keyDown(window, { key: "/" });

    // The page flips the prop on the next render; the focus has to wait for
    // the input to exist rather than be lost in the gap.
    rerender(<Rail {...props} collapsed={false} />);

    expect(document.activeElement).toBe(screen.getByRole("textbox"));
  });

  it("carries the whole network footer in one accessible name", () => {
    // `role="img"`, not `role="status"`: a live region whose text never
    // changes can never announce, and there is no text inside it to read.
    // The name has to carry all three lines, including a held guard.
    renderRail({ collapsed: true, guard: HELD });

    const badge = screen.getByRole("img", {
      name: /Held · en7 is not a tunnel/,
    });
    const name = badge.getAttribute("aria-label") ?? "";
    expect(name).toMatch(/DHT · 1,456 nodes/);
    expect(name).toMatch(/Port 42221 open/);
  });

  it("distinguishes held from healthy by more than colour", () => {
    // The binding rule, and the whole reason this rail collapses to 56px
    // rather than to zero. `--flume-ok` against `--flume-warn` is 1.09:1 --
    // same luminance, different hue -- so a coloured dot alone says nothing
    // to a protanope or on a greyscale display, which would leave the icon
    // rail exactly as useful as no rail for the users it was kept for.
    const held = renderRail({ collapsed: true, guard: HELD });
    expect(screen.getByText("Held")).toBeDefined();
    held.unmount();

    renderRail({ collapsed: true, guard: TUNNELLED });
    expect(screen.getByText("Net")).toBeDefined();
    expect(screen.queryByText("Held")).toBeNull();
  });

  it("keeps focus on the toggle across a collapse", () => {
    // The toggle must hold one position in the tree. Alternating between a
    // <button> and a wrapped <span> makes React remount it, which drops focus
    // to <body> -- so a keyboard user who Tabs to the chevron and presses
    // Enter loses their place and cannot immediately expand again.
    const { rerender, props } = renderRail({ collapsed: false });
    const before = screen.getByRole("button", { name: "Collapse sidebar" });
    before.focus();
    expect(document.activeElement).toBe(before);

    rerender(<Rail {...props} collapsed={true} />);

    const after = screen.getByRole("button", { name: "Expand sidebar" });
    expect(after).toBe(before);
    expect(document.activeElement).toBe(after);
  });

  it("moves focus to the toggle when collapsing out from under search", () => {
    // Collapsing unmounts the input. On WKWebView a click does not move
    // focus, so a user typing in search who clicks the chevron would be left
    // on <body>; the toggle is the nearest thing that can bring it back.
    const { rerender, props } = renderRail({ collapsed: false });
    screen.getByRole("textbox").focus();

    rerender(<Rail {...props} collapsed={true} />);

    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: "Expand sidebar" }),
    );
  });

  it("does not draw the search field or the view names", () => {
    renderRail({ collapsed: true });
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.queryByText("Downloading")).toBeNull();
  });
});

describe("Rail, expanded", () => {
  it("focuses search on / as before", () => {
    renderRail({ collapsed: false });
    fireEvent.keyDown(window, { key: "/" });
    expect(document.activeElement).toBe(screen.getByRole("textbox"));
  });

  it("renders the footer as three lines in words", () => {
    renderRail({ collapsed: false, guard: HELD });
    expect(screen.getByText(/Held · en7 is not a tunnel/)).toBeDefined();
    expect(screen.getByText(/nodes/)).toBeDefined();
    expect(screen.getByText(/open/)).toBeDefined();
  });

  it("hides the toggle when no handler is given", () => {
    renderRail({ collapsed: false, onToggleCollapsed: undefined });
    expect(screen.queryByRole("button", { name: /sidebar/ })).toBeNull();
  });
});
