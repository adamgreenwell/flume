"use client";

/**
 * Seed-ratio stops, coarsening as they climb.
 *
 * Same reasoning as {@link RATE_STEPS}: the difference between 1.0 and 1.5 is
 * a real decision and the difference between 8 and 10 is not. `null` sits at
 * the top because "seed forever" is the same decision as "seed to a very high
 * ratio", not a separate mode.
 */
export const RATIO_STEPS: ReadonlyArray<number | null> = [
  0.5,
  1,
  1.5,
  2,
  2.5,
  3,
  4,
  5,
  8,
  10,
  null,
];

/**
 * Seed-time stops in seconds, from an hour to a fortnight.
 *
 * Counts time spent seeding rather than time since the torrent was added, so
 * these are longer than they look: a day here is a day of actual seeding.
 */
export const SEED_TIME_STEPS: ReadonlyArray<number | null> = [
  3_600,
  6 * 3_600,
  12 * 3_600,
  86_400,
  2 * 86_400,
  3 * 86_400,
  7 * 86_400,
  14 * 86_400,
  null,
];

/**
 * Renders a stepped slider over `steps`, with `null` meaning no limit.
 *
 * Factored out of the rate control when the seed limits arrived, and shared
 * again when those limits became settable per torrent. The global row and the
 * per-torrent one must behave identically -- they set the same kind of value,
 * and a slider that snapped differently in two places would be a bug nobody
 * would think to look for.
 */
export function SteppedSlider({
  steps,
  value,
  label,
  format,
  onChange,
  width,
}: {
  steps: ReadonlyArray<number | null>;
  value: number | null;
  label: string;
  format: (value: number) => string;
  onChange: (next: number | null) => void;
  width: string;
}) {
  // Nearest step, so a value set from outside this slider still lands
  // somewhere sensible on it rather than snapping to the start.
  let index = steps.length - 1;
  if (value !== null) {
    let best = Number.POSITIVE_INFINITY;
    steps.forEach((step, i) => {
      if (step === null) return;
      const distance = Math.abs(step - value);
      if (distance < best) {
        best = distance;
        index = i;
      }
    });
  }

  const text = value === null ? "No limit" : format(value);

  return (
    <div className="flex items-center gap-2.5">
      <input
        type="range"
        min={0}
        max={steps.length - 1}
        step={1}
        value={index}
        aria-label={label}
        aria-valuetext={text}
        onChange={(event) =>
          onChange(steps[Number(event.target.value)] ?? null)
        }
        className="accent-acc w-[150px]"
      />
      <span
        className={`flume-num text-fg-0 ${width} shrink-0 text-right text-[11.5px]`}
      >
        {text}
      </span>
    </div>
  );
}
