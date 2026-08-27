import type { Meta, StoryObj } from "@storybook/nextjs-vite";

import { StatCard } from "./StatCard";

const meta = {
  title: "Primitives/StatCard",
  component: StatCard,
  parameters: {
    docs: {
      description: {
        component:
          "Label above, mono value, caption below. Tabular figures are not " +
          "decoration — without them a number updating at 1 Hz shoves its " +
          "neighbours sideways as digit widths change.",
      },
    },
  },
  args: { label: "Share ratio", value: "0.22" },
} satisfies Meta<typeof StatCard>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Dock: Story = {};

export const Strip: Story = {
  args: {
    size: "strip",
    label: "Availability",
    value: "4.2×",
    hint: "rarest piece on 4 peers",
  },
};

/** A value carrying its unit at a smaller size, as the inspector draws it. */
export const WithUnit: Story = {
  args: {
    size: "strip",
    label: "Down",
    value: (
      <>
        6.6 <span className="text-fg-2 text-xs">MB/s</span>
      </>
    ),
    hint: "peak 8.8 MB/s",
  },
};

/** The library dock's aggregate readout. */
export const DockRow: Story = {
  render: () => (
    <div className="flex gap-8">
      {(
        [
          ["Active", "6 of 13"],
          ["Session down", "184 GB"],
          ["Session up", "41.2 GB"],
          ["Share ratio", "0.22"],
          ["Connected peers", "106"],
          ["Connecting", "17"],
          ["Disk queue", "3.1 MB"],
          ["Uptime", "4 d 07 h"],
        ] as const
      ).map(([label, value]) => (
        <StatCard key={label} label={label} value={value} />
      ))}
    </div>
  ),
};

/** The inspector's seven-across stat strip. */
export const StatStrip: Story = {
  render: () => (
    <div className="border-line bg-bg-1 flex rounded-lg border">
      {(
        [
          ["Progress", "42.7%", "19.7 of 46.1 GB selected"],
          ["Time left", "1 h 07 min", "steady for 12 min"],
          ["Peers", "41 / 206", "38 seeds · 168 downloading"],
          ["Availability", "4.2×", "rarest piece on 4 peers"],
          ["Ratio", "0.02", "target 2.00"],
        ] as const
      ).map(([label, value, hint]) => (
        <StatCard
          key={label}
          size="strip"
          label={label}
          value={value}
          hint={hint}
        />
      ))}
    </div>
  ),
};
