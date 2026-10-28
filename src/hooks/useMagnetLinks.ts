"use client";

import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

import { takePendingLinks } from "@/lib/ipc/client";

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
 * Subscribe, *then* take what Rust held. A link that launches Flume arrives
 * before this page exists, and Tauri does not buffer events, so Rust holds
 * links until it is told the window is listening (#241). Asking before
 * subscribing would leave a gap in which a link is emitted to nobody.
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

      // Delivered even if this effect has since been cleaned up. Rust hands
      // held links over exactly once, so an effect that ran twice -- React
      // does that on purpose in development -- must not discard them; the
      // ref reaches whichever handler is current.
      for (const link of await takePendingLinks()) {
        handlerRef.current(link);
      }
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
