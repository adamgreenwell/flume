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

/**
 * How often the first-paint fetch is retried while no snapshot has arrived.
 *
 * Matched to `TELEMETRY_INTERVAL` in `src-tauri/src/telemetry.rs`: when an
 * engine does exist, a pushed event lands within one tick and stops this after
 * a single extra call.
 */
const RETRY_INTERVAL_MS = 1000;

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

    /**
     * Asks for a snapshot, and reports whether asking again could help.
     *
     * `false` means nothing answered IPC at all — the page is open in a plain
     * browser rather than the Tauri webview — which is the one failure no
     * amount of waiting fixes. A `CommandError` is the opposite: something
     * answered and said why, and the why can change.
     */
    const primeFirstPaint = async (): Promise<boolean> => {
      try {
        const initial = await getTelemetry();
        if (!active || hasLiveDataRef.current) return true;
        setTelemetry(initial);
        setError(null);
        return true;
      } catch (caught: unknown) {
        const reachable = isCommandError(caught);
        if (!active || hasLiveDataRef.current) return reachable;
        // The engine is usually just still starting; the next pushed event
        // will clear this. When it is not, the message says what went wrong
        // instead, and the retry below is what lets it replace this one.
        setError(
          reachable ? caught.message : "Could not reach the torrent engine.",
        );
        return reachable;
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
    const attempt = () => {
      if (!active || hasLiveDataRef.current) return;
      void primeFirstPaint().then((reachable) => {
        // No IPC at all, so there is no backend whose answer could change —
        // Storybook, or `next dev` in a real browser. Left running, this would
        // also overwrite the message `subscribe` sets to explain exactly that,
        // once a second, forever.
        if (!reachable) clearInterval(retry);
      });
    };

    // Retried rather than asked once, because *why* there is no engine changes
    // after the first answer. A start that fails takes its whole deadline to
    // fail — up to `SESSION_START_TIMEOUT`, two minutes — so the first call
    // always returns "still starting", and a start that then times out has no
    // other way to say so: the backend swallows it (nothing is in flight to
    // return it to) and the telemetry loop emits nothing without an engine.
    // One fetch leaves that first sentence frozen on screen for good, along
    // with the message naming the torrent to remove. See #154.
    //
    // Bounded by the flag that already discards a late first paint: any pushed
    // snapshot means there is an engine, and this stops for good. So it runs
    // during startup, during a guard hold, and after a failed start — the three
    // states where the app is idle anyway — and never once data is flowing.
    //
    // Created before the first attempt, so an attempt that finds no IPC can
    // always cancel it rather than leaving one stray retry behind.
    const retry = setInterval(attempt, RETRY_INTERVAL_MS);
    attempt();

    return () => {
      active = false;
      // `UnlistenFn` is typed as returning void, but Tauri's implementation is
      // async and rejects when the event plugin is already gone — during app
      // shutdown, or a hot reload. Left bare that surfaces as an unhandled
      // rejection in the webview. `active` has already stopped us applying
      // events by this point, so a failed unsubscribe is not actionable.
      void Promise.resolve(unlisten?.()).catch(() => {});
      clearInterval(retry);
    };
  }, []);

  return { telemetry, error, isLoading };
}
