import { describe, expect, it } from "vitest";

import type { SortState, TorrentSummary } from "@/lib/ipc/types";
import { SORTS, defaultDirectionFor, sortTorrents } from "./sort";

function torrent(over: Partial<TorrentSummary> = {}): TorrentSummary {
  return {
    id: 1,
    infoHash: "a".repeat(40),
    name: "torrent",
    state: "downloading",
    progressBytes: 500,
    totalBytes: 1000,
    uploadedBytes: 250,
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

function order(torrents: readonly TorrentSummary[], sort: SortState): number[] {
  return sortTorrents(torrents, sort).map((t) => t.id);
}

describe("the order is total", () => {
  it("never leaves two rows comparing equal", () => {
    // The property that matters most. Rows update every second and the list
    // re-sorts each tick, so a comparison that can return 0 lets equal rows
    // swap places continuously and makes clicking a moving target.
    const same = [3, 1, 2].map((id) =>
      torrent({ id, name: "identical", totalBytes: 1000 }),
    );

    for (const by of SORTS.map((s) => s.by)) {
      for (const direction of ["asc", "desc"] as const) {
        expect(order(same, { by, direction })).toEqual([1, 2, 3]);
      }
    }
  });

  it("breaks a size tie by id rather than by arrival order", () => {
    // Byte-identical sizes are common — several files from one source often
    // match exactly — and `size` is the key most likely to tie.
    const rows = [
      torrent({ id: 9, totalBytes: 1000 }),
      torrent({ id: 2, totalBytes: 1000 }),
      torrent({ id: 5, totalBytes: 1000 }),
    ];

    expect(order(rows, { by: "size", direction: "desc" })).toEqual([2, 5, 9]);
  });

  it("does not depend on the order it was given", () => {
    const rows = [
      torrent({ id: 1, totalBytes: 300 }),
      torrent({ id: 2, totalBytes: 100 }),
      torrent({ id: 3, totalBytes: 200 }),
    ];
    const sort: SortState = { by: "size", direction: "asc" };

    expect(order(rows, sort)).toEqual(order([...rows].reverse(), sort));
  });
});

describe("rows with no value", () => {
  it("keeps them last whichever way the arrow points", () => {
    // "No estimate" is not "finishes soonest". A user flipping the arrow is
    // asking about the rows that have a value, and marching the empty ones
    // to the top answers a question nobody asked.
    const rows = [
      torrent({ id: 1, etaSeconds: null }),
      torrent({ id: 2, etaSeconds: 60 }),
      torrent({ id: 3, etaSeconds: 30 }),
    ];

    expect(order(rows, { by: "eta", direction: "asc" })).toEqual([3, 2, 1]);
    expect(order(rows, { by: "eta", direction: "desc" })).toEqual([2, 3, 1]);
  });

  it("treats a torrent that has downloaded nothing as having no ratio", () => {
    const rows = [
      torrent({ id: 1, progressBytes: 0, uploadedBytes: 0 }),
      torrent({ id: 2, progressBytes: 100, uploadedBytes: 50 }),
    ];

    expect(order(rows, { by: "ratio", direction: "asc" })).toEqual([2, 1]);
    expect(order(rows, { by: "ratio", direction: "desc" })).toEqual([2, 1]);
  });

  it("keeps a torrent with no arrival time last", () => {
    // It predates the library record. Guessing where it belongs would invent
    // an order, which is why the field is null rather than backfilled.
    const rows = [
      torrent({ id: 1, addedAt: null }),
      torrent({ id: 2, addedAt: 1_700_000_000 }),
    ];

    expect(order(rows, { by: "added", direction: "desc" })).toEqual([2, 1]);
    expect(order(rows, { by: "added", direction: "asc" })).toEqual([2, 1]);
  });
});

describe("the keys", () => {
  it("sorts progress by share complete, not by bytes", () => {
    // A nearly-finished small torrent above a barely-started large one.
    // Sorting by bytes downloaded is what the Size column is for.
    const rows = [
      torrent({ id: 1, progressBytes: 900, totalBytes: 1_000_000 }),
      torrent({ id: 2, progressBytes: 90, totalBytes: 100 }),
    ];

    expect(order(rows, { by: "progress", direction: "desc" })).toEqual([2, 1]);
  });

  it("sorts a finished torrent as complete before its metadata resolves", () => {
    // The row draws 100% for this — `progressFraction` treats finished as
    // complete whatever `totalBytes` says — so the Progress key has to agree.
    // Computing its own fraction here put a visibly complete row below
    // incomplete ones.
    const rows = [
      torrent({ id: 1, progressBytes: 500, totalBytes: 1000 }),
      torrent({ id: 2, finished: true, progressBytes: 0, totalBytes: 0 }),
    ];

    expect(order(rows, { by: "progress", direction: "desc" })).toEqual([2, 1]);
  });

  it("sorts an over-complete counter as complete, not as more than complete", () => {
    const rows = [
      torrent({ id: 1, progressBytes: 1200, totalBytes: 1000 }),
      torrent({ id: 2, finished: true, progressBytes: 1000, totalBytes: 1000 }),
    ];

    // Both are 1, so the tie-break decides and neither outranks the other.
    expect(order(rows, { by: "progress", direction: "desc" })).toEqual([1, 2]);
  });

  it("sorts a zero-byte torrent's progress as nothing rather than dividing by zero", () => {
    const rows = [
      torrent({ id: 1, progressBytes: 0, totalBytes: 0 }),
      torrent({ id: 2, progressBytes: 1, totalBytes: 100 }),
    ];

    expect(order(rows, { by: "progress", direction: "desc" })).toEqual([2, 1]);
  });

  it("sorts activity by the two rates together", () => {
    const rows = [
      torrent({ id: 1, downloadBps: 100, uploadBps: 0 }),
      torrent({ id: 2, downloadBps: 10, uploadBps: 200 }),
    ];

    expect(order(rows, { by: "activity", direction: "desc" })).toEqual([2, 1]);
  });

  it("falls back to the name when an idle list is sorted by activity", () => {
    // Every row is at zero on an idle list, and alphabetical reads better
    // there than arrival order does.
    const rows = [
      torrent({ id: 1, name: "zebra" }),
      torrent({ id: 2, name: "apple" }),
    ];

    expect(order(rows, { by: "activity", direction: "desc" })).toEqual([2, 1]);
  });

  it("sorts names as text, both ways", () => {
    const rows = [
      torrent({ id: 1, name: "beta" }),
      torrent({ id: 2, name: "alpha" }),
    ];

    expect(order(rows, { by: "name", direction: "asc" })).toEqual([2, 1]);
    expect(order(rows, { by: "name", direction: "desc" })).toEqual([1, 2]);
  });

  it("groups states active-first ascending", () => {
    const rows = [
      torrent({ id: 1, state: "error" }),
      torrent({ id: 2, state: "downloading" }),
      torrent({ id: 3, state: "paused" }),
      torrent({ id: 4, state: "seeding" }),
    ];

    expect(order(rows, { by: "state", direction: "asc" })).toEqual([
      2, 4, 3, 1,
    ]);
  });
});

describe("the direction a key starts in", () => {
  it("starts numbers largest-first and names A to Z", () => {
    // Picking "Size" and getting the smallest torrent is a small, constant
    // annoyance.
    expect(defaultDirectionFor("size")).toBe("desc");
    expect(defaultDirectionFor("activity")).toBe("desc");
    expect(defaultDirectionFor("name")).toBe("asc");
    expect(defaultDirectionFor("eta")).toBe("asc");
  });
});

describe("the input", () => {
  it("is not mutated", () => {
    // `torrents` is telemetry state; sorting it in place would mutate what
    // the next tick diffs against.
    const rows = [torrent({ id: 2 }), torrent({ id: 1 })];
    const before = rows.map((t) => t.id);

    sortTorrents(rows, { by: "name", direction: "asc" });

    expect(rows.map((t) => t.id)).toEqual(before);
  });
});
