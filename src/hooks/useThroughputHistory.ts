"use client";

import { useEffect, useRef, useState } from "react";

import { WINDOW_SIZE, type ThroughputSample } from "@/lib/chart";
import type { TelemetrySnapshot } from "@/lib/ipc/types";

/**
 * Keeps the last minute of session throughput.
 *
 * Held here rather than in the engine because it is presentation state: the
 * chart is the only thing that wants it, it is cheap to accumulate, and
 * pushing sixty samples across IPC every tick to redraw a chart that already
 * has fifty-nine of them would be per-tick waste for no gain.
 *
 * Samples are keyed on the session's uptime rather than on object identity.
 * React can re-run an effect with the same snapshot — Strict Mode does it
 * deliberately — and two consecutive ticks can carry byte-identical rates, so
 * comparing values cannot tell a repeat from a genuine second of steady
 * transfer. Uptime increments once per tick and is the only field that
 * reliably does.
 *
 * @param telemetry - The latest snapshot, or `null` before the first arrives.
 * @returns Up to {@link WINDOW_SIZE} samples, oldest first.
 */
export function useThroughputHistory(
  telemetry: TelemetrySnapshot | null,
): ThroughputSample[] {
  const [history, setHistory] = useState<ThroughputSample[]>([]);
  const lastUptime = useRef<number | null>(null);

  useEffect(() => {
    if (!telemetry) return;

    const { uptimeSeconds, downloadBps, uploadBps } = telemetry.core;
    if (lastUptime.current === uptimeSeconds) return;

    // A backwards jump means the engine restarted under us. The old samples
    // describe a session that no longer exists, so the window starts again
    // rather than splicing two sessions into one misleading line.
    const restarted =
      lastUptime.current !== null && uptimeSeconds < lastUptime.current;
    lastUptime.current = uptimeSeconds;

    const sample = { downBps: downloadBps, upBps: uploadBps };
    setHistory((current) =>
      restarted ? [sample] : [...current, sample].slice(-WINDOW_SIZE),
    );
  }, [telemetry]);

  return history;
}
