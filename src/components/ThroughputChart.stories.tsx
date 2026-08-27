import type { Meta, StoryObj } from "@storybook/nextjs-vite";

import type { ThroughputSample } from "@/lib/chart";
import { WINDOW_SIZE } from "@/lib/chart";

import { ThroughputChart } from "./ThroughputChart";

/**
 * A minute of plausible traffic.
 *
 * Two sine waves at different periods, so the trace has the uneven shape of
 * real transfer rather than an obviously repeating pattern that would make the
 * chart look better than it is.
 */
function session(count = WINDOW_SIZE): ThroughputSample[] {
  return Array.from({ length: count }, (_, i) => ({
    downBps:
      6_600_000 *
      (1 + 0.28 * Math.sin((i / 17) * Math.PI * 2) + 0.12 * Math.sin(i / 5)),
    upBps:
      900_000 *
      (1 + 0.34 * Math.sin((i / 11) * Math.PI * 2 + 2.4) + 0.15 * Math.sin(i)),
  }));
}

const meta = {
  title: "Library/ThroughputChart",
  component: ThroughputChart,
  parameters: {
    docs: {
      description: {
        component:
          "Two series on one shared scale. Download and upload are told " +
          "apart three ways — colour, a direction glyph, and a filled area " +
          "under download only — because the two series converge under " +
          "tritanopia and colour alone can never carry the distinction.",
      },
    },
  },
  args: { history: session(), limitBps: null },
} satisfies Meta<typeof ThroughputChart>;

export default meta;
type Story = StoryObj<typeof meta>;

export const AFullMinute: Story = {};

/** Ten seconds in. The line grows leftward rather than stretching to fit. */
export const JustStarted: Story = {
  args: { history: session(10) },
};

/** Before the first tick. Everything holds its space and says nothing false. */
export const NoSamplesYet: Story = {
  args: { history: [] },
};

/**
 * With a rate limit set, the ceiling is the limit rather than the peak — the
 * question becomes how close to it you are, which a rescaling axis cannot
 * answer.
 */
export const AgainstARateLimit: Story = {
  args: { history: session(), limitBps: 18_000_000 },
};

/** An idle session. The axis floors at 1 MB/s rather than magnifying noise. */
export const Idle: Story = {
  args: {
    history: Array.from({ length: WINDOW_SIZE }, () => ({
      downBps: 0,
      upBps: 0,
    })),
  },
};
