"use client";

import { useCallback, useRef, useState } from "react";

import {
  type ColumnId,
  type ResolvedWidths,
  columnStyle,
  minRowWidth,
  resolveWidths,
  toSaved,
  widthVar,
} from "@/lib/columns";
import type { ColumnWidths } from "@/lib/ipc/types";

/** What {@link useColumnWidths} hands the list. */
export interface ColumnWidthsControl {
  /** Every resizable column's committed width. */
  widths: ResolvedWidths;
  /** Custom properties for the list container. */
  style: Record<string, string>;
  /** Attach to the list container, the element `style` is applied to. */
  containerRef: React.RefObject<HTMLDivElement | null>;
  /** Repaint a column mid-drag without a React render. */
  onResizing: (id: ColumnId, width: number) => void;
  /** Keep a width: render it and persist it. */
  onResized: (id: ColumnId, width: number) => void;
  /** Adopt widths read from settings. */
  load: (saved: ColumnWidths | null | undefined) => void;
}

/**
 * Column widths for the torrent list (#147).
 *
 * A drag writes the custom property straight onto the container on every
 * pointer move and touches React state only when it ends. Rows re-render on
 * the 1 Hz telemetry tick anyway; re-rendering every row on every pointer
 * move as well would make the drag stutter in exactly the large libraries
 * where someone is most likely to be adjusting columns.
 *
 * @param persist - Writes the new value of `Settings.columns`.
 * @returns See {@link ColumnWidthsControl}.
 */
export function useColumnWidths(
  persist: (columns: ColumnWidths) => void,
): ColumnWidthsControl {
  const [widths, setWidths] = useState<ResolvedWidths>(() =>
    resolveWidths(null),
  );
  const containerRef = useRef<HTMLDivElement | null>(null);
  // The widths as last painted, including a drag in progress, so the row
  // minimum can be recomputed without waiting for a render.
  const painted = useRef<ResolvedWidths>(widths);

  const onResizing = useCallback((id: ColumnId, width: number) => {
    painted.current = { ...painted.current, [id]: width };
    const el = containerRef.current;
    if (!el) return;
    el.style.setProperty(widthVar(id), `${width}px`);
    el.style.setProperty(
      "--flume-list-min-w",
      `${minRowWidth(painted.current)}px`,
    );
  }, []);

  const onResized = useCallback(
    (id: ColumnId, width: number) => {
      const next = { ...painted.current, [id]: width };
      painted.current = next;
      setWidths(next);
      persist(toSaved(next));
    },
    [persist],
  );

  const load = useCallback((saved: ColumnWidths | null | undefined) => {
    const next = resolveWidths(saved);
    painted.current = next;
    setWidths(next);
  }, []);

  return {
    widths,
    style: columnStyle(widths),
    containerRef,
    onResizing,
    onResized,
    load,
  };
}
