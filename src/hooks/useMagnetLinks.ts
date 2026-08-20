"use client";

import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

/**
 * Event the backend emits when the OS hands Flume a magnet link.
 *
 * Must match `OPEN_MAGNET_EVENT` in `src-tauri/src/deeplink.rs`.
 */
export const OPEN_MAGNET_EVENT = "flume://open-magnet";

/**
 * Subscribes to magnet links arriving from outside the app.
 *
 * Fires when the user clicks a magnet link in a browser, or launches Flume
 * with one on the command line. Both paths route through the backend so a
 * second launch focuses the running window rather than starting a rival
 * engine that would fight over the listen port.
 *
 * @param onMagnet - Called with each magnet URI received.
 */
export function useMagnetLinks(onMagnet: (uri: string) => void): void {
  // Held in a ref so a changing callback identity does not tear down and
  // re-establish the subscription, which could drop a link arriving mid-swap.
  const handlerRef = useRef(onMagnet);
  useEffect(() => {
    handlerRef.current = onMagnet;
  }, [onMagnet]);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    const subscribe = async () => {
      const stop = await listen<string>(OPEN_MAGNET_EVENT, (event) => {
        if (active) handlerRef.current(event.payload);
      });
      if (active) unlisten = stop;
      else stop();
    };

    // Outside the Tauri webview there is no event plumbing; deep links simply
    // do not apply there.
    void subscribe().catch(() => {});

    return () => {
      active = false;
      // Typed as returning void but actually async; see useTelemetry.
      void Promise.resolve(unlisten?.()).catch(() => {});
    };
  }, []);
}
