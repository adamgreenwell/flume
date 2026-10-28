"use client";

import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";

import { getStatistics } from "@/lib/ipc/client";
import type { Statistics } from "@/lib/ipc/types";

/**
 * Event name the Rust backend emits on.
 *
 * Must match `STATISTICS_EVENT` in `src-tauri/src/telemetry.rs`.
 */
export const STATISTICS_EVENT = "flume://statistics";

/**
 * Subscribes to the lifetime and this-launch transfer totals.
 *
 * Pushed every second, like telemetry, but on its own event: telemetry stops
 * while the egress guard holds, and the time-open figures should not. An
 * initial `get_statistics` covers the first render, and is discarded if a
 * pushed event lands first -- the same arrangement as {@link useUpdateStatus}.
 *
 * Mounted only while the statistics view is open, so nothing listens the rest
 * of the time.
 *
 * @returns The latest totals, or `null` before the first arrive.
 */
export function useStatistics(): Statistics | null {
  const [statistics, setStatistics] = useState<Statistics | null>(null);
  const hasLiveDataRef = useRef(false);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    const subscribe = async () => {
      const stop = await listen<Statistics>(STATISTICS_EVENT, (event) => {
        if (!active) return;
        hasLiveDataRef.current = true;
        setStatistics(event.payload);
      });
      if (active) {
        unlisten = stop;
      } else {
        stop();
      }
    };

    // Rejects in a plain browser, where `listen` has nothing to hook into.
    void subscribe().catch(() => {});

    void getStatistics()
      .then((initial) => {
        if (!active || hasLiveDataRef.current) return;
        setStatistics(initial);
      })
      .catch(() => {
        // The view shows its loading state; there is nothing better to say.
      });

    return () => {
      active = false;
      void Promise.resolve(unlisten?.()).catch(() => {});
    };
  }, []);

  return statistics;
}
