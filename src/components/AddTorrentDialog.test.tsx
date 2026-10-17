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
const pickFolder = vi.fn<() => Promise<string | null>>();
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: () => pickFolder() }));
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
  pickFolder.mockReset();
  pickFolder.mockResolvedValue(null);
  previewTorrent.mockReset();
  confirmAdd.mockReset();
  destinationPreflight.mockReset();
  discardPreview.mockReset();
  previewTorrent.mockResolvedValue(PREVIEW);
  confirmAdd.mockResolvedValue(1);
  discardPreview.mockResolvedValue(undefined);
  destinationPreflight.mockResolvedValue({
    exists: true,
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

  it("clears a remembered override when the add uses the session folder", async () => {
    // Returning to the default has to *remove* the override, not store it as
    // another one. Reporting only paths left the old one behind: the next add
    // collapsed it to `null` without clearing it, so changing the global
    // folder later revived a destination the user had moved away from.
    previewTorrent.mockResolvedValue({
      ...PREVIEW,
      // Gone, so the remembered folder is dropped and the add falls back to
      // the session folder — the same end state as choosing it by hand.
      savePath: PREVIEW.savePath,
    });
    destinationPreflight.mockResolvedValue({
      exists: false,
      freeBytes: null,
      alreadyOnDisk: [false],
    });
    const onFolderUsed = vi.fn();

    render(
      <AddTorrentDialog
        droppedPath="/tmp/x.torrent"
        defaultFolder="/Volumes/Gone"
        onFolderUsed={onFolderUsed}
        onClose={() => {}}
      />,
    );

    await waitFor(() =>
      expect(screen.getByText(/not there any more/)).toBeDefined(),
    );

    fireEvent.click(screen.getByRole("button", { name: /^Add 1 file/ }));

    await waitFor(() =>
      expect(confirmAdd).toHaveBeenCalledWith(PREVIEW.infoHash, null, null),
    );
    expect(onFolderUsed).toHaveBeenCalledWith(null);
  });

  it("does not write settings when the destination is unchanged", async () => {
    // An ordinary add, with nothing remembered and nothing chosen, has
    // nothing to say about destinations.
    const onFolderUsed = vi.fn();

    render(
      <AddTorrentDialog
        droppedPath="/tmp/x.torrent"
        onFolderUsed={onFolderUsed}
        onClose={() => {}}
      />,
    );

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /^Add 1 file/ })).toBeDefined(),
    );
    fireEvent.click(screen.getByRole("button", { name: /^Add 1 file/ }));

    await waitFor(() => expect(confirmAdd).toHaveBeenCalled());
    expect(onFolderUsed).not.toHaveBeenCalled();
  });

  it("discards a check that was still running when the default came back", async () => {
    // Going back to the session folder leaves any in-flight check about
    // somewhere else. Without marking it obsolete the older reply still
    // matched its guard and installed the custom folder's free space and
    // on-disk marks beside a destination that had already changed.
    let release: (value: DestinationPreflight) => void = () => {};
    destinationPreflight.mockImplementation(
      () =>
        new Promise<DestinationPreflight>((resolve) => {
          release = resolve;
        }),
    );
    pickFolder.mockResolvedValue(PREVIEW.savePath);

    render(
      <AddTorrentDialog
        droppedPath="/tmp/x.torrent"
        defaultFolder="/Volumes/Media"
        onClose={() => {}}
      />,
    );

    // The adopted folder's check is hanging.
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Checking that folder…" }),
      ).toBeDefined(),
    );

    // Back to the default while it is still in flight.
    fireEvent.click(screen.getByRole("button", { name: "Change" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /^Add 1 file/ })).toBeDefined(),
    );

    // The stale reply lands. It must not be installed.
    release({ exists: true, freeBytes: 42, alreadyOnDisk: [true] });

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /^Add 1 file/ })).toBeDefined(),
    );
    fireEvent.click(screen.getByRole("button", { name: /^Add 1 file/ }));
    await waitFor(() =>
      expect(confirmAdd).toHaveBeenCalledWith(PREVIEW.infoHash, null, null),
    );
  });

  it("will not send a remembered folder that is no longer there", async () => {
    // The remembered folder can be on a volume that has since been
    // unmounted. The engine would create the path again on the local disk
    // and fill it, while the free-space figure described that disk rather
    // than the one the user meant — so the add silently lands on the wrong
    // volume. Falling back to the session folder is the only answer that
    // cannot write somewhere surprising.
    destinationPreflight.mockResolvedValue({
      exists: false,
      freeBytes: 500_000_000,
      alreadyOnDisk: [false],
    });

    render(
      <AddTorrentDialog
        droppedPath="/tmp/x.torrent"
        defaultFolder="/Volumes/Gone"
        onClose={() => {}}
      />,
    );

    await waitFor(() => expect(destinationPreflight).toHaveBeenCalled());
    await waitFor(() =>
      expect(screen.getByText(/not there any more/)).toBeDefined(),
    );
    // The session's own folder, which is what `null` asks the engine for.
    expect(screen.getByText(PREVIEW.savePath)).toBeDefined();

    fireEvent.click(screen.getByRole("button", { name: /^Add 1 file/ }));

    await waitFor(() =>
      expect(confirmAdd).toHaveBeenCalledWith(PREVIEW.infoHash, null, null),
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

  it("neither offers nor remembers a destination for a torrent already added", async () => {
    // Adding an existing torrent replaces its file selection and leaves its
    // data where it is, so the folder is ignored. Remembering it anyway
    // would default the *next* add to a folder nothing was written to — and
    // `savePath` here describes where a new add would go, not where this
    // torrent's files actually are, so showing it would assert a wrong path.
    previewTorrent.mockResolvedValue({ ...PREVIEW, alreadyAdded: true });
    const onFolderUsed = vi.fn();

    render(
      <AddTorrentDialog
        droppedPath="/tmp/x.torrent"
        defaultFolder="/Volumes/Media"
        onFolderUsed={onFolderUsed}
        onClose={() => {}}
      />,
    );

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /^Add 1 file/ })).toBeDefined(),
    );
    expect(screen.queryByText("Saving to")).toBeNull();
    expect(destinationPreflight).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /^Add 1 file/ }));

    await waitFor(() =>
      expect(confirmAdd).toHaveBeenCalledWith(PREVIEW.infoHash, null, null),
    );
    expect(onFolderUsed).not.toHaveBeenCalled();
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

    release({ exists: true, freeBytes: 1, alreadyOnDisk: [false] });

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /^Add 1 file/ })).toBeDefined(),
    );
  });
});
