"use client";

import { useEffect } from "react";

import { useStatistics } from "@/hooks/useStatistics";
import { formatBytes, formatDuration } from "@/lib/format";
import type { Statistics, Totals } from "@/lib/ipc/types";

import { Icon } from "./Icon";

/**
 * Upload over download, to two places, or a dash when nothing has come down.
 *
 * A dash rather than the dock's zero: this view is about a span long enough
 * that "nothing downloaded" is a real answer, and a ratio of zero would claim
 * it had been measured.
 *
 * @param totals - The span to compute it over.
 * @returns The ratio as display text.
 */
export function shareRatio(totals: Totals): string {
  if (totals.downloadedBytes === 0) return "—";
  return (totals.uploadedBytes / totals.downloadedBytes).toFixed(2);
}

const ROWS: { label: string; value: (t: Totals) => string }[] = [
  { label: "Downloaded", value: (t) => formatBytes(t.downloadedBytes) },
  { label: "Uploaded", value: (t) => formatBytes(t.uploadedBytes) },
  { label: "Share ratio", value: shareRatio },
  { label: "Time open", value: (t) => formatDuration(t.activeSeconds) },
];

/** Props for {@link StatisticsTable}. */
export interface StatisticsTableProps {
  /** The totals, or `null` before they arrive. */
  statistics: Statistics | null;
}

/**
 * The figures themselves: this launch beside all time.
 *
 * Separate from the dialog so it can be rendered and tested without the
 * subscription behind it.
 *
 * @param props - See {@link StatisticsTableProps}.
 * @returns The rendered table.
 */
export function StatisticsTable({ statistics }: StatisticsTableProps) {
  const cell = (span: "session" | "allTime", value: (t: Totals) => string) =>
    statistics ? value(statistics[span]) : "—";

  return (
    <>
      <table className="w-full text-[12.5px]">
        <thead>
          <tr className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
            <td />
            <th scope="col" className="pb-2 text-right font-semibold">
              This session
            </th>
            <th scope="col" className="pb-2 text-right font-semibold">
              All time
            </th>
          </tr>
        </thead>
        <tbody>
          {ROWS.map(({ label, value }) => (
            <tr key={label} className="border-line border-t">
              <th scope="row" className="text-fg-2 py-2 text-left font-normal">
                {label}
              </th>
              <td className="flume-num text-fg-0 py-2 text-right">
                {cell("session", value)}
              </td>
              <td className="flume-num text-fg-0 py-2 text-right">
                {cell("allTime", value)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="text-fg-2 text-[11.5px] leading-[1.5]">
        {statistics ? (
          <>
            Launched{" "}
            <span className="flume-num">
              {statistics.launches.toLocaleString()}
            </span>{" "}
            {statistics.launches === 1 ? "time" : "times"}. Time open counts
            while the tunnel guard holds, and stops while the machine sleeps.
          </>
        ) : (
          "Reading totals…"
        )}
      </p>
    </>
  );
}

/** Props for {@link StatisticsDialog}. */
export interface StatisticsDialogProps {
  /** Called when the user closes the dialog. */
  onClose: () => void;
}

/**
 * Lifetime transfer totals, opened from Window › Statistics.
 *
 * Live while open: the totals tick every second, on their own event, so they
 * keep moving while the egress guard holds and the rest of the window is
 * dashes.
 *
 * @param props - See {@link StatisticsDialogProps}.
 * @returns The rendered dialog.
 */
export function StatisticsDialog({ onClose }: StatisticsDialogProps) {
  const statistics = useStatistics();

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Statistics"
        className="border-line bg-bg-1 flex w-full max-w-md flex-col gap-4 rounded-xl border p-5 shadow-2xl"
      >
        <div className="flex items-center">
          <h2 className="text-fg-0 text-base font-semibold">Statistics</h2>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close statistics"
            className="text-fg-2 hover:text-fg-0 ml-auto"
            autoFocus
          >
            <Icon name="plus" size={16} className="rotate-45" />
          </button>
        </div>
        <StatisticsTable statistics={statistics} />
      </div>
    </div>
  );
}
