"use client";

import type { Bottleneck, LimitFactor } from "@/lib/ipc/types";

/** Props for {@link FactorRow}. */
interface FactorRowProps {
  factor: LimitFactor;
}

/**
 * One constraint: name, bar, value, verdict.
 *
 * The bar is omitted rather than drawn empty when `utilisation` is `null`. An
 * empty bar reads as "plenty of headroom", which is a claim — and the whole
 * reason the field is nullable is that Flume cannot make it.
 */
function FactorRow({ factor }: FactorRowProps) {
  const { name, utilisation, value, binding } = factor;

  return (
    <div className="flex items-center gap-3 py-[5px]">
      <div className="w-[132px] shrink-0 truncate text-[12px]">{name}</div>

      <div className="h-[6px] grow overflow-hidden rounded-full">
        {utilisation === null ? (
          // Dashes, not an empty track: this is "not measured", which is a
          // different statement from "measured at zero".
          <div
            className="border-line-2 h-full w-full rounded-full border border-dashed"
            title="Flume cannot measure a ceiling for this"
          />
        ) : (
          <div
            className="bg-bg-3 h-full w-full rounded-full"
            role="progressbar"
            aria-valuenow={Math.round(utilisation)}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={`${name} utilisation`}
          >
            <div
              className={`h-full rounded-full ${binding ? "bg-warn" : "bg-acc"}`}
              style={{ width: `${Math.min(100, Math.max(0, utilisation))}%` }}
            />
          </div>
        )}
      </div>

      <div className="flume-num text-fg-1 w-[152px] shrink-0 truncate text-right text-[11.5px]">
        {value}
      </div>

      <div className="w-[104px] shrink-0 text-right">
        <span
          className={`inline-flex h-[21px] items-center rounded-sm px-2 text-[11px] font-medium whitespace-nowrap ${
            binding ? "text-warn bg-warn/15" : "text-fg-3 bg-bg-2"
          }`}
        >
          {binding
            ? "Limiting now"
            : utilisation === null
              ? "Not measured"
              : "Headroom"}
        </span>
      </div>
    </div>
  );
}

/** Props for {@link BottleneckPanel}. */
export interface BottleneckPanelProps {
  /** The ranking, or `null` when the torrent is not downloading. */
  bottleneck: Bottleneck | null;
}

/**
 * What is limiting this download.
 *
 * The ranking is computed in Rust — this only draws it. That is deliberate:
 * the panel's promise is that at most one factor is marked binding, and a
 * promise enforced in one place is a promise, while one enforced in two is a
 * pair of things that drift.
 *
 * Renders nothing when there is no ranking. A paused or seeding torrent is not
 * being limited, and a panel saying so would be answering a question nobody
 * asked.
 *
 * @param props - See {@link BottleneckPanelProps}.
 * @returns The rendered panel, or `null`.
 */
export function BottleneckPanel({ bottleneck }: BottleneckPanelProps) {
  if (bottleneck === null) return null;

  return (
    <section className="border-line bg-bg-1 rounded-lg border p-4">
      <h3 className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
        What is limiting this download
      </h3>

      <div className="mt-3">
        {bottleneck.factors.map((factor) => (
          <FactorRow key={factor.name} factor={factor} />
        ))}
      </div>

      <p className="text-fg-1 border-line mt-3 border-t pt-3 text-[12px] leading-[1.55]">
        {bottleneck.explanation}
      </p>
    </section>
  );
}
