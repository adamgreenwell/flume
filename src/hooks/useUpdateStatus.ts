"use client";

import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";

import { getUpdateStatus } from "@/lib/ipc/client";
import type { UpdateStatus } from "@/lib/ipc/types";

/**
 * Event name the Rust backend emits on.
 *
 * Must match `UPDATE_EVENT` in `src-tauri/src/updates/checker.rs`.
 */
export const UPDATE_EVENT = "flume://update";

/**
 * Subscribes to what Flume knows about updates.
 *
 * Changes are pushed, and only on change -- a check runs once a day, so
 * there is no countdown to tick the way the guard's status does. An initial
 * `get_update_status` covers the first render; if a pushed event lands first,
 * the slower fetch is discarded rather than overwriting it. Both halves are
 * the pattern {@link useEgressGuard} settled on, for the same reasons.
 *
 * @returns The latest status, or `null` before the first one arrives.
 */
export function useUpdateStatus(): UpdateStatus | null {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const hasLiveDataRef = useRef(false);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    const subscribe = async () => {
      const stop = await listen<UpdateStatus>(UPDATE_EVENT, (event) => {
        if (!active) return;
        hasLiveDataRef.current = true;
        setStatus(event.payload);
      });
      if (active) {
        unlisten = stop;
      } else {
        stop();
      }
    };

    // Rejects in a plain browser, where `listen` has nothing to hook into.
    // Update status is supplementary; the app works without it.
    void subscribe().catch(() => {});

    void getUpdateStatus()
      .then((initial) => {
        if (!active || hasLiveDataRef.current) return;
        setStatus(initial);
      })
      .catch(() => {
        // Nothing to show is the right outcome: a missing update notice is
        // invisible, a wrong one is not.
      });

    return () => {
      active = false;
      void Promise.resolve(unlisten?.()).catch(() => {});
    };
  }, []);

  return status;
}
