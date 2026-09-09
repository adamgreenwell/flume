import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { SeedLimits, TorrentRules } from "@/lib/ipc/types";

const getSeedLimits = vi.fn<(hash: string) => Promise<SeedLimits>>();
const setTorrentRules =
  vi.fn<(hash: string, rules: TorrentRules | null) => Promise<void>>();

vi.mock("@/lib/ipc/client", () => ({
  getSeedLimits: (hash: string) => getSeedLimits(hash),
  setTorrentRules: (hash: string, rules: TorrentRules | null) =>
    setTorrentRules(hash, rules),
}));

const { SeedLimitsPanel } = await import("./SeedLimitsPanel");

const GLOBAL: TorrentRules = {
  seedRatioLimit: 2,
  seedTimeLimitSecs: 86_400,
};

function panel(limits: SeedLimits) {
  getSeedLimits.mockResolvedValue(limits);
  return render(<SeedLimitsPanel infoHash={"a".repeat(40)} />);
}

beforeEach(() => {
  getSeedLimits.mockReset();
  setTorrentRules.mockReset();
  setTorrentRules.mockResolvedValue(undefined);
});

describe("a torrent following the global limits", () => {
  it("says what those limits actually are", async () => {
    // "Follows your global limits" alone tells the user nothing they can act
    // on — the point of showing the globals here is that the sentence is
    // checkable without opening Settings.
    panel({ torrent: null, global: GLOBAL });

    await waitFor(() =>
      expect(
        screen.getByText(
          /stop at a ratio of 2\.00 or after .* whichever comes first/,
        ),
      ).toBeTruthy(),
    );
  });

  it("seeds an override from the globals rather than from nothing", async () => {
    // Switching to "just this torrent" must not silently lift every limit the
    // torrent already had.
    panel({ torrent: null, global: GLOBAL });
    await waitFor(() => screen.getByText("Just this torrent"));

    screen.getByText("Just this torrent").click();

    await waitFor(() =>
      expect(setTorrentRules).toHaveBeenCalledWith("a".repeat(40), GLOBAL),
    );
  });
});

describe("a torrent with its own limits", () => {
  it("describes them as a departure from the globals", async () => {
    panel({
      torrent: { seedRatioLimit: 5, seedTimeLimitSecs: null },
      global: GLOBAL,
    });

    await waitFor(() =>
      expect(
        screen.getByText(/stop at a ratio of 5\.00, whatever the global/),
      ).toBeTruthy(),
    );
  });

  it("expresses seed-forever rather than treating it as no override", async () => {
    // The distinction the wholesale-replacement rule exists for: an override
    // with both limits cleared is "seed this one forever", not "follow the
    // globals".
    panel({
      torrent: { seedRatioLimit: null, seedTimeLimitSecs: null },
      global: GLOBAL,
    });

    await waitFor(() =>
      expect(screen.getByText(/seed with no limit/)).toBeTruthy(),
    );
    expect(screen.queryByText(/follows your global limits/)).toBeNull();
  });

  it("returns the torrent to the globals when asked", async () => {
    panel({
      torrent: { seedRatioLimit: 5, seedTimeLimitSecs: null },
      global: GLOBAL,
    });
    await waitFor(() => screen.getByText("Follow global"));

    screen.getByText("Follow global").click();

    await waitFor(() =>
      expect(setTorrentRules).toHaveBeenCalledWith("a".repeat(40), null),
    );
  });

  it("marks which of the two states is current", async () => {
    // The pair is a radiogroup, so exactly one has to read as chosen.
    panel({
      torrent: { seedRatioLimit: 5, seedTimeLimitSecs: null },
      global: GLOBAL,
    });

    await waitFor(() => screen.getByText("Just this torrent"));
    const chosen = screen
      .getAllByRole("radio")
      .filter((b) => b.getAttribute("aria-checked") === "true");

    expect(chosen).toHaveLength(1);
    expect(chosen[0].textContent).toBe("Just this torrent");
  });
});

describe("when the limits cannot be read", () => {
  it("says so rather than showing limits that are not real", async () => {
    getSeedLimits.mockRejectedValue(new Error("nope"));
    render(<SeedLimitsPanel infoHash={"a".repeat(40)} />);

    await waitFor(() =>
      expect(screen.getByRole("alert").textContent).toMatch(
        /Could not read this torrent's seed limits/,
      ),
    );
    expect(screen.queryByRole("radio")).toBeNull();
  });
});
