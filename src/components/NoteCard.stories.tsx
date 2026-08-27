import type { Meta, StoryObj } from "@storybook/nextjs-vite";

import { NoteCard } from "./NoteCard";

const meta = {
  title: "Library/NoteCard",
  component: NoteCard,
  parameters: {
    docs: {
      description: {
        component:
          "Never a bare adjective. Every note names a cause and, where there " +
          "is one, the next move. The text is derived in Rust — the engine is " +
          "the only thing that knows why a torrent is in the state it is in.",
      },
    },
  },
  args: {
    note: {
      severity: "ok",
      title: "Pulling from 41 of 206 known peers",
      body: "19.7 GB verified so far, 26.4 GB to go, arriving at 6.6 MB/s. About 1 h 07 min left at this rate.",
    },
  },
} satisfies Meta<typeof NoteCard>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Healthy: Story = {};

export const NobodyAnswering: Story = {
  args: {
    note: {
      severity: "err",
      title: "Nobody reachable has the rest of this",
      body: "3 peers are known for this torrent and none of them is answering right now. Flume keeps asking the DHT and the trackers every few minutes; 79.8 GB is still missing.",
    },
  },
};

export const DiskFull: Story = {
  args: {
    note: {
      severity: "err",
      title: "The disk this is saving to is full",
      body: "Writing piece 489 failed: No space left on device. Free some space, then press Resume — everything already verified is kept and will not be downloaded again.",
    },
  },
};

export const ConnectedButIdle: Story = {
  args: {
    note: {
      severity: "warn",
      title: "Connected to 6 peers, but nothing is arriving",
      body: "The connections are open and no data is coming down them. That usually means every connected peer is choking you — they have nothing you need, or they are busy serving someone else. 203 GB is still missing.",
    },
  },
};

/** Neutral is a claim, not an absence: nothing is wrong, and you should know. */
export const Paused: Story = {
  args: {
    note: {
      severity: "neutral",
      title: "Paused, nothing lost",
      body: "Your 4.87 GB is verified on disk. Resuming reconnects to the swarm and picks up from there — nothing is downloaded twice.",
    },
  },
};

/** Every severity together, to check the glyphs carry the difference. */
export const AllSeverities: Story = {
  render: () => (
    <div className="flex flex-col gap-2.5">
      <NoteCard
        note={{
          severity: "ok",
          title: "Pulling from 41 of 206 known peers",
          body: "Arriving at 6.6 MB/s.",
        }}
      />
      <NoteCard
        note={{
          severity: "warn",
          title: "No peers found yet",
          body: "Flume is asking the DHT and this torrent's trackers for somewhere to start.",
        }}
      />
      <NoteCard
        note={{
          severity: "err",
          title: "The disk this is saving to is full",
          body: "Free some space, then press Resume.",
        }}
      />
      <NoteCard
        note={{
          severity: "neutral",
          title: "Paused, nothing lost",
          body: "Your 4.87 GB is verified on disk.",
        }}
      />
    </div>
  ),
};
