import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { PieceMap } from "@/lib/ipc/types";

import { PieceStrip } from "./PieceStrip";

/**
 * Rendering rules for the availability strip.
 *
 * The unit tests for `resample`/`resampleRarest` live in `PieceStrip.test.ts`;
 * this covers when the lower strip is drawn at all, which is a judgement about
 * whether the question it answers still applies.
 */
function map(overrides: Partial<PieceMap> = {}): PieceMap {
  return {
    totalPieces: 100,
    piecesComplete: 40,
    piecesPerBucket: 1,
    buckets: Array.from({ length: 100 }, () => 128),
    availability: Array.from({ length: 100 }, () => 5),
    ...overrides,
  };
}

const strip = () =>
  screen
    .queryAllByRole("img")
    .find((el) => /Availability/.test(el.getAttribute("aria-label") ?? ""));

describe("PieceStrip availability", () => {
  it("draws the strip while a torrent is still downloading", () => {
    render(<PieceStrip pieces={map()} />);
    expect(strip()).toBeDefined();
  });

  it("warns when a region is held by nobody", () => {
    const availability = Array.from({ length: 100 }, (_, i) =>
      i > 40 && i < 60 ? 0 : 5,
    );
    render(<PieceStrip pieces={map({ availability })} />);
    expect(screen.getByText(/no peer holds some regions/)).toBeDefined();
  });

  /**
   * The false alarm this guards against.
   *
   * Availability counts connected peers and excludes our own bitfield, so a
   * completed torrent seeding to leechers computes zero copies everywhere. Drawn
   * literally that is a full-width red strip warning of a stall on a torrent
   * that has already finished.
   */
  it("draws nothing once the torrent is complete, however starved the peers look", () => {
    render(
      <PieceStrip
        pieces={map({
          piecesComplete: 100,
          availability: Array.from({ length: 100 }, () => 0),
        })}
      />,
    );

    expect(strip()).toBeUndefined();
    expect(screen.queryByText(/no peer holds some regions/)).toBeNull();
    // The completion strip and its own caption stay.
    expect(screen.getByText(/100 of 100 pieces/)).toBeDefined();
  });

  it("draws nothing when there are no bitfields to judge from", () => {
    render(<PieceStrip pieces={map({ availability: null })} />);
    expect(strip()).toBeUndefined();
  });
});
