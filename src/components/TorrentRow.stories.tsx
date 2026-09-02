import type { Meta, StoryObj } from "@storybook/nextjs-vite";

import type { TorrentSummary } from "@/lib/ipc/types";

import { ColumnHeader } from "./ColumnHeader";
import { TorrentRow } from "./TorrentRow";

/**
 * The design's fixture set, trimmed to what a row renders.
 *
 * Deliberately includes the states that are easy to forget: nobody seeding,
 * out of disk mid-piece, re-hashing after a power cut.
 */
function make(over: Partial<TorrentSummary>): TorrentSummary {
  return {
    id: 1,
    infoHash: "a".repeat(40),
    name: "debian-13.2.0-amd64-DVD-1.iso",
    state: "downloading",
    progressBytes: 19_700_000_000,
    totalBytes: 46_100_000_000,
    uploadedBytes: 394_000_000,
    downloadBps: 6_600_000,
    uploadBps: 900_000,
    livePeers: 41,
    knownPeers: 206,
    health: "unknown",
    detail: "1 h 07 min left",
    etaSeconds: 4020,
    finished: false,
    addedAt: null,
    error: null,
    outputFolder: "/Volumes/Media/Linux",
    ...over,
  };
}

const meta = {
  title: "Library/TorrentRow",
  component: TorrentRow,
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component:
          "Seven columns at fixed widths so the numbers line up down the " +
          "list rather than wandering with content. The meta line never " +
          "repeats the state — the icon already says that.",
      },
    },
  },
  args: {
    torrent: make({}),
    selected: false,
    onSelect: () => {},
    onOpen: () => {},
    onContextMenu: () => {},
  },
} satisfies Meta<typeof TorrentRow>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Downloading: Story = {};

export const Selected: Story = { args: { selected: true } };

export const NoSeeds: Story = {
  args: {
    torrent: make({
      name: "OpenStreetMap Planet Dump 2026-08-17",
      progressBytes: 5_090_000_000,
      totalBytes: 84_900_000_000,
      downloadBps: 0,
      uploadBps: 0,
      livePeers: 0,
      knownPeers: 3,
      health: "none",
      detail: "none of the 3 known peers are answering",
      etaSeconds: null,
    }),
  },
};

export const Errored: Story = {
  args: {
    torrent: make({
      name: "Big Buck Bunny 60fps 4K — Blender Foundation",
      state: "error",
      progressBytes: 4_100_000_000,
      totalBytes: 9_310_000_000,
      downloadBps: 0,
      uploadBps: 0,
      livePeers: 0,
      knownPeers: 0,
      health: "idle",
      detail: "stopped — /Volumes/Scratch has 0 B free",
      error: "/Volumes/Scratch has 0 B free",
      etaSeconds: null,
    }),
  },
};

/** Every state under the real column header, which is how it is read. */
export const TheWholeList: Story = {
  render: (args) => (
    <div className="bg-bg-0 flex flex-col">
      <ColumnHeader />
      {(
        [
          {},
          {
            name: "MusicBrainz Database Dump 2026-08-20",
            progressBytes: 7_330_000_000,
            totalBytes: 7_800_000_000,
            downloadBps: 3_100_000,
            uploadBps: 400_000,
            livePeers: 12,
            knownPeers: 44,
            detail: "2 min 30 s left",
          },
          {
            name: "archlinux-2026.08.01-x86_64.iso",
            state: "seeding" as const,
            progressBytes: 1_190_000_000,
            totalBytes: 1_190_000_000,
            downloadBps: 0,
            uploadBps: 600_000,
            livePeers: 9,
            knownPeers: 61,
            health: "seeding" as const,
            detail: "seeding to 9 of 61 peers · ratio 4.82",
            finished: true,
            addedAt: null,
          },
          {
            name: "OpenStreetMap Planet Dump 2026-08-17",
            progressBytes: 5_090_000_000,
            totalBytes: 84_900_000_000,
            downloadBps: 0,
            uploadBps: 0,
            livePeers: 0,
            knownPeers: 3,
            health: "none" as const,
            detail: "none of the 3 known peers are answering",
          },
          {
            name: "Ubuntu 26.04 LTS Desktop amd64",
            state: "paused" as const,
            progressBytes: 4_870_000_000,
            totalBytes: 5_940_000_000,
            downloadBps: 0,
            uploadBps: 0,
            livePeers: 0,
            knownPeers: 0,
            health: "idle" as const,
            detail: "paused — everything downloaded is verified on disk",
          },
          {
            name: "Wikipedia English Dump 2026-08-01 (multistream)",
            state: "checking" as const,
            progressBytes: 9_270_000_000,
            totalBytes: 22_600_000_000,
            downloadBps: 0,
            uploadBps: 0,
            livePeers: 0,
            knownPeers: 0,
            health: "idle" as const,
            detail: "re-checking data already on disk",
          },
          {
            name: "Big Buck Bunny 60fps 4K — Blender Foundation",
            state: "error" as const,
            progressBytes: 4_100_000_000,
            totalBytes: 9_310_000_000,
            downloadBps: 0,
            uploadBps: 0,
            livePeers: 0,
            knownPeers: 0,
            health: "idle" as const,
            detail: "stopped — /Volumes/Scratch has 0 B free",
          },
        ] as Partial<TorrentSummary>[]
      ).map((over, index) => (
        <TorrentRow
          {...args}
          key={index}
          torrent={make({ ...over, id: index, infoHash: String(index) })}
        />
      ))}
    </div>
  ),
};
