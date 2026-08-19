"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { getCoreStatus } from "@/lib/ipc/client";
import { isCommandError, type CoreStatus } from "@/lib/ipc/types";

/** Telemetry poll interval, matching the ~1 Hz budget in the architecture notes. */
const POLL_INTERVAL_MS = 1000;

/** What {@link useCoreStatus} returns. */
export interface UseCoreStatusResult {
  /** Latest snapshot, or `null` before the first successful poll. */
  status: CoreStatus | null;
  /** Human-readable error from the most recent poll, or `null`. */
  error: string | null;
  /** True until the first poll settles, whether it succeeds or fails. */
  isLoading: boolean;
}

/**
 * Polls the Rust backend for engine status at roughly 1 Hz.
 *
 * Polling rather than subscribing is deliberate for Phase 0: it is the
 * smallest thing that proves the `invoke` path works end to end. Phase 1
 * replaces this with backend-pushed events, which scale better once there are
 * many torrents.
 *
 * The interval is chained rather than fixed (`setTimeout` after each settle,
 * not `setInterval`) so a slow call cannot stack up overlapping requests.
 *
 * @returns The latest {@link UseCoreStatusResult}.
 */
export function useCoreStatus(): UseCoreStatusResult {
  const [status, setStatus] = useState<CoreStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  // Guards against setting state after unmount, and stops the chained timer.
  const activeRef = useRef(true);

  const poll = useCallback(async () => {
    try {
      const next = await getCoreStatus();
      if (!activeRef.current) return;
      setStatus(next);
      setError(null);
    } catch (caught: unknown) {
      if (!activeRef.current) return;
      setError(
        isCommandError(caught)
          ? caught.message
          : "Could not reach the torrent engine.",
      );
    } finally {
      if (activeRef.current) setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    activeRef.current = true;
    let timer: ReturnType<typeof setTimeout>;

    const tick = async () => {
      await poll();
      if (activeRef.current) timer = setTimeout(tick, POLL_INTERVAL_MS);
    };
    void tick();

    return () => {
      activeRef.current = false;
      clearTimeout(timer);
    };
  }, [poll]);

  return { status, error, isLoading };
}
