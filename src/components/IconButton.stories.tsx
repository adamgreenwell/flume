import type { Meta, StoryObj } from "@storybook/nextjs-vite";

import { IconButton } from "./IconButton";

const meta = {
  title: "Primitives/IconButton",
  component: IconButton,
  parameters: {
    docs: {
      description: {
        component:
          "28px pointer target. Sits at `fg-2` until hovered or focused — " +
          "colour only, so it stays hit-testable and keyboard reachable " +
          "rather than appearing on hover and stranding everyone else.",
      },
    },
  },
  args: { icon: "pause", label: "Pause" },
} satisfies Meta<typeof IconButton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Destructive: Story = {
  args: { icon: "trash", label: "Remove", destructive: true },
};

export const Disabled: Story = {
  args: { icon: "folder", label: "Open containing folder", disabled: true },
};

/** The row action cluster, as it appears at the right of a torrent. */
export const RowActions: Story = {
  render: () => (
    <div className="flex items-center gap-0.5">
      <IconButton icon="pause" label="Pause" />
      <IconButton icon="files" label="Files and details" />
      <IconButton icon="folder" label="Open containing folder" />
      <IconButton icon="trash" label="Remove" destructive />
    </div>
  ),
};
