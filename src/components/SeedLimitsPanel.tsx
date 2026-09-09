"use client";

import { useCallback, useEffect, useState } from "react";

import { getSeedLimits, setTorrentRules } from "@/lib/ipc/client";
import { formatDuration } from "@/lib/format";
import {
  isCommandError,
  type SeedLimits,
  type TorrentRules,
} from "@/lib/ipc/types";

import { RATIO_STEPS, SEED_TIME_STEPS, SteppedSlider } from "./SteppedSlider";
import { Skeleton } from "./Skeleton";

/** Renders a set of limits as a sentence. */
function describe(rules: TorrentRules): string {
  const { seedRatioLimit: ratio, seedTimeLimitSecs: secs } = rules;

  if (ratio === null && secs === null) return "seed with no limit";
  if (ratio !== null && secs === null)
    return `stop at a ratio of ${ratio.toFixed(2)}`;
  if (ratio === null && secs !== null)
    return `stop after ${formatDuration(secs)} of seeding`;
  return `stop at a ratio of ${ratio!.toFixed(2)} or after ${formatDuration(
    secs!,
  )}, whichever comes first`;
}

/** What this panel needs. */
export interface SeedLimitsPanelProps {
  /** The torrent whose limits these are. */
  infoHash: string;
}

/**
 * Sets one torrent's seed limits, or returns it to the global ones.
 *
 * The two states are a deliberate either/or rather than a set of fields with
 * a "use global" checkbox beside each. That mirrors how the rules actually
 * work: an override replaces the globals **wholesale**, so a torrent cannot
 * take its ratio from here and its seed time from settings. A per-field UI
 * would promise exactly that and quietly not deliver it.
 *
 * It is also what makes "seed this one forever" expressible: an override with
 * both limits cleared. That is the same thing the Keep seeding button writes.
 *
 * @param props - See {@link SeedLimitsPanelProps}.
 * @returns The rendered panel.
 */
export function SeedLimitsPanel({ infoHash }: SeedLimitsPanelProps) {
  // The hash the stored result belongs to is kept alongside it, so switching
  // torrents is handled by *deriving* a null rather than clearing state inside
  // an effect — the same shape `useTorrentDetail` uses, and for the same
  // reason: clearing there sets state during render's commit and cascades an
  // extra render every time the inspector opens.
  const [state, setState] = useState<{
    infoHash: string;
    limits: SeedLimits | null;
    error: string | null;
  } | null>(null);

  useEffect(() => {
    let live = true;

    // Fetched once rather than polled, unlike the detail beside it. These
    // change only when someone changes them here, and this panel is the only
    // thing that can.
    void (async () => {
      try {
        const next = await getSeedLimits(infoHash);
        if (live) setState({ infoHash, limits: next, error: null });
      } catch (caught: unknown) {
        if (!live) return;
        setState({
          infoHash,
          limits: null,
          error: isCommandError(caught)
            ? caught.message
            : "Could not read this torrent's seed limits.",
        });
      }
    })();

    return () => {
      live = false;
    };
  }, [infoHash]);

  const write = useCallback(
    async (rules: TorrentRules | null) => {
      // Applied locally first. The value is the user's own input, the write
      // cannot reject it, and a slider that waits a round trip to move reads
      // as broken. A failure puts the message up and the next open re-reads
      // the truth from disk.
      setState((current) =>
        current?.limits
          ? {
              ...current,
              limits: { ...current.limits, torrent: rules },
              error: null,
            }
          : current,
      );
      try {
        await setTorrentRules(infoHash, rules);
      } catch (caught: unknown) {
        setState((current) =>
          current
            ? {
                ...current,
                error: isCommandError(caught)
                  ? caught.message
                  : "Could not save this torrent's seed limits.",
              }
            : current,
        );
      }
    },
    [infoHash],
  );

  // A result for a different torrent is not this torrent's result.
  const settled = state !== null && state.infoHash === infoHash ? state : null;
  const limits = settled?.limits ?? null;
  const error = settled?.error ?? null;

  if (error !== null && limits === null) {
    return (
      <section className="border-line bg-bg-1 flex flex-col gap-3 rounded-lg border p-5">
        <div className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
          Seed limits
        </div>
        <p className="text-err text-[11.5px]" role="alert">
          {error}
        </p>
      </section>
    );
  }

  if (limits === null) {
    return (
      <section className="border-line bg-bg-1 flex flex-col gap-3 rounded-lg border p-5">
        <div className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
          Seed limits
        </div>
        <Skeleton rows={2} label="Loading seed limits" />
      </section>
    );
  }

  const override = limits.torrent;
  const following = override === null;

  return (
    <section className="border-line bg-bg-1 flex flex-col gap-3 rounded-lg border p-5">
      <div className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
        Seed limits
      </div>

      <div
        role="radiogroup"
        aria-label="Seed limits for this torrent"
        className="border-line bg-bg-2 flex rounded-md border p-0.5"
      >
        <button
          type="button"
          role="radio"
          aria-checked={following}
          onClick={() => void write(null)}
          className={`h-[26px] grow rounded-[4px] text-[11.5px] ${
            following ? "bg-bg-0 text-fg-0" : "text-fg-2 hover:text-fg-0"
          }`}
        >
          Follow global
        </button>
        <button
          type="button"
          role="radio"
          aria-checked={!following}
          // Seeded from the globals rather than from nothing, so switching to
          // "just this one" starts where the torrent already was instead of
          // silently lifting every limit it had.
          onClick={() => void write(limits.global)}
          className={`h-[26px] grow rounded-[4px] text-[11.5px] ${
            following ? "text-fg-2 hover:text-fg-0" : "bg-bg-0 text-fg-0"
          }`}
        >
          Just this torrent
        </button>
      </div>

      {following ? (
        <p className="text-fg-2 text-[11.5px]">
          This torrent follows your global limits, so it will{" "}
          {describe(limits.global)}. Changing them in Settings changes it here
          too.
        </p>
      ) : (
        <>
          <div className="flex flex-col gap-2.5">
            <div className="flex items-center justify-between gap-3">
              <span className="text-fg-2 shrink-0 text-[11.5px]">
                Stop at a ratio of
              </span>
              <SteppedSlider
                steps={RATIO_STEPS}
                value={override.seedRatioLimit}
                label="Stop this torrent at a ratio of"
                format={(ratio) => ratio.toFixed(2)}
                onChange={(next) =>
                  void write({ ...override, seedRatioLimit: next })
                }
                width="w-[76px]"
              />
            </div>
            <div className="flex items-center justify-between gap-3">
              <span className="text-fg-2 shrink-0 text-[11.5px]">
                Stop after
              </span>
              <SteppedSlider
                steps={SEED_TIME_STEPS}
                value={override.seedTimeLimitSecs}
                label="Stop this torrent after"
                format={formatDuration}
                onChange={(next) =>
                  void write({ ...override, seedTimeLimitSecs: next })
                }
                width="w-[86px]"
              />
            </div>
          </div>
          <p className="text-fg-2 text-[11.5px]">
            This torrent will {describe(override)}, whatever the global limits
            say.
          </p>
        </>
      )}

      {error !== null ? (
        <p className="text-err text-[11.5px]" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}
