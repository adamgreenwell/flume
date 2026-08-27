import type { Meta, StoryObj } from "@storybook/nextjs-vite";

import { Checkbox } from "./Checkbox";

const meta = {
  title: "Primitives/Checkbox",
  component: Checkbox,
  parameters: {
    docs: {
      description: {
        component:
          "`partial` is what makes folder checkboxes usable: without a third " +
          "mark, a folder with one file deselected looks identical to one " +
          'with everything deselected. Exposed as `aria-checked="mixed"`, ' +
          "which is the real ARIA value and is announced as such.",
      },
    },
  },
  args: { state: "on", label: "Select all files", onChange: () => {} },
} satisfies Meta<typeof Checkbox>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Checked: Story = {};
export const Empty: Story = { args: { state: "off" } };
export const Partial: Story = { args: { state: "partial" } };

/** All three together. The empty box's border is the 3:1 token — it has to be visible. */
export const AllStates: Story = {
  render: () => (
    <div className="flex items-center gap-4">
      <Checkbox state="on" label="checked" onChange={() => {}} />
      <Checkbox state="partial" label="partial" onChange={() => {}} />
      <Checkbox state="off" label="empty" onChange={() => {}} />
    </div>
  ),
};
