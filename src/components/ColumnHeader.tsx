"use client";

import { useRef } from "react";

import {
  COLUMNS,
  MAX_COLUMN_WIDTH,
  STATE_WIDTH,
  clampWidth,
  type ColumnDef,
  type ColumnId,
  type ResolvedWidths,
  widthVar,
} from "@/lib/columns";
import type { SortBy, SortState } from "@/lib/ipc/types";

import { Icon } from "./Icon";

/** How far one arrow-key press moves a divider; Shift moves it four times as far. */
const KEY_STEP = 8;

/** Props for {@link ColumnHeader}. */
export interface ColumnHeaderProps {
  /** The sort in force, so the sorted header can say so. */
  sort: SortState;
  /** A header was clicked: sort by its key. */
  onSort: (by: SortBy) => void;
  /** Every resizable column's current width. */
  widths: ResolvedWidths;
  /**
   * A divider is moving. Called on every pointer move, so the handler
   * should repaint through the CSS custom property rather than through
   * React state -- see `useColumnWidths`.
   */
  onResizing: (id: ColumnId, width: number) => void;
  /** A drag or a key press finished: keep this width. */
  onResized: (id: ColumnId, width: number) => void;
}

/**
 * The 28px header above the list.
 *
 * Every header but the state dot sorts by its column when clicked, and
 * clicking the sorted one flips it (#147). Every column but the dot and the
 * name can be resized by its left edge. Widths come from the CSS custom
 * properties in `src/lib/columns.ts`, which the rows read too, so the header
 * and the rows cannot drift apart.
 *
 * Marked up as a real `row` of `columnheader`s so the grid a screen reader
 * sees has headers to announce as it moves across a torrent, with
 * `aria-sort` on the one the list is ordered by.
 *
 * @param props - See {@link ColumnHeaderProps}.
 * @returns The rendered header.
 */
export function ColumnHeader({
  sort,
  onSort,
  widths,
  onResizing,
  onResized,
}: ColumnHeaderProps) {
  return (
    <div
      role="row"
      className="border-line bg-bg-0 text-fg-3 sticky top-0 z-10 flex h-7 min-w-[var(--flume-list-min-w)] shrink-0 items-center gap-4 border-b px-[18px] text-[10px] font-semibold tracking-[0.09em] uppercase"
    >
      <span
        role="columnheader"
        className="shrink-0"
        style={{ width: STATE_WIDTH }}
      >
        <span className="sr-only">State</span>
      </span>
      <span
        role="columnheader"
        aria-sort={ariaSort(sort, "name")}
        className="min-w-0 grow"
      >
        <SortButton label="Name" by="name" sort={sort} onSort={onSort} />
      </span>
      {COLUMNS.map((c) => (
        <span
          key={c.id}
          role="columnheader"
          aria-sort={ariaSort(sort, c.sortBy)}
          className={`relative flex shrink-0 ${
            c.align === "right" ? "justify-end" : ""
          } ${c.id === "health" ? "pl-[14px]" : ""}`}
          style={{ width: `var(${widthVar(c.id)})` }}
        >
          <ResizeHandle
            column={c}
            width={widths[c.id]}
            onResizing={onResizing}
            onResized={onResized}
          />
          <SortButton
            label={c.label}
            by={c.sortBy}
            sort={sort}
            onSort={onSort}
            alignRight={c.align === "right"}
          />
        </span>
      ))}
    </div>
  );
}

/** `aria-sort` for a header: set only on the column the list is ordered by. */
function ariaSort(
  sort: SortState,
  by: SortBy,
): "ascending" | "descending" | undefined {
  if (sort.by !== by) return undefined;
  return sort.direction === "asc" ? "ascending" : "descending";
}

/**
 * A header's label as a button that sorts by it.
 *
 * The arrow only appears on the sorted column, and the direction is also in
 * `aria-sort` on the header, so it is never carried by the glyph alone.
 */
function SortButton({
  label,
  by,
  sort,
  onSort,
  alignRight = false,
}: {
  label: string;
  by: SortBy;
  sort: SortState;
  onSort: (by: SortBy) => void;
  alignRight?: boolean;
}) {
  const active = sort.by === by;
  return (
    <button
      type="button"
      onClick={() => onSort(by)}
      className={`hover:text-fg-1 focus-visible:text-fg-1 flex min-w-0 items-center gap-1 uppercase ${
        active ? "text-fg-1" : ""
      } ${alignRight ? "flex-row-reverse" : ""}`}
    >
      <span className="truncate">{label}</span>
      {active ? (
        <Icon
          name={sort.direction === "asc" ? "arrow-up" : "arrow-down"}
          size={10}
        />
      ) : null}
    </button>
  );
}

/**
 * The draggable divider on a column's left edge.
 *
 * On the left because the name column to the left of every resizable one
 * absorbs the change: widening a column moves its left edge and leaves its
 * right edge, and every column after it, where they were -- so the divider
 * follows the pointer.
 *
 * A focusable `separator`, so it answers to the keyboard: Left and Right move
 * the divider (Shift for bigger steps), Home and End take the column to its
 * narrowest and widest, and Enter -- or a double-click -- puts it back to its
 * default.
 */
function ResizeHandle({
  column,
  width,
  onResizing,
  onResized,
}: {
  column: ColumnDef;
  width: number;
  onResizing: (id: ColumnId, width: number) => void;
  onResized: (id: ColumnId, width: number) => void;
}) {
  const drag = useRef<{ x: number; width: number; last: number } | null>(null);

  const set = (next: number) =>
    onResized(column.id, clampWidth(column.id, next));

  return (
    <span
      role="separator"
      aria-orientation="vertical"
      aria-label={`Resize ${column.label} column`}
      aria-valuenow={width}
      aria-valuemin={column.minWidth}
      aria-valuemax={MAX_COLUMN_WIDTH}
      aria-valuetext={`${width} pixels wide`}
      tabIndex={0}
      title="Drag to resize · double-click to reset"
      className="group absolute top-0 -left-3 z-10 flex h-full w-2 cursor-col-resize touch-none justify-center outline-none"
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        drag.current = { x: event.clientX, width, last: width };
      }}
      onPointerMove={(event) => {
        const d = drag.current;
        if (!d) return;
        // Left edge: moving the pointer left widens the column.
        const next = clampWidth(column.id, d.width + (d.x - event.clientX));
        if (next !== d.last) {
          d.last = next;
          onResizing(column.id, next);
        }
      }}
      onPointerUp={(event) => {
        const d = drag.current;
        drag.current = null;
        if (event.currentTarget.hasPointerCapture(event.pointerId)) {
          event.currentTarget.releasePointerCapture(event.pointerId);
        }
        if (d && d.last !== d.width) onResized(column.id, d.last);
      }}
      onPointerCancel={() => {
        const d = drag.current;
        drag.current = null;
        // Put the column back where the drag found it.
        if (d) onResizing(column.id, d.width);
      }}
      onDoubleClick={() => set(column.defaultWidth)}
      onClick={(event) => event.stopPropagation()}
      onKeyDown={(event) => {
        const step = event.shiftKey ? KEY_STEP * 4 : KEY_STEP;
        // Left and Right move the divider, so Left widens the column.
        if (event.key === "ArrowLeft") set(width + step);
        else if (event.key === "ArrowRight") set(width - step);
        else if (event.key === "Home") set(column.minWidth);
        else if (event.key === "End") set(MAX_COLUMN_WIDTH);
        else if (event.key === "Enter") set(column.defaultWidth);
        else return;
        event.preventDefault();
      }}
    >
      {/*
        Always drawn, faintly: a divider that only appears on hover is one
        nobody finds. It brightens to the accent under the pointer or focus.
      */}
      <span className="bg-line-2 group-hover:bg-acc group-focus-visible:bg-acc h-3.5 w-px self-center transition-colors group-hover:h-full group-focus-visible:h-full" />
    </span>
  );
}
