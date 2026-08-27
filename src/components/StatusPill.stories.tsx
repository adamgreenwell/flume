import type { Meta, StoryObj } from "@storybook/nextjs-vite";

import type { EngineHealth } from "@/lib/ipc/types";

import { StatusPill } from "./StatusPill";

const meta = {
  title: "Primitives/StatusPill",
  component: StatusPill,
  parameters: {
    docs: {
      description: {
        component:
          "A dot and a word, never colour alone. The pill is only the " +
          "adjective — the sentence saying what the state means and what to " +
          "do about it belongs to whatever the pill labels.",
      },
    },
  },
  args: { health: "ready" },
} satisfies Meta<typeof StatusPill>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Ready: Story = {};
export const Starting: Story = { args: { health: "starting" } };
export const Connecting: Story = {
  args: { health: "connecting", pulse: true },
};
export const Degraded: Story = { args: { health: "degraded" } };

/**
 * All four together. Only `degraded` carries a tint — reserving the fill for
 * "look at this" is what keeps it meaning anything.
 */
export const AllStates: Story = {
  render: () => (
    <div className="flex flex-col items-start gap-2">
      {(["starting", "connecting", "ready", "degraded"] as EngineHealth[]).map(
        (health) => (
          <StatusPill key={health} health={health} />
        ),
      )}
    </div>
  ),
};
