import type { Meta, StoryObj } from "@storybook/nextjs-vite";

import type { CoreStatus, GuardStatus } from "@/lib/ipc/types";
import type { ViewId } from "@/lib/views";

import { Rail } from "./Rail";

const STATUS: CoreStatus = {
  clientVersion: "Flume 1.1.0",
  listenPort: 42221,
  announcePort: 42221,
  dht: { enabled: true, nodesV4: 1400, nodesV6: 56, outstandingRequests: 0 },
  downloadDir: "/Users/you/Downloads",
  uptimeSeconds: 900,
  downloadBps: 4_000_000,
  uploadBps: 250_000,
  livePeers: 12,
  health: "ready",
};

const TUNNELLED: GuardStatus = {
  guard: "hold",
  report: {
    path: { v4: { interface: "utun12", kind: "tunnel" }, v6: null },
    verdict: {
      verdict: "tunnelled",
      interface: "utun12",
      otherFamilyOutside: false,
    },
  },
  held: false,
  resumesInSeconds: null,
};

const HELD: GuardStatus = {
  guard: "hold",
  report: {
    path: { v4: { interface: "en7", kind: "ordinary" }, v6: null },
    verdict: { verdict: "direct", interface: "en7" },
  },
  held: true,
  resumesInSeconds: null,
};

const COUNTS: Record<ViewId, number> = {
  all: 7,
  downloading: 2,
  seeding: 4,
  attention: 1,
  completed: 5,
  paused: 0,
};

const meta = {
  title: "Layout/Rail",
  component: Rail,
  parameters: {
    layout: "fullscreen",
    docs: {
      description: {
        component:
          "The sidebar at both widths. Collapsed is an icon rail, never " +
          "nothing: the network footer is where a held tunnel check is " +
          "reported, and it becomes one status dot whose accessible name " +
          "carries all three lines. Axe runs against both states.",
      },
    },
  },
  // The width comes from the page grid in the app; here the story supplies it
  // so the rail renders at the size the user would see.
  decorators: [
    (Story, { args }) => (
      <div
        className="grid h-[560px] grid-rows-[44px_1fr]"
        style={{ gridTemplateColumns: args.collapsed ? "56px" : "248px" }}
      >
        <Story />
      </div>
    ),
  ],
  args: {
    view: "all",
    onViewChange: () => {},
    counts: COUNTS,
    query: "",
    onQueryChange: () => {},
    status: STATUS,
    loading: false,
    guard: TUNNELLED,
    onToggleCollapsed: () => {},
    collapsed: false,
  },
} satisfies Meta<typeof Rail>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Expanded: Story = {};

export const Collapsed: Story = { args: { collapsed: true } };

/**
 * The reason it never collapses to zero. Held, the footer shows a warning
 * glyph and the word "Held" — shape and text, not hue, because `--flume-ok`
 * against `--flume-warn` is 1.09:1 and says nothing on a greyscale display.
 * Its accessible name carries the whole footer as a sentence.
 *
 * `status` is null here, so the DHT and port lines read as their absent
 * states; that is what a held rail actually looks like, since holding means
 * no engine is running.
 */
export const CollapsedHeld: Story = {
  args: { collapsed: true, guard: HELD, status: null },
};

export const ExpandedHeld: Story = {
  args: { collapsed: false, guard: HELD, status: null },
};
