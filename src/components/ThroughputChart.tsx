"use client";

import { useState } from "react";

import {
  WINDOW_SIZE,
  areaPath,
  chartCeiling,
  linePath,
  sampleAt,
  toPoints,
  type ThroughputSample,
} from "@/lib/chart";
import { formatSpeed } from "@/lib/format";

import { Icon } from "./Icon";

/** The chart's user-unit box. The SVG scales; these keep the maths readable. */
const WIDTH = 620;
const HEIGHT = 88;

/**
 * One legend entry: a swatch, a direction glyph and a word.
 *
 * Both class names are written out rather than derived from one another.
 * Tailwind generates utilities by scanning source text, so a class built at
 * runtime — `tone.replace("bg-", "text-")` — is invisible to it and produces a
 * colourless glyph unless some unrelated file happens to mention the same
 * class.
 */
function Legend({
  swatch,
  ink,
  icon,
  children,
}: {
  swatch: string;
  ink: string;
  icon: "arrow-down" | "arrow-up";
  children: React.ReactNode;
}) {
  return (
    <span className="text-fg-1 flex items-center gap-1.5 text-[11px]">
      <span className={`block h-[3px] w-2.5 rounded-sm ${swatch}`} />
      <span className={ink}>
        <Icon name={icon} size={11} />
      </span>
      {children}
    </span>
  );
}

/** Props for {@link ThroughputChart}. */
export interface ThroughputChartProps {
  /** Up to {@link WINDOW_SIZE} samples, oldest first. */
  history: readonly ThroughputSample[];
  /** Configured download limit in bytes/sec, or `null` for unlimited. */
  limitBps: number | null;
  /**
   * What the trace is of.
   *
   * A prop rather than a constant because the same chart draws the whole
   * session in the dock and a single torrent in the inspector, and a chart
   * that mislabels which one it is showing is worse than no chart.
   */
  label?: string;
}

/**
 * Session throughput over the last minute, two series on one scale.
 *
 * Download and upload are told apart three ways — colour, a direction glyph in
 * the legend, and a filled area under download only. The two series separate
 * cleanly under normal, protan and deutan vision but converge under
 * tritanopia, so colour alone can never carry the distinction.
 *
 * The crosshair is the reason this is a chart rather than two numbers: "it
 * dropped about forty seconds ago" is a question a sparkline can answer and a
 * readout cannot.
 *
 * @param props - See {@link ThroughputChartProps}.
 * @returns The rendered chart.
 */
export function ThroughputChart({
  history,
  limitBps,
  label = "Session throughput",
}: ThroughputChartProps) {
  const [hover, setHover] = useState<number | null>(null);

  const peak = history.reduce((max, s) => Math.max(max, s.downBps, s.upBps), 0);
  const ceiling = chartCeiling(peak, limitBps);

  const downPoints = toPoints(
    history.map((s) => s.downBps),
    WIDTH,
    HEIGHT,
    ceiling,
  );
  const upPoints = toPoints(
    history.map((s) => s.upBps),
    WIDTH,
    HEIGHT,
    ceiling,
  );

  const at = hover === null ? null : history[hover];
  const hoverX = hover === null ? 0 : (downPoints[hover]?.x ?? 0);
  // Samples are one second apart, newest last.
  const secondsAgo = hover === null ? 0 : history.length - 1 - hover;

  return (
    <div className="relative flex min-w-0 flex-1 flex-col gap-1.5 px-5 pt-3 pb-2.5">
      <div className="flex items-center gap-4">
        <span className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
          {label} · last {WINDOW_SIZE} s
        </span>
        <Legend swatch="bg-chart-down" ink="text-chart-down" icon="arrow-down">
          Download
        </Legend>
        <Legend swatch="bg-chart-up" ink="text-chart-up" icon="arrow-up">
          Upload
        </Legend>
        <span className="text-fg-3 flex items-center gap-1.5 text-[11px]">
          <span className="bg-fg-3 block h-px w-2.5" />
          {limitBps === null ? "Peak" : "Limit"}{" "}
          <span className="flume-num">{formatSpeed(ceiling)}</span>
        </span>
      </div>

      <svg
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        // Stretches to whatever width it is given rather than holding 620px.
        // A time series has no meaningful aspect ratio — squeezing the minute
        // into less space is the correct response to a narrower window, and
        // letterboxing it would waste the room instead.
        preserveAspectRatio="none"
        className="block h-[88px] w-full overflow-visible"
        role="img"
        aria-label={
          history.length === 0
            ? "Throughput chart, no samples yet"
            : `Throughput over the last ${history.length} seconds. Download now ${formatSpeed(history[history.length - 1].downBps)}, upload now ${formatSpeed(history[history.length - 1].upBps)}, peak ${formatSpeed(peak)}.`
        }
        onMouseMove={(event) => {
          const box = event.currentTarget.getBoundingClientRect();
          // The SVG scales to fit, so pointer pixels are converted back into
          // user units before they mean anything.
          const x = ((event.clientX - box.left) / box.width) * WIDTH;
          setHover(sampleAt(x, WIDTH, history.length));
        }}
        onMouseLeave={() => setHover(null)}
      >
        <line
          x1="0"
          y1="0.5"
          x2={WIDTH}
          y2="0.5"
          stroke="var(--flume-line-2)"
          strokeWidth="1"
          strokeDasharray="3 3"
        />
        <line
          x1="0"
          y1={HEIGHT / 2}
          x2={WIDTH}
          y2={HEIGHT / 2}
          stroke="var(--flume-line)"
          strokeWidth="1"
        />
        <line
          x1="0"
          y1={HEIGHT - 0.5}
          x2={WIDTH}
          y2={HEIGHT - 0.5}
          stroke="var(--flume-line)"
          strokeWidth="1"
        />

        {/*
          Only download is filled. Two overlapping translucent areas make the
          overlap a third colour that means nothing, and hide whichever series
          is smaller behind whichever is larger.
        */}
        <path
          d={areaPath(downPoints, HEIGHT)}
          fill="var(--flume-chart-down)"
          opacity="0.14"
        />
        <path
          d={linePath(downPoints)}
          fill="none"
          stroke="var(--flume-chart-down)"
          strokeWidth="2"
          strokeLinejoin="round"
          strokeLinecap="round"
          vectorEffect="non-scaling-stroke"
        />
        <path
          d={linePath(upPoints)}
          fill="none"
          stroke="var(--flume-chart-up)"
          strokeWidth="2"
          strokeLinejoin="round"
          strokeLinecap="round"
          vectorEffect="non-scaling-stroke"
        />

        {at ? (
          <line
            x1={hoverX}
            y1="0"
            x2={hoverX}
            y2={HEIGHT}
            stroke="var(--flume-line-2)"
            strokeWidth="1"
          />
        ) : null}
      </svg>

      {at ? (
        <div
          className="bg-bg-3 border-line-2 pointer-events-none absolute z-10 rounded-sm border px-[9px] py-[7px] shadow-[0_6px_20px_rgba(0,0,0,0.45)]"
          style={{
            // Flips to the left of the crosshair near the right edge so the
            // tooltip never leaves the dock.
            left:
              hoverX / WIDTH > 0.72
                ? undefined
                : `calc(${(hoverX / WIDTH) * 100}% - 4px)`,
            right:
              hoverX / WIDTH > 0.72
                ? `calc(${100 - (hoverX / WIDTH) * 100}% + 4px)`
                : undefined,
            top: 44,
          }}
        >
          <div className="text-fg-3 mb-1 text-[10px] font-semibold tracking-[0.09em] uppercase">
            {secondsAgo === 0 ? "Now" : `${secondsAgo} s ago`}
          </div>
          <div className="text-fg-1 flex items-center gap-1.5 text-[11px]">
            <span className="bg-chart-down block h-[3px] w-2.5 rounded-sm" />
            Down
            <span className="flume-num text-fg-0">
              {formatSpeed(at.downBps)}
            </span>
          </div>
          <div className="text-fg-1 mt-0.5 flex items-center gap-1.5 text-[11px]">
            <span className="bg-chart-up block h-[3px] w-2.5 rounded-sm" />
            Up
            <span className="flume-num text-fg-0">{formatSpeed(at.upBps)}</span>
          </div>
        </div>
      ) : null}
    </div>
  );
}
