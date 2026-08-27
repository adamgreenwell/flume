"use client";

import { useEffect, useRef, useState } from "react";

import { WINDOW_SIZE, type ThroughputSample } from "@/lib/chart";
import type { TelemetrySnapshot } from "@/lib/ipc/types";

/**
 * Keeps the last minute of a rate pair.
 *
 * Held in the frontend rather than the engine because it is presentation
 * state: a chart is the only thing that wants it, it is cheap to accumulate,
 * and pushing sixty samples across IPC every tick to redraw a chart that
 * already has fifty-nine of them would be per-tick waste for no gain.
 *
 * `tick` is what makes a sample a sample. React can re-run an effect with the
 * same values — Strict Mode does it deliberately — and two consecutive seconds
 * of steady transfer carry byte-identical rates, so comparing values cannot
 * tell a repeat from a genuine second. The caller passes something that
 * increments once per real tick instead; the session's uptime is the only
 * field that reliably does.
 *
 * @param downBps - Download rate for this tick.
 * @param upBps - Upload rate for this tick.
 * @param tick - Monotonic per-tick key, or `null` before the first tick.
 * @returns Up to {@link WINDOW_SIZE} samples, oldest first.
 */
export function useRateHistory(
  downBps: number,
  upBps: number,
  tick: number | null,
): ThroughputSample[] {
  const [history, setHistory] = useState<ThroughputSample[]>([]);
  const lastTick = useRef<number | null>(null);

  useEffect(() => {
    if (tick === null || lastTick.current === tick) return;

    // A backwards jump means the engine restarted under us, or the caller
    // switched to a different subject. Either way the old samples describe
    // something that is no longer on screen, so the window starts again rather
    // than splicing two series into one misleading line.
    const restarted = lastTick.current !== null && tick < lastTick.current;
    lastTick.current = tick;

    const sample = { downBps, upBps };
    setHistory((current) =>
      restarted ? [sample] : [...current, sample].slice(-WINDOW_SIZE),
    );
  }, [downBps, upBps, tick]);

  return history;
}

/**
 * The last minute of session-wide throughput.
 *
 * @param telemetry - The latest snapshot, or `null` before the first arrives.
 * @returns Up to {@link WINDOW_SIZE} samples, oldest first.
 */
export function useThroughputHistory(
  telemetry: TelemetrySnapshot | null,
): ThroughputSample[] {
  return useRateHistory(
    telemetry?.core.downloadBps ?? 0,
    telemetry?.core.uploadBps ?? 0,
    telemetry?.core.uptimeSeconds ?? null,
  );
}
