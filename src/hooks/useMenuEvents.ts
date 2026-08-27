"use client";

import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

/**
 * Event the Rust menu emits.
 *
 * Must match `MENU_EVENT` in `src-tauri/src/menu.rs`.
 */
export const MENU_EVENT = "flume://menu";

/** Menu items the frontend acts on. Must match the ids in `menu.rs`. */
export type MenuAction = "open_settings" | "add_torrent";

/**
 * Runs a handler when the native menu asks for something.
 *
 * The menu decides *that* the user wants settings; this decides *what that
 * means* — which is the frontend's business, because it is the frontend that
 * knows whether a dialog is already open. A Rust-side handler reaching into
 * the webview would put the same decision in two places.
 *
 * The handler is held in a ref so a caller can pass an inline closure without
 * the subscription tearing down and rebuilding on every render, which would
 * drop menu events fired in the gap.
 *
 * @param onAction - Called with the menu item's id.
 */
export function useMenuEvents(onAction: (action: MenuAction) => void): void {
  const handler = useRef(onAction);

  // Kept current in an effect rather than assigned during render: writing to a
  // ref while rendering is not safe under concurrent rendering, where a render
  // can be thrown away after the write has already happened.
  useEffect(() => {
    handler.current = onAction;
  }, [onAction]);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    void listen<string>(MENU_EVENT, (event) => {
      if (!active) return;
      handler.current(event.payload as MenuAction);
    }).then((stop) => {
      // The effect may have been cleaned up while `listen` was in flight.
      if (active) unlisten = stop;
      else stop();
    });

    return () => {
      active = false;
      unlisten?.();
    };
  }, []);
}
