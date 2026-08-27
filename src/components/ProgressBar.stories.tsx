import type { Meta, StoryObj } from "@storybook/nextjs-vite";

import { ProgressBar } from "./ProgressBar";

const meta = {
  title: "Primitives/ProgressBar",
  component: ProgressBar,
  parameters: {
    docs: {
      description: {
        component:
          "5px tall, 3px radius, always with its percentage beside it. At " +
          "this height a 3% fill and a 0% fill are the same two pixels, so a " +
          "bar without a number lies exactly when the user cares most.",
      },
    },
  },
  args: { value: 0.68, state: "downloading", label: "Download progress" },
} satisfies Meta<typeof ProgressBar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Downloading: Story = {};

export const Complete: Story = {
  args: { value: 1, state: "seeding" },
};

export const Verifying: Story = {
  args: { value: 0.41, state: "checking" },
};

export const Stopped: Story = {
  args: { value: 0.44, state: "error" },
};

export const Paused: Story = {
  args: { value: 0.82, state: "paused" },
};

/**
 * The case the numeric label exists for. The bar is indistinguishable from
 * empty; the number is the only thing carrying the value.
 */
export const BarelyStarted: Story = {
  args: { value: 0.03, state: "downloading" },
};

/** Every state at the design sheet's own percentages. */
export const AllStates: Story = {
  render: () => (
    <div className="flex w-[280px] flex-col gap-3">
      {(
        [
          [0.68, "downloading", "downloading"],
          [1, "seeding", "seeding / complete"],
          [0.41, "checking", "verifying"],
          [0.44, "error", "stopped"],
          [0.82, "paused", "paused or queued"],
        ] as const
      ).map(([value, state, caption]) => (
        <div key={caption} className="flex flex-col gap-1">
          <ProgressBar value={value} state={state} label={caption} />
          <span className="text-fg-3 text-[10.5px]">{caption}</span>
        </div>
      ))}
    </div>
  ),
};
