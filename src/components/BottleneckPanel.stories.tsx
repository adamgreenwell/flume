import type { Meta, StoryObj } from "@storybook/nextjs-vite";

import { BottleneckPanel } from "./BottleneckPanel";

const meta = {
  title: "Library/BottleneckPanel",
  component: BottleneckPanel,
  parameters: {
    docs: {
      description: {
        component:
          "The inspector's centrepiece. The ranking is computed in Rust — at " +
          "most one factor is ever marked binding, and a factor whose ceiling " +
          "Flume cannot measure carries no bar rather than an invented one. " +
          "Three of the five factors the design names have no data behind " +
          "them and are absent entirely.",
      },
    },
  },
  args: {
    bottleneck: {
      factors: [
        {
          name: "Peer upload",
          utilisation: 100,
          value: "6.6 MB/s",
          binding: true,
        },
        {
          name: "Piece availability",
          utilisation: 0,
          value: "rarest on 5 peers",
          binding: false,
        },
        {
          name: "Your download cap",
          utilisation: 41,
          value: "16.0 MB/s",
          binding: false,
        },
      ],
      explanation:
        "The 41 connected peers are supplying 6.6 MB/s, and that is all they " +
        "are offering. Your cap of 16.0 MB/s is not being reached, so no " +
        "setting will make this faster — only more or better-connected peers " +
        "will.",
    },
  },
} satisfies Meta<typeof BottleneckPanel>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The common case: nothing local is limiting, so the swarm is. */
export const SwarmLimited: Story = {};

/** The one factor the user can act on. */
export const CapLimited: Story = {
  args: {
    bottleneck: {
      factors: [
        {
          name: "Your download cap",
          utilisation: 96,
          value: "8.0 MB/s",
          binding: true,
        },
        {
          name: "Piece availability",
          utilisation: 0,
          value: "rarest on 5 peers",
          binding: false,
        },
        {
          name: "Peer upload",
          utilisation: null,
          value: "7.7 MB/s",
          binding: false,
        },
      ],
      explanation:
        "Your download cap of 8.0 MB/s is the limit — the swarm is offering " +
        "at least this much. Raising it in Settings will make this faster, up " +
        "to whatever the peers can supply.",
    },
  },
};

/** Terminal rather than slow: no setting will fix a missing piece. */
export const Starved: Story = {
  args: {
    bottleneck: {
      factors: [
        {
          name: "Piece availability",
          utilisation: 100,
          value: "a missing piece",
          binding: true,
        },
        {
          name: "Your download cap",
          utilisation: 12,
          value: "8.0 MB/s",
          binding: false,
        },
        {
          name: "Peer upload",
          utilisation: null,
          value: "1.0 MB/s",
          binding: false,
        },
      ],
      explanation:
        "No piece is held by any of the 12 connected peers, so this cannot " +
        "finish as it stands. No setting will change that — it needs a peer " +
        "holding the missing pieces to appear.",
    },
  },
};

/** A paused or seeding torrent is not being limited, so there is no panel. */
export const NotLimited: Story = {
  args: { bottleneck: null },
};
