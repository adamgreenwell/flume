import { describe, expect, it } from "vitest";

import type { TorrentSummary } from "@/lib/ipc/types";
import { progressFraction, ratio } from "./torrent";

function torrent(over: Partial<TorrentSummary> = {}): TorrentSummary {
  return {
    id: 1,
    infoHash: "a".repeat(40),
    name: "torrent",
    state: "downloading",
    progressBytes: 0,
    totalBytes: 0,
    uploadedBytes: 0,
    downloadBps: 0,
    uploadBps: 0,
    livePeers: 0,
    knownPeers: 0,
    health: "unknown",
    detail: "",
    etaSeconds: null,
    finished: false,
    addedAt: null,
    pauseReason: null,
    error: null,
    outputFolder: "/tmp",
    forced: false,
    queuePosition: null,
    ...over,
  };
}

describe("progressFraction", () => {
  // The four cases `TorrentSummary::progress_fraction` asserts in
  // src-tauri/src/engine/torrent.rs. Change one side, change the other.
  it("is complete for a finished torrent whose metadata has not resolved", () => {
    // The case that made this worth sharing rather than reimplementing: the
    // row drew 100% and the Progress key sorted it as 0%.
    expect(progressFraction(torrent({ finished: true, totalBytes: 0 }))).toBe(
      1,
    );
  });

  it("is nothing for an unfinished torrent of unknown size", () => {
    expect(progressFraction(torrent({ totalBytes: 0 }))).toBe(0);
  });

  it("is the share verified in between", () => {
    expect(
      progressFraction(torrent({ progressBytes: 250, totalBytes: 1000 })),
    ).toBe(0.25);
  });

  it("clamps a counter that reads over-complete", () => {
    expect(
      progressFraction(torrent({ progressBytes: 1200, totalBytes: 1000 })),
    ).toBe(1);
  });
});

describe("ratio", () => {
  it("is nothing when there is no denominator", () => {
    // Not zero. A seed limit must not fire on a torrent that has downloaded
    // nothing, and a sort puts "no ratio" last rather than first.
    expect(ratio(torrent({ progressBytes: 0, uploadedBytes: 500 }))).toBeNull();
  });

  it("divides uploaded by downloaded, matching policy::rules::ratio", () => {
    expect(ratio(torrent({ progressBytes: 1000, uploadedBytes: 1500 }))).toBe(
      1.5,
    );
  });
});
