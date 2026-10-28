import { render, screen, waitFor, within } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";

import { STATISTICS_EVENT } from "@/hooks/useStatistics";
import type { Statistics } from "@/lib/ipc/types";

import {
  StatisticsDialog,
  StatisticsTable,
  shareRatio,
} from "./StatisticsDialog";

const STATS: Statistics = {
  session: {
    downloadedBytes: 2_500_000_000,
    uploadedBytes: 1_250_000_000,
    activeSeconds: 4_020,
  },
  allTime: {
    downloadedBytes: 1_200_000_000_000,
    uploadedBytes: 3_000_000_000_000,
    activeSeconds: 360_000,
  },
  launches: 42,
};

afterEach(() => {
  clearMocks();
});

/** The two figures in the row labelled `label`: this session, then all time. */
function row(label: string): [string, string] {
  const header = screen.getByRole("rowheader", { name: label });
  const cells = within(header.closest("tr") as HTMLElement).getAllByRole(
    "cell",
  );
  return [cells[0].textContent ?? "", cells[1].textContent ?? ""];
}

describe("shareRatio", () => {
  it("divides upload by download", () => {
    expect(shareRatio(STATS.allTime)).toBe("2.50");
  });

  // Nothing downloaded is a real answer over a span this long, and a zero
  // would claim a measurement.
  it("is a dash when nothing has been downloaded", () => {
    expect(
      shareRatio({ downloadedBytes: 0, uploadedBytes: 9, activeSeconds: 1 }),
    ).toBe("—");
  });
});

describe("StatisticsTable", () => {
  it("shows this session beside all time", () => {
    render(<StatisticsTable statistics={STATS} />);

    expect(row("Downloaded")).toEqual(["2.50 GB", "1.20 TB"]);
    expect(row("Uploaded")).toEqual(["1.25 GB", "3.00 TB"]);
    expect(row("Share ratio")).toEqual(["0.50", "2.50"]);
    expect(row("Time open")).toEqual(["1 h 07 min", "100 h 00 min"]);
    expect(screen.getByText(/Launched/).textContent).toContain(
      "Launched 42 times.",
    );
  });

  it("says one time, not one times", () => {
    render(<StatisticsTable statistics={{ ...STATS, launches: 1 }} />);
    expect(screen.getByText(/Launched/).textContent).toContain(
      "Launched 1 time.",
    );
  });

  it("shows dashes until the totals arrive", () => {
    render(<StatisticsTable statistics={null} />);
    expect(row("Downloaded")).toEqual(["—", "—"]);
    expect(screen.queryByText("Reading totals…")).not.toBeNull();
  });
});

describe("StatisticsDialog", () => {
  it("renders what get_statistics returns, then follows pushed totals", async () => {
    mockIPC(
      (cmd) => {
        if (cmd === "get_statistics") return STATS;
        throw new Error(`unexpected command: ${cmd}`);
      },
      { shouldMockEvents: true },
    );

    render(<StatisticsDialog onClose={() => {}} />);
    await waitFor(() => expect(row("Downloaded")[0]).toBe("2.50 GB"));

    await emit(STATISTICS_EVENT, {
      ...STATS,
      session: { ...STATS.session, downloadedBytes: 3_000_000_000 },
    });
    await waitFor(() => expect(row("Downloaded")[0]).toBe("3.00 GB"));
  });

  // The slower initial fetch must not overwrite a pushed value that landed
  // first, or the view would step backwards a second after opening.
  it("keeps a pushed value over an initial fetch that answers later", async () => {
    let answer: (value: Statistics) => void = () => {};
    mockIPC(
      (cmd) => {
        if (cmd === "get_statistics") {
          return new Promise<Statistics>((resolve) => {
            answer = resolve;
          });
        }
        throw new Error(`unexpected command: ${cmd}`);
      },
      { shouldMockEvents: true },
    );

    render(<StatisticsDialog onClose={() => {}} />);
    // Give the subscription a moment to exist before emitting into it.
    await new Promise((resolve) => setTimeout(resolve, 20));
    await emit(STATISTICS_EVENT, {
      ...STATS,
      session: { ...STATS.session, downloadedBytes: 9_000_000_000 },
    });
    await waitFor(() => expect(row("Downloaded")[0]).toBe("9.00 GB"));

    answer(STATS);
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(row("Downloaded")[0]).toBe("9.00 GB");
  });
});
