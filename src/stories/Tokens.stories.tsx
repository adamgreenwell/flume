import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { useEffect, useState } from "react";

/** Every colour token, with the role it is allowed to play. */
const COLOURS: ReadonlyArray<readonly [string, string]> = [
  ["bg-0", "window ground, list background"],
  ["bg-1", "rails, docks, cards"],
  ["bg-2", "inputs, raised rows"],
  ["bg-3", "hover, empty progress track"],
  ["line", "hairline between rows"],
  ["line-2", "control borders — clears 3:1 everywhere"],
  ["fg-0", "primary text"],
  ["fg-1", "secondary text, consequence lines"],
  ["fg-2", "meta lines, units"],
  ["fg-3", "10px labels — the floor that clears 4.5:1"],
  ["fg-dis", "disabled only — never load-bearing text"],
  ["acc", "the one interactive colour"],
  ["acc-dim", "verified pieces, filled tracks"],
  ["acc-deep", "selected chips, tinted cards"],
  ["on-acc", "ink on an accent fill"],
  ["acc-hi", "accent hover"],
  ["ok", "seeding, healthy swarm, verified"],
  ["ok-deep", "tinted healthy surfaces"],
  ["warn", "thin swarm, below ratio, verifying"],
  ["err", "stopped, disk full, tracker refusal"],
  ["chart-down", "chart series: download"],
  ["chart-up", "chart series: upload"],
];

/** The type ramp, as specified in the design system sheet. */
const RAMP: ReadonlyArray<{
  spec: string;
  sample: string;
  use: string;
  className: string;
}> = [
  {
    spec: "Display 30 / 600 · −0.035em",
    sample: "Three answers and you are done.",
    use: "first-run, empty states",
    className: "text-[30px] font-semibold tracking-[-0.035em] leading-[1.1]",
  },
  {
    spec: "Section 20 / 600 · −0.028em",
    sample: "Needs attention",
    use: "settings and view titles",
    className: "text-xl font-semibold tracking-[-0.028em]",
  },
  {
    spec: "Stat 20 / 500 mono",
    sample: "14.8 MB/s",
    use: "the numbers you watch",
    className: "flume-num text-xl font-medium tracking-[-0.028em]",
  },
  {
    spec: "Title 15 / 600 · −0.015em",
    sample: "Sprite Fright — Blender Open Movie",
    use: "card and panel headings",
    className: "text-[15px] font-semibold tracking-[-0.015em]",
  },
  {
    spec: "Row 13 / 500 · −0.005em",
    sample: "debian-13.2.0-amd64-DVD-1.iso",
    use: "torrent names, setting names",
    className: "text-[13px] font-medium tracking-[-0.005em]",
  },
  {
    spec: "Body 12.5 / 400 · lh 1.5",
    sample: "Stops the classic failure where three torrents fill the disk.",
    use: "consequence lines, help text",
    className: "text-[12.5px] leading-[1.5]",
  },
  {
    spec: "Meta 11 / 400 · fg-2",
    sample: "4.31 GB · Linux · 2 min 40 s left",
    use: "second line of a row",
    className: "text-fg-2 text-[11px]",
  },
  {
    spec: "Data 11.5 mono",
    sample: "24 / 118   9.4 MB/s   0.14",
    use: "every table cell holding a number",
    className: "flume-num text-[11.5px]",
  },
  {
    spec: "Label 10 / 600 · caps 0.09em",
    sample: "SWARM HEALTH",
    use: "column heads, field labels",
    className:
      "text-fg-3 text-[10px] font-semibold uppercase tracking-[0.09em]",
  },
];

/**
 * Reads a token's resolved value from the document.
 *
 * Reading rather than hardcoding is the point: the swatch shows what the
 * active theme actually computes, so flipping the toolbar proves the runtime
 * swap works instead of showing a value someone typed in twice.
 */
function useTokenValues(names: readonly string[]): Record<string, string> {
  const [values, setValues] = useState<Record<string, string>>({});

  useEffect(() => {
    const observer = new MutationObserver(read);
    function read() {
      const style = getComputedStyle(document.documentElement);
      setValues(
        Object.fromEntries(
          names.map((n) => [n, style.getPropertyValue(`--flume-${n}`).trim()]),
        ),
      );
    }
    read();
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => observer.disconnect();
  }, [names]);

  return values;
}

function Swatches() {
  const names = COLOURS.map(([name]) => name);
  const values = useTokenValues(names);

  return (
    <div className="grid grid-cols-4 gap-2.5">
      {COLOURS.map(([name, use]) => (
        <div
          key={name}
          className="border-line bg-bg-1 overflow-hidden rounded-lg border"
        >
          <div
            className="border-line h-12 border-b"
            style={{ background: `var(--flume-${name})` }}
          />
          <div className="flex flex-col gap-0.5 px-2.5 py-2">
            <span className="flume-num text-fg-0 text-[11px] font-semibold">
              {name}
            </span>
            <span className="flume-num text-fg-2 text-[10.5px]">
              {values[name] ?? "…"}
            </span>
            <span className="text-fg-3 text-[10px] leading-[1.4]">{use}</span>
          </div>
        </div>
      ))}
    </div>
  );
}

const meta = {
  title: "Design system/Tokens",
  parameters: {
    docs: {
      description: {
        component:
          "The vocabulary. Values are read live from the document, so " +
          "switching the theme in the toolbar shows what each role actually " +
          "resolves to rather than a hex someone typed twice.",
      },
    },
  },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

export const Colours: Story = {
  render: () => <Swatches />,
};

export const Type: Story = {
  render: () => (
    <div className="flex flex-col">
      {RAMP.map((row) => (
        <div
          key={row.spec}
          className="border-line flex items-baseline gap-6 border-b py-3 last:border-b-0"
        >
          <span className="text-fg-2 w-[210px] shrink-0 text-[11px]">
            {row.spec}
          </span>
          <span className={`text-fg-0 grow ${row.className}`}>
            {row.sample}
          </span>
          <span className="text-fg-3 w-[190px] shrink-0 text-[11px]">
            {row.use}
          </span>
        </div>
      ))}
    </div>
  ),
};

export const Radii: Story = {
  render: () => (
    <div className="flex items-end gap-5">
      {(
        [
          ["r-sm", "4px", "chips, tags, small controls"],
          ["r-md", "6px", "buttons, inputs, nav items"],
          ["r-lg", "9px", "cards, panels"],
        ] as const
      ).map(([token, size, use]) => (
        <div key={token} className="flex flex-col items-center gap-2">
          <div
            className="border-line-2 bg-bg-2 h-16 w-16 border"
            style={{ borderRadius: `var(--flume-${token})` }}
          />
          <span className="flume-num text-fg-0 text-[11px]">{size}</span>
          <span className="text-fg-3 w-24 text-center text-[10px]">{use}</span>
        </div>
      ))}
    </div>
  ),
};

export const ControlHeights: Story = {
  render: () => (
    <div className="flex items-end gap-5">
      {(
        [
          ["h-chip", "28px", "chips, icon buttons"],
          ["h-control", "30px", "chrome buttons, inputs"],
          ["h-primary", "34px", "a sheet's primary action"],
        ] as const
      ).map(([token, size, use]) => (
        <div key={token} className="flex flex-col items-center gap-2">
          <div
            className="border-line-2 bg-bg-2 flex w-32 items-center justify-center rounded-md border"
            style={{ height: `var(--flume-${token})` }}
          >
            <span className="flume-num text-fg-1 text-[11px]">{size}</span>
          </div>
          <span className="text-fg-3 w-32 text-center text-[10px]">{use}</span>
        </div>
      ))}
    </div>
  ),
};
