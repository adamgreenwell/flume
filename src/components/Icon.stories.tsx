import type { Meta, StoryObj } from "@storybook/nextjs-vite";

import { Icon, type IconName } from "./Icon";

const ALL: IconName[] = [
  "pause",
  "play",
  "folder",
  "files",
  "trash",
  "plus",
  "settings",
  "search",
  "chevron-right",
  "chevron-down",
  "arrow-down",
  "arrow-up",
  "check",
  "dash",
  "clock",
  "check-circle",
  "alert-circle",
  "alert-triangle",
];

/** Glyphs the design never drew, built in its idiom and awaiting review. */
const UNDESIGNED: ReadonlySet<IconName> = new Set([
  "play",
  "trash",
  "settings",
]);

const meta = {
  title: "Primitives/Icon",
  component: Icon,
  parameters: {
    docs: {
      description: {
        component:
          "Stroked SVG on the design's 16×16 grid. Nothing is filled — a " +
          "solid glyph beside stroked ones reads as a different weight class.",
      },
    },
  },
  args: { name: "search", size: 16 },
} satisfies Meta<typeof Icon>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Single: Story = {};

/** The whole set. Anything marked below is not from the design. */
export const Gallery: Story = {
  render: () => (
    <div className="grid grid-cols-6 gap-4">
      {ALL.map((name) => (
        <div
          key={name}
          className="border-line bg-bg-1 flex flex-col items-center gap-2 rounded-lg border p-3"
        >
          <span className="text-fg-0">
            <Icon name={name} size={20} />
          </span>
          <span className="text-fg-3 text-center text-[10px]">
            {name}
            {UNDESIGNED.has(name) ? " *" : ""}
          </span>
        </div>
      ))}
    </div>
  ),
};

/**
 * The same glyph at every size it is drawn at, to check that the stroke holds
 * a constant optical weight rather than thickening as the icon shrinks.
 */
export const Sizes: Story = {
  render: () => (
    <div className="text-fg-0 flex items-end gap-5">
      {[12, 14, 15, 16, 20, 24].map((size) => (
        <div key={size} className="flex flex-col items-center gap-2">
          <Icon name="folder" size={size} />
          <span className="text-fg-3 flume-num text-[10px]">{size}px</span>
        </div>
      ))}
    </div>
  ),
};
