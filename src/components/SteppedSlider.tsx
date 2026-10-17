"use client";

import { useState } from "react";

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
 * Active-torrent stops, coarsening as they climb.
 *
 * Dense at the low end because that is where the decision is: the difference
 * between two and three concurrent downloads is one a user feels, and the
 * difference between fifteen and twenty is one nobody tunes deliberately.
 * `null` sits at the top because "no limit" is the same decision as "a very
 * high limit" -- it is where the slider already was before anyone touched it.
 */
export const COUNT_STEPS: ReadonlyArray<number | null> = [
  1,
  2,
  3,
  4,
  5,
  6,
  8,
  10,
  15,
  20,
  null,
];

/**
 * Peer-connection stops, per torrent, coarsening as they climb.
 *
 * The only table here with no `null`, and that is the point: there is no "no
 * limit" to offer. librqbit caps peers at 128 when Flume passes nothing, so
 * the slider's job is to move an existing ceiling rather than to remove one.
 *
 * 128 is a stop because it is that existing default -- a user who drags the
 * slider and changes their mind can put it back exactly. The top stop is the
 * same figure `MAX_PEER_LIMIT` validates against, so the guided path cannot
 * produce a value the Rust side will reject. The floor is 10 rather than 1:
 * the validator only refuses 0, but a torrent held to one peer is broken in a
 * way that looks like a dead swarm, and a slider should not offer that.
 */
export const PEER_STEPS: ReadonlyArray<number | null> = [
  10, 20, 30, 40, 50, 65, 80, 100, 128, 160, 200, 300, 500,
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
  commit = "change",
}: {
  steps: ReadonlyArray<number | null>;
  value: number | null;
  label: string;
  format: (value: number) => string;
  onChange: (next: number | null) => void;
  width: string;
  /**
   * When to report a new value: on every stop crossed, or once on release.
   *
   * `"change"` is right for a setting that applies live — the engine takes a
   * new rate limit in microseconds, and a figure that only moves when you let
   * go feels unresponsive.
   *
   * `"release"` is for a setting whose write is expensive. A drag across this
   * control crosses every stop between the ends, and each one is a round trip
   * to `update_settings`; where that rebuilds the librqbit session, a single
   * drag would otherwise queue a dozen engine restarts, each tearing down the
   * listener and re-announcing every torrent to its trackers. The caret still
   * tracks the drag — only the commit waits.
   */
  commit?: "change" | "release";
}) {
  // Set while dragging a `commit: "release"` slider, so the control can show
  // where the caret is without having reported it yet. `undefined` means not
  // dragging; `null` is a real value, so it cannot stand in for that.
  const [pending, setPending] = useState<number | null | undefined>(undefined);
  const shown = pending === undefined ? value : pending;

  // Nearest step, so a value set from outside this slider still lands
  // somewhere sensible on it rather than snapping to the start.
  let index = steps.length - 1;
  if (shown !== null) {
    let best = Number.POSITIVE_INFINITY;
    steps.forEach((step, i) => {
      if (step === null) return;
      const distance = Math.abs(step - shown);
      if (distance < best) {
        best = distance;
        index = i;
      }
    });
  }

  const text = shown === null ? "No limit" : format(shown);

  // Also on blur, not only on pointer-up: a drag released outside the window,
  // or a keyboard user tabbing away mid-adjustment, must not leave the value
  // shown and uncommitted.
  const release = () => {
    if (pending === undefined) return;
    const next = pending;
    setPending(undefined);
    if (next !== value) onChange(next);
  };

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
        onChange={(event) => {
          const next = steps[Number(event.target.value)] ?? null;
          if (commit === "release") setPending(next);
          else onChange(next);
        }}
        onPointerUp={release}
        onKeyUp={release}
        onBlur={release}
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
