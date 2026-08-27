import type { Meta, StoryObj } from "@storybook/nextjs-vite";

import { PreflightTiles } from "./PreflightTiles";

const meta = {
  title: "Add/PreflightTiles",
  component: PreflightTiles,
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component:
          "The four facts worth knowing before agreeing to a download. " +
          "Three recalculate as files are toggled; the swarm tile reports " +
          "what was measured while the metadata was fetched. Every tile " +
          "states its own basis underneath — a number whose derivation is " +
          "invisible is a number the user has to take on trust.",
      },
    },
  },
  args: {
    seenPeers: 6,
    selectedBytes: 55_904_022_200,
    totalBytes: 62_304_022_200,
    selectedCount: 10,
    totalCount: 11,
    freeBytes: 1_420_000_000_000,
    rateBps: 5_500_000,
  },
} satisfies Meta<typeof PreflightTiles>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Comfortable: Story = {};

/** Everything chosen: the note says so in files, not in bytes. */
export const NothingSkipped: Story = {
  args: { selectedBytes: 62_304_022_200, selectedCount: 11 },
};

/** The case the tile exists to catch, before 62 GB starts arriving. */
export const WillNotFit: Story = {
  args: { freeBytes: 30_000_000_000 },
};

/** Fits, but only just. Worth seeing coming rather than discovering. */
export const Tight: Story = {
  args: { freeBytes: 60_000_000_000 },
};

/** A volume that will not report its free space says so, rather than zero. */
export const FreeSpaceUnknown: Story = {
  args: { freeBytes: null },
};

/** A fresh session has no transfer to estimate from, and admits it. */
export const NoRateYet: Story = {
  args: { rateBps: null },
};

/** Metadata read from a .torrent file rather than fetched from peers. */
export const FromAFile: Story = {
  args: { seenPeers: 0 },
};
