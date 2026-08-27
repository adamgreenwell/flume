import { describe, expect, it } from "vitest";

import type {
  SwarmHealth,
  TorrentState,
  TorrentSummary,
} from "@/lib/ipc/types";

import { VIEWS, matchesQuery, matchesView, viewCounts } from "./views";

/**
 * Builds a summary with only the fields a view actually inspects.
 *
 * @param over - Fields to override.
 * @returns A torrent summary.
 */
function torrent(over: Partial<TorrentSummary> = {}): TorrentSummary {
  return {
    id: 1,
    infoHash: "a".repeat(40),
    name: "debian-13.2.0-amd64-DVD-1.iso",
    state: "downloading" as TorrentState,
    progressBytes: 100,
    totalBytes: 1000,
    uploadedBytes: 0,
    downloadBps: 0,
    uploadBps: 0,
    livePeers: 0,
    knownPeers: 0,
    health: "unknown" as SwarmHealth,
    detail: "",
    etaSeconds: null,
    finished: false,
    error: null,
    outputFolder: "/tmp",
    ...over,
  };
}

describe("matchesView", () => {
  it("puts everything in All", () => {
    expect(matchesView(torrent({ state: "error" }), "all")).toBe(true);
  });

  it("counts a checking torrent as downloading", () => {
    // Re-hashing is work in progress toward a download. Filing it elsewhere
    // makes a torrent vanish from the view the user is watching it in.
    expect(matchesView(torrent({ state: "checking" }), "downloading")).toBe(
      true,
    );
  });

  it("treats Completed as finished, not as seeding", () => {
    // A finished torrent that the user paused is still completed.
    expect(
      matchesView(torrent({ state: "paused", finished: true }), "completed"),
    ).toBe(true);
    expect(
      matchesView(torrent({ state: "seeding", finished: false }), "completed"),
    ).toBe(false);
  });

  describe("Needs attention", () => {
    it("includes anything stopped by a failure", () => {
      expect(matchesView(torrent({ state: "error" }), "attention")).toBe(true);
    });

    it("includes a download with nobody to ask", () => {
      expect(matchesView(torrent({ health: "none" }), "attention")).toBe(true);
    });

    it("leaves out a torrent the user paused on purpose", () => {
      // Paused is a decision, not a problem. A view that lists the user's own
      // choices back at them as faults stops being worth opening.
      expect(
        matchesView(torrent({ state: "paused", health: "idle" }), "attention"),
      ).toBe(false);
    });

    it("leaves out active downloads whose swarm cannot be judged", () => {
      // `unknown` means Flume cannot tell a thin swarm from a healthy one.
      // Sweeping every active download in here would make the view useless.
      expect(matchesView(torrent({ health: "unknown" }), "attention")).toBe(
        false,
      );
    });
  });
});

describe("matchesQuery", () => {
  it("matches everything when the query is blank", () => {
    expect(matchesQuery(torrent(), "")).toBe(true);
    expect(matchesQuery(torrent(), "   ")).toBe(true);
  });

  it("matches case-insensitively on a substring", () => {
    expect(matchesQuery(torrent(), "DEBIAN")).toBe(true);
    expect(matchesQuery(torrent(), "amd64")).toBe(true);
  });

  it("does not match an unrelated query", () => {
    expect(matchesQuery(torrent(), "fedora")).toBe(false);
  });
});

describe("viewCounts", () => {
  it("counts every view, including the empty ones", () => {
    const counts = viewCounts([
      torrent({ id: 1, state: "downloading" }),
      torrent({ id: 2, state: "seeding", finished: true }),
      torrent({ id: 3, state: "error", health: "idle" }),
    ]);

    expect(counts.all).toBe(3);
    expect(counts.downloading).toBe(1);
    expect(counts.seeding).toBe(1);
    expect(counts.completed).toBe(1);
    expect(counts.attention).toBe(1);
    // Present and zero, not absent — the rail renders a badge for every view.
    expect(counts.paused).toBe(0);
  });

  it("returns a count for every view the rail lists", () => {
    const counts = viewCounts([]);
    for (const view of VIEWS) {
      expect(counts[view.id]).toBe(0);
    }
  });
});
