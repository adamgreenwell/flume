/**
 * Geometry for the dock's throughput chart.
 *
 * Pure functions with no React or DOM dependency, so the shapes can be tested
 * directly rather than by looking at a rendered chart and deciding it seems
 * about right.
 */

/** One second of session-wide throughput. */
export interface ThroughputSample {
  /** Download rate, bytes per second. */
  downBps: number;
  /** Upload rate, bytes per second. */
  upBps: number;
}

/** How many samples the chart shows: 60 seconds at 1 Hz. */
export const WINDOW_SIZE = 60;

/**
 * The lowest ceiling the chart will draw, in bytes per second.
 *
 * 1 MB/s. Without a floor, an idle session scales its axis to a few hundred
 * bytes and renders background chatter as dramatic peaks — the chart would be
 * at its most alarming exactly when nothing is happening.
 */
const MIN_CEILING = 1_000_000;

/**
 * Picks the value the top of the chart represents.
 *
 * A configured rate limit wins: when the user has set one, the interesting
 * question is how close to it they are, and an axis that rescales as traffic
 * varies cannot answer that. Without one, the axis is a round number above the
 * busiest sample, so the shape of the last minute fills the space available.
 *
 * Rounds to 1, 2 or 5 times a power of ten. Arbitrary maxima make the gridline
 * label unreadable and the chart's height meaningless between glances.
 *
 * @param peakBps - The largest sample in the window, bytes per second.
 * @param limitBps - The configured rate limit, or `null` for unlimited.
 * @returns The ceiling in bytes per second, never zero.
 */
export function chartCeiling(peakBps: number, limitBps: number | null): number {
  if (limitBps !== null && limitBps > 0) return limitBps;

  const target = Math.max(peakBps, MIN_CEILING);
  const magnitude = 10 ** Math.floor(Math.log10(target));
  const normalized = target / magnitude;

  const step =
    normalized <= 1 ? 1 : normalized <= 2 ? 2 : normalized <= 5 ? 5 : 10;
  return step * magnitude;
}

/**
 * Maps samples onto x/y coordinates in the chart's box.
 *
 * Both series share one scale so the two lines can be compared by eye — a
 * chart that scaled upload to its own axis would show a trickle and a torrent
 * as the same height.
 *
 * The window is always {@link WINDOW_SIZE} wide even when fewer samples exist,
 * so a chart that has been running for ten seconds draws ten seconds of line
 * at the right rather than stretching them across the full width and implying
 * a minute of history it does not have.
 *
 * @param values - Rates in bytes per second, oldest first.
 * @param width - Chart width in user units.
 * @param height - Chart height in user units.
 * @param ceiling - What the top of the chart represents.
 * @returns One point per sample, positioned right-aligned in the window.
 */
export function toPoints(
  values: readonly number[],
  width: number,
  height: number,
  ceiling: number,
): Array<{ x: number; y: number }> {
  if (values.length === 0) return [];

  const stepX = width / (WINDOW_SIZE - 1);
  // Right-aligned: the newest sample sits at the right edge regardless of how
  // many there are, so the line grows leftward as history accumulates.
  const offset = width - (values.length - 1) * stepX;

  return values.map((value, index) => ({
    x: offset + index * stepX,
    y: height - Math.min(value / ceiling, 1) * height,
  }));
}

/**
 * Builds a smooth-stepped line through the points.
 *
 * Each segment runs flat out of one sample and flat into the next, joined by a
 * cubic whose control points sit at the midpoint. This is what the design
 * draws, and it is the honest shape for a 1 Hz sample: a straight line between
 * two readings claims the rate moved evenly between them, which is a
 * measurement nobody took.
 *
 * @param points - Points from {@link toPoints}.
 * @returns An SVG path `d`, or an empty string for fewer than two points.
 */
export function linePath(
  points: ReadonlyArray<{ x: number; y: number }>,
): string {
  if (points.length < 2) return "";

  let d = `M${points[0].x.toFixed(1)} ${points[0].y.toFixed(1)}`;
  for (let i = 1; i < points.length; i++) {
    const previous = points[i - 1];
    const current = points[i];
    const mid = ((previous.x + current.x) / 2).toFixed(1);
    d += ` C${mid} ${previous.y.toFixed(1)} ${mid} ${current.y.toFixed(1)} ${current.x.toFixed(1)} ${current.y.toFixed(1)}`;
  }
  return d;
}

/**
 * Closes a line path down to the baseline so it can be filled.
 *
 * @param points - Points from {@link toPoints}.
 * @param height - Chart height in user units, i.e. where zero sits.
 * @returns An SVG path `d`, or an empty string for fewer than two points.
 */
export function areaPath(
  points: ReadonlyArray<{ x: number; y: number }>,
  height: number,
): string {
  const line = linePath(points);
  if (line === "") return "";

  const first = points[0];
  const last = points[points.length - 1];
  return `${line} L${last.x.toFixed(1)} ${height} L${first.x.toFixed(1)} ${height} Z`;
}

/**
 * Which sample a pointer at `x` is over.
 *
 * Returns `null` when the pointer or the width is not a usable number. That
 * is what a caller gets from an element which has not been laid out — a hidden
 * tab, a zero-size container — and a zero width makes the arithmetic below
 * divide by zero. Better a missing tooltip than an index of `NaN` reaching
 * into the array.
 *
 * @param x - Pointer position within the chart, in user units.
 * @param width - Chart width in user units.
 * @param count - How many samples exist.
 * @returns The sample index, clamped into range, or `null` when there are none.
 */
export function sampleAt(
  x: number,
  width: number,
  count: number,
): number | null {
  if (count === 0 || width <= 0 || !Number.isFinite(x)) return null;

  const stepX = width / (WINDOW_SIZE - 1);
  const offset = width - (count - 1) * stepX;
  const index = Math.round((x - offset) / stepX);

  return Math.min(Math.max(index, 0), count - 1);
}
