import type { Meta, StoryObj } from "@storybook/nextjs-vite";

import { PieceStrip } from "./PieceStrip";

const BUCKETS = 320;

/** Completion: a solid head, a ragged working edge, then untouched. */
const buckets = Array.from({ length: BUCKETS }, (_, i) => {
  if (i < 150) return 255;
  if (i < 175) return (i * 37) % 255;
  return 0;
});

const meta = {
  title: "Library/PieceStrip",
  component: PieceStrip,
  parameters: {
    docs: {
      description: {
        component:
          "Two stacked strips sharing one bucketing: completion above, and " +
          "how many peers hold each region below. The lower strip is what " +
          "reveals a torrent about to stall, so both are resampled to the " +
          "same cell count — and the availability strip resamples by " +
          "*minimum*, because a region containing one piece nobody holds is " +
          "precisely what an average would hide.",
      },
    },
  },
  args: {
    pieces: {
      totalPieces: 23_280,
      piecesComplete: 150 * 73 + Math.round(25 * 73 * 0.5),
      piecesPerBucket: 73,
      buckets,
      availability: Array.from({ length: BUCKETS }, (_, i) =>
        i < 150 ? 8 + (i % 3) : i < 260 ? 4 + (i % 3) : 2 + (i % 2),
      ),
    },
  },
} satisfies Meta<typeof PieceStrip>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Deep at the head, thinning towards the tail. */
export const Healthy: Story = {};

/**
 * The case the strip exists for: a stretch no connected peer holds.
 *
 * Those bars are drawn full height in the error colour rather than as zero
 * height — it is the most important thing on the strip, and a zero-height bar
 * would be the least visible thing on it. The caption changes too, so the
 * warning is never colour alone.
 */
export const RegionNobodyHolds: Story = {
  args: {
    pieces: {
      totalPieces: 23_280,
      piecesComplete: 150 * 73,
      piecesPerBucket: 73,
      buckets,
      availability: Array.from({ length: BUCKETS }, (_, i) =>
        i > 200 && i < 240 ? 0 : 6 + (i % 3),
      ),
    },
  },
};

/**
 * No peer bitfields yet, so there is nothing to judge from and the lower strip
 * is absent entirely rather than drawn empty.
 */
export const AvailabilityUnknown: Story = {
  args: {
    pieces: {
      totalPieces: 23_280,
      piecesComplete: 150 * 73,
      piecesPerBucket: 73,
      buckets,
      availability: null,
    },
  },
};
