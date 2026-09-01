"use client";

import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";

import { checkEgress } from "@/lib/ipc/client";
import type { GuardStatus } from "@/lib/ipc/types";

/**
 * Event name the Rust backend emits on.
 *
 * Must match `GUARD_EVENT` in `src-tauri/src/guard.rs`.
 */
export const EGRESS_EVENT = "flume://egress";

/** What {@link useEgressGuard} returns. */
export interface UseEgressGuardResult {
  /** Latest status, or `null` before the first one arrives. */
  status: GuardStatus | null;
  /**
   * Whether transfer is being held right now.
   *
   * Convenience for the many places that only need the boolean, and safe
   * before the first event: `false` while unknown, because a UI that announced
   * a hold that is not happening would be worse than one that is a second
   * late.
   */
  held: boolean;
}

/**
 * Subscribes to the egress guard's published status.
 *
 * The backend emits once a second whether or not anything changed, because the
 * settle countdown has to tick down on screen — a change-only emitter would
 * show "resumes in 10 s" and then nothing until it resumed.
 *
 * This is deliberately separate from {@link useTelemetry}. Telemetry stops
 * entirely while the guard is holding, since holding means no torrent session
 * exists at all; the guard's own status is the one thing still arriving, and it
 * is what explains the silence.
 *
 * An initial `check_egress` call runs alongside the subscription so the first
 * paint does not wait up to a full tick. If it loses the race with a pushed
 * event its result is discarded rather than overwriting newer data.
 *
 * @returns The latest {@link UseEgressGuardResult}.
 */
export function useEgressGuard(): UseEgressGuardResult {
  const [status, setStatus] = useState<GuardStatus | null>(null);

  // Tracks whether a pushed event has already landed, so the slower initial
  // fetch cannot clobber fresher data.
  const hasLiveDataRef = useRef(false);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    const subscribe = async () => {
      const stop = await listen<GuardStatus>(EGRESS_EVENT, (event) => {
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

    // `subscribe` can reject outright — notably when the page is open in a
    // plain browser rather than the Tauri webview, where `listen` has no IPC
    // internals to hook into. Left unhandled it surfaces as an
    // unhandledRejection, which is how this arrived as six errors beside six
    // passing tests. `useTelemetry` documents the same trap.
    void subscribe().catch(() => {
      // Nothing to show. The guard's status is supplementary: the rest of the
      // UI works without it, and claiming a hold we cannot confirm would be
      // worse than saying nothing.
    });

    // The backend publishes a status during startup, before the first tick, so
    // this never has to invent one.
    void checkEgress()
      .then((initial) => {
        if (!active || hasLiveDataRef.current) return;
        setStatus(initial);
      })
      .catch(() => {
        // Swallowed on purpose. The subscription is the real source; a failed
        // first fetch resolves itself within a second, and surfacing it would
        // put an error on screen for a condition that has already passed.
      });

    return () => {
      active = false;
      // `UnlistenFn` is typed as returning void, but Tauri's implementation is
      // async and rejects when the event plugin is already gone — during
      // shutdown, or a hot reload. `active` has already stopped us applying
      // events by this point, so a failed unsubscribe is not actionable.
      void Promise.resolve(unlisten?.()).catch(() => {});
    };
  }, []);

  return { status, held: status?.held ?? false };
}
