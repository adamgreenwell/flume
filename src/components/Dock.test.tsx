import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { CoreStatus, TorrentSummary } from "@/lib/ipc/types";

import { Dock } from "./Dock";

const CORE: CoreStatus = {
  clientVersion: "Flume 1.0.0",
  listenPort: 42221,
  announcePort: 42221,
  dht: { enabled: true, nodesV4: 300, nodesV6: 12, outstandingRequests: 0 },
  downloadDir: "/Users/test/Downloads",
  uptimeSeconds: 90,
  downloadBps: 4_000_000,
  uploadBps: 250_000,
  livePeers: 24,
  health: "ready",
};

const TORRENT = {
  id: 1,
  infoHash: "a".repeat(40),
  name: "example",
  state: "downloading",
  finished: false,
  progress: 0.5,
  totalBytes: 1000,
  downloadedBytes: 500,
  uploadedBytes: 100,
  downloadBps: 4_000_000,
  uploadBps: 250_000,
  livePeers: 24,
  knownPeers: 60,
  etaSeconds: 60,
  ratio: 0.2,
  health: "healthy",
  detail: "",
} as unknown as TorrentSummary;

describe("Dock while the egress guard holds", () => {
  it("reads every figure as a dash rather than a zero", () => {
    // A zero is a measurement. While held there is no torrent session at all,
    // so "0 connected peers" claims something false in a more believable way
    // than a stale number would.
    render(
      <Dock
        status={null}
        torrents={[]}
        history={[]}
        limitBps={null}
        held={true}
      />,
    );

    for (const label of [
      "Active",
      "Session down",
      "Session up",
      "Share ratio",
      "Connected peers",
      "Known peers",
      "DHT nodes",
      "Uptime",
    ]) {
      const tile = screen.getByText(label).closest("div");
      expect(tile?.textContent, `${label} should read as a dash`).toContain(
        "—",
      );
    }

    expect(screen.queryByText("0 of 0")).toBeNull();
  });

  it("shows real figures when the guard is not holding", () => {
    render(
      <Dock
        status={CORE}
        torrents={[TORRENT]}
        history={[]}
        limitBps={null}
        held={false}
      />,
    );

    // `getByText` throws when absent, so finding them is the assertion.
    expect(screen.getByText("1 of 1")).toBeTruthy();
    expect(screen.getByText("24")).toBeTruthy();
    expect(screen.getByText("312")).toBeTruthy();
  });
});
