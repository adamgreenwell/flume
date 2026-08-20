"use client";

import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";

import { getTelemetry } from "@/lib/ipc/client";
import { isCommandError, type TelemetrySnapshot } from "@/lib/ipc/types";

/**
 * Event name the Rust backend emits on.
 *
 * Must match `TELEMETRY_EVENT` in `src-tauri/src/telemetry.rs`.
 */
export const TELEMETRY_EVENT = "flume://telemetry";

/** What {@link useTelemetry} returns. */
export interface UseTelemetryResult {
  /** Latest snapshot, or `null` before the first one arrives. */
  telemetry: TelemetrySnapshot | null;
  /** Human-readable error from the initial fetch, or `null`. */
  error: string | null;
  /** True until the first snapshot arrives or the initial fetch fails. */
  isLoading: boolean;
}

/**
 * Subscribes to backend-pushed telemetry.
 *
 * The backend emits one batched payload per second covering the session and
 * every torrent. Subscribing rather than polling keeps IPC volume flat as the
 * torrent count grows — this replaced a polling hook precisely because
 * per-torrent polling would not scale.
 *
 * An initial `get_telemetry` call runs alongside the subscription so the first
 * paint does not wait up to a full tick. If that call loses the race with the
 * first pushed event, its result is discarded rather than overwriting newer
 * data.
 *
 * @returns The latest {@link UseTelemetryResult}.
 */
export function useTelemetry(): UseTelemetryResult {
  const [telemetry, setTelemetry] = useState<TelemetrySnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  // Tracks whether a pushed event has already landed, so the slower initial
  // fetch cannot clobber fresher data.
  const hasLiveDataRef = useRef(false);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    const subscribe = async () => {
      const stop = await listen<TelemetrySnapshot>(TELEMETRY_EVENT, (event) => {
        if (!active) return;
        hasLiveDataRef.current = true;
        setTelemetry(event.payload);
        setError(null);
        setIsLoading(false);
      });
      // The effect may have been cleaned up while `listen` was in flight.
      if (active) unlisten = stop;
      else stop();
    };

    const primeFirstPaint = async () => {
      try {
        const initial = await getTelemetry();
        if (!active || hasLiveDataRef.current) return;
        setTelemetry(initial);
        setError(null);
      } catch (caught: unknown) {
        if (!active || hasLiveDataRef.current) return;
        // The engine is usually just still starting; the next pushed event
        // will clear this.
        setError(
          isCommandError(caught)
            ? caught.message
            : "Could not reach the torrent engine.",
        );
      } finally {
        if (active && !hasLiveDataRef.current) setIsLoading(false);
      }
    };

    // `subscribe` can reject outright — notably when the page is open in a
    // plain browser rather than the Tauri webview, where `listen` has no IPC
    // internals to hook into. Unhandled, that surfaces as a console-level
    // unhandledRejection and the UI just sits there silently.
    void subscribe().catch(() => {
      if (!active) return;
      setError("Live updates are unavailable outside the Flume app.");
      setIsLoading(false);
    });
    void primeFirstPaint();

    return () => {
      active = false;
      // `UnlistenFn` is typed as returning void, but Tauri's implementation is
      // async and rejects when the event plugin is already gone — during app
      // shutdown, or a hot reload. Left bare that surfaces as an unhandled
      // rejection in the webview. `active` has already stopped us applying
      // events by this point, so a failed unsubscribe is not actionable.
      void Promise.resolve(unlisten?.()).catch(() => {});
    };
  }, []);

  return { telemetry, error, isLoading };
}
