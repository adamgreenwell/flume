import type { Meta, StoryObj } from "@storybook/nextjs-vite";

import { Button } from "./Button";
import { Icon } from "./Icon";

const meta = {
  title: "Primitives/Button",
  component: Button,
  parameters: {
    docs: {
      description: {
        component:
          "One primary action per decision. The primary names the object and " +
          "the count — “Add 6 files · 46.1 GB”, never “OK”.",
      },
    },
  },
  args: { children: "Add torrent" },
} satisfies Meta<typeof Button>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Primary: Story = {
  args: { variant: "primary" },
};

export const Secondary: Story = {
  args: { variant: "secondary", children: "Choose folder…" },
};

export const Ghost: Story = {
  args: { variant: "ghost", children: "Cancel" },
};

export const Danger: Story = {
  args: { variant: "danger", children: "Remove and delete data" },
  parameters: {
    docs: {
      description: {
        story:
          "Not in the design system sheet — built from its vocabulary and " +
          "awaiting review. Tinted only on hover, so a destructive action " +
          "does not shout from across the window.",
      },
    },
  },
};

/** The 34px scale a sheet uses for the one decision it exists to ask. */
export const DialogScale: Story = {
  args: {
    variant: "primary",
    size: "dialog",
    children: "Add 6 files · 46.1 GB",
  },
};

export const Disabled: Story = {
  args: { variant: "primary", disabled: true, children: "Add 0 files" },
  parameters: {
    docs: {
      description: {
        story:
          "Disabled is a recolour, not a fade. The label still has to be " +
          "readable — it usually names the thing you cannot yet do.",
      },
    },
  },
};

export const WithIcon: Story = {
  args: {
    variant: "primary",
    children: (
      <>
        <Icon name="plus" size={14} />
        Add torrent
      </>
    ),
  },
};

/** Every variant and scale together, for checking weight against each other. */
export const AllVariants: Story = {
  render: () => (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2.5">
        <Button variant="primary">Add torrent</Button>
        <Button variant="secondary">Secondary</Button>
        <Button variant="ghost">Ghost</Button>
        <Button variant="danger">Remove</Button>
      </div>
      <div className="flex flex-wrap items-center gap-2.5">
        <Button variant="primary" disabled>
          Add torrent
        </Button>
        <Button variant="secondary" disabled>
          Secondary
        </Button>
        <Button variant="ghost" disabled>
          Ghost
        </Button>
        <Button variant="danger" disabled>
          Remove
        </Button>
      </div>
      <div className="flex flex-wrap items-center gap-2.5">
        <Button variant="primary" size="dialog">
          Add 6 files · 46.1 GB
        </Button>
        <Button variant="ghost" size="dialog">
          Cancel
        </Button>
      </div>
    </div>
  ),
};
