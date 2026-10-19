import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { PeerInfo, TorrentDetail, TorrentSummary } from "@/lib/ipc/types";

import { ExpandedRow } from "./ExpandedRow";

/**
 * Builds a summary with only the fields this panel reads.
 *
 * @param over - Fields to override.
 * @returns A torrent summary.
 */
function torrent(over: Partial<TorrentSummary> = {}): TorrentSummary {
  return {
    id: 1,
    infoHash: "a".repeat(40),
    name: "debian-13.2.0-amd64-DVD-1.iso",
    state: "seeding",
    progressBytes: 1000,
    totalBytes: 1000,
    uploadedBytes: 0,
    downloadBps: 0,
    uploadBps: 0,
    livePeers: 1,
    knownPeers: 239,
    health: "seeding",
    detail: "",
    etaSeconds: null,
    finished: true,
    addedAt: null,
    pauseReason: null,
    error: null,
    outputFolder: "/tmp",
    forced: false,
    queuePosition: null,
    ...over,
  };
}

/** A connected peer that has supplied nothing. */
function peer(index: number): PeerInfo {
  return {
    address: `10.0.0.${index}:6881`,
    client: null,
    transport: "tcp",
    state: "live",
    downloadedBytes: 0,
    uploadedBytes: 0,
    piecesContributed: 0,
    errors: 0,
  };
}

function detail(peers: PeerInfo[]): TorrentDetail {
  return {
    peers,
    trackers: [],
    pieces: null,
    swarm: {
      live: peers.length,
      connecting: 0,
      queued: 0,
      seen: 239,
      dead: 0,
      liveTcp: peers.length,
      liveUtp: 0,
      peerLimit: 128,
      seeds: null,
      availability: null,
      rarest: null,
    },
    note: { severity: "ok", title: "Seeding", body: "" },
    bottleneck: null,
  };
}

function show(peers: PeerInfo[]) {
  render(
    <ExpandedRow
      torrent={torrent()}
      detail={detail(peers)}
      error={null}
      onToggle={() => {}}
      onKeepSeeding={() => {}}
      onReveal={() => {}}
      onOpen={() => {}}
    />,
  );
}

describe("ExpandedRow contributors", () => {
  /** The bug: a single peer read "1 peers connected". */
  it("uses the singular for one connected peer", () => {
    show([peer(1)]);
    expect(
      screen.getByText(
        "1 peer connected, but it has not sent a verified piece yet.",
      ),
    ).toBeDefined();
  });

  it("uses the plural for more than one", () => {
    show([peer(1), peer(2)]);
    expect(
      screen.getByText(
        "2 peers connected, none has sent a verified piece yet.",
      ),
    ).toBeDefined();
  });

  it("says so plainly when nobody is connected", () => {
    show([]);
    expect(screen.getByText("No peers connected.")).toBeDefined();
  });
});

describe("a long save path", () => {
  const LONG =
    "\\\\Mac\\Home\\Downloads\\Buddy (2026) [2160p] [4K] [WEB] [5.1] [YTS.GG - YTS.BZ]";

  /**
   * Every flex ancestor of the path has to allow the shrink.
   *
   * Structural rather than behavioural, deliberately: jsdom has no layout
   * engine, so `scrollWidth` and `clientWidth` are both 0 and the overflow
   * this guards cannot be measured here. What *can* be pinned is the rule
   * that was broken — `min-width` defaults to `auto` on a flex item, which
   * refuses to shrink below min-content, and `truncate` makes the path's
   * min-content the entire string. One ancestor without `min-w-0` and the
   * truncation never engages, however correct the `Stat` itself is.
   *
   * Walked rather than asserted against two known lines, so inserting a new
   * wrapper into the chain fails this too.
   */
  it("can shrink the whole way down, so the path truncates", () => {
    render(
      <ExpandedRow
        torrent={torrent({ outputFolder: LONG })}
        detail={null}
        error={null}
        onToggle={() => {}}
        onKeepSeeding={() => {}}
        onReveal={() => {}}
        onOpen={() => {}}
      />,
    );

    const value = screen.getByText(LONG);
    expect(value.className).toContain("truncate");
    expect(value.getAttribute("title")).toBe(LONG);

    const offenders: string[] = [];
    for (
      let node = value.parentElement;
      node && !node.className.includes("pl-[52px]");
      node = node.parentElement
    ) {
      const classes = node.className;
      const isFlexRow =
        classes.includes("flex") && !classes.includes("flex-col");
      if (
        isFlexRow &&
        !classes.includes("min-w-0") &&
        !classes.includes("shrink-0")
      ) {
        offenders.push(classes);
      }
    }

    expect(offenders).toEqual([]);
  });
});

describe("keeping a torrent seeding", () => {
  it("offers Keep seeding instead of Resume when a limit stopped it", () => {
    // Instead of, not beside. Resume alone leaves the limit in force, so the
    // torrent would stop again within a second and the button would look
    // broken.
    render(
      <ExpandedRow
        torrent={torrent({ state: "paused", pauseReason: "ratioReached" })}
        detail={null}
        error={null}
        onToggle={() => {}}
        onKeepSeeding={() => {}}
        onReveal={() => {}}
        onOpen={() => {}}
      />,
    );

    expect(screen.getByText("Keep seeding")).toBeTruthy();
    expect(screen.queryByText("Resume")).toBeNull();
  });

  it("still offers Resume for a torrent the user paused", () => {
    render(
      <ExpandedRow
        torrent={torrent({ state: "paused", pauseReason: null })}
        detail={null}
        error={null}
        onToggle={() => {}}
        onKeepSeeding={() => {}}
        onReveal={() => {}}
        onOpen={() => {}}
      />,
    );

    expect(screen.getByText("Resume")).toBeTruthy();
    expect(screen.queryByText("Keep seeding")).toBeNull();
  });

  it("does not offer it for a queued torrent", () => {
    // A queue slot is not something the user can lift. Offering to fix it
    // would be offering to fix something that is not broken.
    render(
      <ExpandedRow
        torrent={torrent({ state: "paused", pauseReason: "queued" })}
        detail={null}
        error={null}
        onToggle={() => {}}
        onKeepSeeding={() => {}}
        onReveal={() => {}}
        onOpen={() => {}}
      />,
    );

    expect(screen.queryByText("Keep seeding")).toBeNull();
  });

  it("passes the torrent to the handler", () => {
    let got: string | null = null;
    render(
      <ExpandedRow
        torrent={torrent({ state: "paused", pauseReason: "seedTimeReached" })}
        detail={null}
        error={null}
        onToggle={() => {}}
        onKeepSeeding={(t) => {
          got = t.infoHash;
        }}
        onReveal={() => {}}
        onOpen={() => {}}
      />,
    );

    screen.getByText("Keep seeding").click();

    expect(got).toBe("a".repeat(40));
  });
});

/** Renders the panel for a summary with the given overrides. */
function withQueue(over: Partial<TorrentSummary>) {
  render(
    <ExpandedRow
      torrent={torrent(over)}
      detail={null}
      error={null}
      onToggle={() => {}}
      onKeepSeeding={() => {}}
      onReveal={() => {}}
      onOpen={() => {}}
    />,
  );
}

describe("the queue", () => {
  it("shows a place only when the queue holds one", () => {
    // A dash would imply the queue has an opinion about every torrent. A
    // running torrent that no limit applies to simply is not in a line.
    withQueue({ queuePosition: null, forced: false });
    expect(screen.queryByText("In queue")).toBeNull();
  });

  it("says where in the queue a waiting torrent is", () => {
    withQueue({ queuePosition: 3, forced: false });
    expect(screen.getByText("In queue")).toBeDefined();
    expect(screen.getByText("3")).toBeDefined();
  });

  it("says when the user started a torrent past the limits", () => {
    // Without this a torrent running over the limit is indistinguishable
    // from one the queue admitted, and the limit reads as broken rather
    // than overridden.
    withQueue({ queuePosition: null, forced: true });
    expect(screen.getByText("Started by you")).toBeDefined();
  });
});
