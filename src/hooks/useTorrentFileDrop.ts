"use client";

import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef, useState } from "react";

/** Only `.torrent` files can be dropped; anything else is ignored. */
const TORRENT_EXTENSION = ".torrent";

/** What {@link useTorrentFileDrop} returns. */
export interface UseTorrentFileDropResult {
  /** True while a drag carrying at least one `.torrent` file is over the window. */
  isDraggingTorrent: boolean;
}

/**
 * Subscribes to OS-level file drops on the window.
 *
 * Tauri reports drops as *paths*, which is exactly what the engine wants — no
 * file contents cross the IPC boundary, and the webview needs no filesystem
 * permission.
 *
 * Non-`.torrent` files are ignored rather than reported as an error: dragging
 * something onto a window is easy to do by accident, and an error dialog for a
 * misdirected drag is noise.
 *
 * @param onDrop - Called with each dropped `.torrent` path.
 * @returns Drag state for rendering an affordance.
 */
export function useTorrentFileDrop(
  onDrop: (path: string) => void,
): UseTorrentFileDropResult {
  const [isDraggingTorrent, setIsDraggingTorrent] = useState(false);

  // Kept in a ref so a changing callback identity does not tear down and
  // re-establish the OS-level subscription on every render.
  const onDropRef = useRef(onDrop);
  useEffect(() => {
    onDropRef.current = onDrop;
  }, [onDrop]);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    const isTorrent = (path: string) =>
      path.toLowerCase().endsWith(TORRENT_EXTENSION);

    const subscribe = async () => {
      const stop = await getCurrentWebview().onDragDropEvent((event) => {
        if (!active) return;
        const payload = event.payload;

        if (payload.type === "enter") {
          setIsDraggingTorrent(payload.paths.some(isTorrent));
        } else if (payload.type === "leave") {
          setIsDraggingTorrent(false);
        } else if (payload.type === "drop") {
          setIsDraggingTorrent(false);
          // Only the first torrent: the add flow is a single-torrent dialog,
          // and silently queueing several would be surprising.
          const first = payload.paths.find(isTorrent);
          if (first) onDropRef.current(first);
        }
      });
      if (active) unlisten = stop;
      else stop();
    };

    // Outside the Tauri webview there is no drag-drop plumbing; the app simply
    // does not support dropping there, which is not worth surfacing.
    void subscribe().catch(() => {});

    return () => {
      active = false;
      // Typed as returning void but actually async; see useTelemetry.
      void Promise.resolve(unlisten?.()).catch(() => {});
    };
  }, []);

  return { isDraggingTorrent };
}
