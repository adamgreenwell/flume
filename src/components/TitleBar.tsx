"use client";

import { formatSpeed } from "@/lib/format";

import { Icon } from "./Icon";

/**
 * Which side of the title bar the OS draws its window buttons on.
 *
 * This inset is the only thing in the entire app that differs between
 * platforms — everything inside the window is byte-identical on all three.
 */
export type WindowControls = "left" | "right";

/** Props for {@link TitleBar}. */
export interface TitleBarProps {
  /**
   * Where the platform draws minimise/maximise/close.
   *
   * macOS puts its traffic lights at the left and needs 88px reserved there;
   * Windows and Linux put theirs at the right and need 138px.
   */
  controls: WindowControls;
  /** Session-wide download rate, bytes per second. */
  downloadBps: number;
  /** Session-wide upload rate, bytes per second. */
  uploadBps: number;
}

/**
 * The app's own 44px title bar.
 *
 * Flume draws its own on all three platforms rather than using the system one,
 * so the window has a single visual language. The reserved inset is the price
 * of that, and it is the only per-platform branch in the UI.
 *
 * @param props - See {@link TitleBarProps}.
 * @returns The rendered title bar.
 */
export function TitleBar({ controls, downloadBps, uploadBps }: TitleBarProps) {
  return (
    <div
      className="bg-bg-1 border-line col-span-full flex h-11 items-center gap-3.5 border-b"
      style={{
        paddingLeft: controls === "left" ? 88 : 14,
        paddingRight: controls === "right" ? 138 : 14,
      }}
      data-tauri-drag-region
    >
      {/*
        Every child is click-through so the drag region actually receives the
        press. Tauri starts a window drag only when the clicked element itself
        carries `data-tauri-drag-region`; with the attribute on the container
        alone, pressing on the wordmark or the rates hit a child and the window
        would not move. Nothing in this bar is interactive, so making the lot
        transparent to the pointer is simpler than repeating the attribute and
        cannot drift as content is added.
      */}
      <span className="text-fg-2 pointer-events-none text-xs font-medium tracking-[0.01em]">
        Flume
      </span>
      <span className="pointer-events-none grow" />
      <span className="text-fg-1 pointer-events-none flex items-center gap-1.5 text-[11.5px]">
        {/*
          Both directions are labelled for assistive tech and told apart by
          glyph, not only by colour. The two series converge under tritanopia,
          so colour alone can never carry this distinction.
        */}
        <span className="text-chart-down flex items-center gap-1.5">
          <Icon name="arrow-down" size={13} />
          <span className="sr-only">Download</span>
        </span>
        <span className="flume-num">{formatSpeed(downloadBps)}</span>
        <span className="text-chart-up ml-2 flex items-center gap-1.5">
          <Icon name="arrow-up" size={13} />
          <span className="sr-only">Upload</span>
        </span>
        <span className="flume-num">{formatSpeed(uploadBps)}</span>
      </span>
    </div>
  );
}
