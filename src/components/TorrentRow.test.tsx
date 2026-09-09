import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { PauseReason, TorrentSummary } from "@/lib/ipc/types";

import { TorrentRow } from "./TorrentRow";

/**
 * Tests for the one thing the row has to say that its icon cannot.
 *
 * A torrent Flume stopped and a torrent the user paused are both `"paused"`,
 * draw the same glyph in the same tone, and are both correctly inert. The
 * distinction lives entirely in words — which is what #55 asks for, and what
 * would silently regress if the row ever went back to keying off state alone.
 */
function torrent(over: Partial<TorrentSummary> = {}): TorrentSummary {
  return {
    id: 1,
    infoHash: "a".repeat(40),
    name: "debian-13.2.0-amd64-DVD-1.iso",
    state: "paused",
    progressBytes: 1_000,
    totalBytes: 1_000,
    uploadedBytes: 4_000,
    downloadBps: 0,
    uploadBps: 0,
    livePeers: 0,
    knownPeers: 0,
    health: "idle",
    detail: "paused — everything downloaded is verified on disk",
    etaSeconds: null,
    finished: true,
    addedAt: null,
    pauseReason: null,
    error: null,
    outputFolder: "/tmp",
    ...over,
  };
}

function row(over: Partial<TorrentSummary> = {}) {
  return render(
    <TorrentRow
      torrent={torrent(over)}
      selected={false}
      onSelect={() => {}}
      onOpen={() => {}}
      onContextMenu={() => {}}
    />,
  );
}

describe("a torrent stopped by a rule", () => {
  it("is not announced the same way as one the user paused", () => {
    // The glyph is hidden, so this label is the whole of what a screen reader
    // gets from the status cell. "Paused" for both would lose the difference
    // entirely for anyone not reading the meta line.
    const reasons: PauseReason[] = [
      "ratioReached",
      "seedTimeReached",
      "queued",
    ];

    // Unmounted before the loop: `screen` queries the whole document, so a
    // leftover render would keep answering for every case after it.
    const plain = row();
    expect(screen.getByText("Paused")).toBeTruthy();
    plain.unmount();

    for (const pauseReason of reasons) {
      const { unmount } = row({ pauseReason });
      expect(
        screen.queryByText("Paused"),
        `${pauseReason} was announced as a plain pause`,
      ).toBeNull();
      unmount();
    }
  });

  it("gives each reason its own wording", () => {
    // Raising a ratio, raising a time and waiting for a slot are three
    // different responses, so none may collapse into another's label.
    const labels = (["ratioReached", "seedTimeReached", "queued"] as const).map(
      (pauseReason) => {
        const { container, unmount } = row({ pauseReason });
        const text = container.querySelector(".sr-only")?.textContent ?? "";
        unmount();
        return text;
      },
    );

    expect(new Set(labels).size).toBe(labels.length);
    expect(labels.every((l) => l.length > 0)).toBe(true);
  });

  it("still shows the sentence the backend wrote", () => {
    // The row renders the engine's detail line verbatim rather than deriving
    // its own copy — one place decides the wording, and it is the side that
    // knows the numbers.
    row({
      pauseReason: "ratioReached",
      detail:
        "stopped at your seed ratio limit — everything is verified on disk",
    });

    expect(
      screen.getByText(
        "stopped at your seed ratio limit — everything is verified on disk",
      ),
    ).toBeTruthy();
  });
});

describe("a queued torrent", () => {
  it("is announced as waiting rather than as paused", () => {
    // The distinction #56 exists to make: a paused torrent stays put, a
    // queued one starts itself. A screen reader gets only this label.
    const plain = row({ state: "paused" });
    expect(screen.getByText("Paused")).toBeTruthy();
    plain.unmount();

    row({ state: "queued", pauseReason: "queued" });

    expect(screen.queryByText("Paused")).toBeNull();
    expect(screen.getByText(/Queued, waiting for a slot/)).toBeTruthy();
  });

  it("does not draw the pause glyph", () => {
    // Queued shares its tone with paused, deliberately — neither is a
    // condition — so the glyph is the only thing separating them by sight.
    // Compared by path data because that is what actually differs; the icons
    // carry no test hook, and adding one to assert on would be testing the
    // hook rather than the drawing.
    const glyph = (over: Partial<TorrentSummary>) => {
      const { container, unmount } = row(over);
      const d = container.querySelector("svg path")?.getAttribute("d") ?? null;
      unmount();
      return d;
    };

    const pause = glyph({ state: "paused" });
    const queued = glyph({ state: "queued", pauseReason: "queued" });

    expect(pause).toBeTruthy();
    expect(queued).toBeTruthy();
    expect(queued).not.toBe(pause);
  });
});
