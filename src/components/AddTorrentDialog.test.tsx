import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DestinationPreflight, TorrentPreview } from "@/lib/ipc/types";

const previewTorrent = vi.fn<() => Promise<TorrentPreview>>();
const confirmAdd =
  vi.fn<
    (
      hash: string,
      only: number[] | null,
      folder: string | null,
    ) => Promise<number>
  >();
const destinationPreflight =
  vi.fn<(hash: string, dir: string) => Promise<DestinationPreflight>>();
const discardPreview = vi.fn<() => Promise<void>>();

vi.mock("@/lib/ipc/client", () => ({
  previewTorrent: () => previewTorrent(),
  confirmAdd: (hash: string, only: number[] | null, folder: string | null) =>
    confirmAdd(hash, only, folder),
  destinationPreflight: (hash: string, dir: string) =>
    destinationPreflight(hash, dir),
  discardPreview: () => discardPreview(),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  readText: vi.fn(async () => ""),
}));

const { AddTorrentDialog } = await import("./AddTorrentDialog");

const PREVIEW: TorrentPreview = {
  infoHash: "a".repeat(40),
  name: "debian-13.2.0-amd64-DVD-1.iso",
  totalBytes: 1000,
  files: [{ index: 0, path: "debian.iso", length: 1000 }],
  alreadyAdded: false,
  savePath: "/Users/you/Downloads",
  freeBytes: 500_000,
  seenPeers: 4,
  alreadyOnDisk: [false],
};

beforeEach(() => {
  previewTorrent.mockReset();
  confirmAdd.mockReset();
  destinationPreflight.mockReset();
  discardPreview.mockReset();
  previewTorrent.mockResolvedValue(PREVIEW);
  confirmAdd.mockResolvedValue(1);
  discardPreview.mockResolvedValue(undefined);
  destinationPreflight.mockResolvedValue({
    freeBytes: 9_000_000,
    alreadyOnDisk: [false],
  });
});

describe("a remembered destination", () => {
  it("is the folder the add actually uses, not just the one displayed", async () => {
    // The bug this covers: the sheet showed the remembered folder and sent
    // `null`, so librqbit wrote to the global download directory instead.
    // Display, check and destination have to be the same path.
    render(
      <AddTorrentDialog
        droppedPath="/tmp/x.torrent"
        defaultFolder="/Volumes/Media"
        onClose={() => {}}
      />,
    );

    await waitFor(() =>
      expect(screen.getByText("/Volumes/Media")).toBeDefined(),
    );
    // Checked against the remembered folder, not the session default.
    await waitFor(() =>
      expect(destinationPreflight).toHaveBeenCalledWith(
        PREVIEW.infoHash,
        "/Volumes/Media",
      ),
    );

    fireEvent.click(screen.getByRole("button", { name: /^Add 1 file/ }));

    await waitFor(() =>
      expect(confirmAdd).toHaveBeenCalledWith(
        PREVIEW.infoHash,
        null,
        "/Volumes/Media",
      ),
    );
  });

  it("treats a trailing separator as the same folder", async () => {
    // librqbit returns the resolved folder with a trailing separator, which
    // the engine test documents. Comparing raw strings made every untouched
    // add look like a custom one: it persisted the global folder as the
    // remembered one, and then kept writing there after the global setting
    // changed — the exact thing `null` is supposed to prevent.
    previewTorrent.mockResolvedValue({
      ...PREVIEW,
      savePath: `${PREVIEW.savePath}/`,
    });

    render(
      <AddTorrentDialog
        droppedPath="/tmp/x.torrent"
        defaultFolder={PREVIEW.savePath}
        onClose={() => {}}
      />,
    );

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /^Add 1 file/ })).toBeDefined(),
    );
    expect(destinationPreflight).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /^Add 1 file/ }));

    await waitFor(() =>
      expect(confirmAdd).toHaveBeenCalledWith(PREVIEW.infoHash, null, null),
    );
  });

  it("stays null when it matches where the session would put it anyway", async () => {
    // Equal values keep the engine's own default as the single source of
    // that path, rather than freezing a copy of it in every add.
    render(
      <AddTorrentDialog
        droppedPath="/tmp/x.torrent"
        defaultFolder={PREVIEW.savePath}
        onClose={() => {}}
      />,
    );

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /^Add 1 file/ })).toBeDefined(),
    );
    expect(destinationPreflight).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /^Add 1 file/ }));

    await waitFor(() =>
      expect(confirmAdd).toHaveBeenCalledWith(PREVIEW.infoHash, null, null),
    );
  });

  it("will not let the add start while the folder is still being checked", async () => {
    // Confirming mid-check would start the torrent against the previous
    // folder's free space, which is the decision the sheet exists to inform.
    let release: (value: DestinationPreflight) => void = () => {};
    destinationPreflight.mockImplementation(
      () =>
        new Promise<DestinationPreflight>((resolve) => {
          release = resolve;
        }),
    );

    render(
      <AddTorrentDialog
        droppedPath="/tmp/x.torrent"
        defaultFolder="/Volumes/Media"
        onClose={() => {}}
      />,
    );

    const pending = await waitFor(() =>
      screen.getByRole("button", { name: "Checking that folder…" }),
    );
    expect(pending.hasAttribute("disabled")).toBe(true);

    release({ freeBytes: 1, alreadyOnDisk: [false] });

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /^Add 1 file/ })).toBeDefined(),
    );
  });
});
