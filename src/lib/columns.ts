/**
 * The torrent list's columns: what each is called, how wide it starts, how
 * narrow it may go, and what clicking its header sorts by (#147).
 *
 * The one source for those facts. The header and every row kind read each
 * width from a CSS custom property set on the list container, so a drag
 * repaints the list without re-rendering a row against the 1 Hz tick.
 *
 * Only the columns in {@link ColumnWidths} resize. The state dot is fixed,
 * and the name takes whatever is left -- which is what keeps every other
 * column anchored to the right edge, and why a column's resize handle sits on
 * its *left* edge: dragging it moves the divider under the pointer while
 * everything to its right stays put.
 */

import type { ColumnWidths, SortBy } from "@/lib/ipc/types";

/** A resizable column's id: a key of {@link ColumnWidths}. */
export type ColumnId = keyof ColumnWidths;

/** One resizable column. */
export interface ColumnDef {
  id: ColumnId;
  /** Header text, also used in the resize handle's accessible name. */
  label: string;
  /** Width in CSS pixels until the user drags it. */
  defaultWidth: number;
  /**
   * The narrowest this column still draws its content at. Below it a rate
   * loses its unit, or a pending magnet's button its label.
   */
  minWidth: number;
  /** Right-aligned figures, or left-aligned content. */
  align: "left" | "right";
  /** What clicking the header sorts by. */
  sortBy: SortBy;
}

/** The widest any column may be dragged. Matches Rust `MAX_COLUMN_WIDTH`. */
export const MAX_COLUMN_WIDTH = 800;

/** The narrowest the name column gets before the list scrolls sideways. */
export const NAME_MIN_WIDTH = 140;

/** The state dot's fixed width. */
export const STATE_WIDTH = 18;

/** The gap between cells (`gap-4`) and the row's side padding (`px-[18px]`). */
const CELL_GAP = 16;
const ROW_PADDING = 18;

/** The resizable columns, left to right. */
export const COLUMNS: readonly ColumnDef[] = [
  {
    id: "progress",
    label: "Progress",
    defaultWidth: 180,
    // A pending magnet's "Choose files…" button sits in this column.
    minWidth: 112,
    align: "left",
    sortBy: "progress",
  },
  {
    id: "down",
    label: "Down",
    defaultWidth: 86,
    minWidth: 68,
    align: "right",
    sortBy: "downloadBps",
  },
  {
    id: "up",
    label: "Up",
    defaultWidth: 86,
    minWidth: 68,
    align: "right",
    sortBy: "uploadBps",
  },
  {
    id: "peers",
    label: "Peers",
    defaultWidth: 78,
    minWidth: 56,
    align: "right",
    sortBy: "peers",
  },
  {
    id: "health",
    label: "Swarm health",
    defaultWidth: 124,
    // The chip's longest word, or a pending magnet's word and trash button.
    minWidth: 104,
    align: "left",
    sortBy: "health",
  },
];

/** Looks up a column by id. */
export function columnDef(id: ColumnId): ColumnDef {
  const def = COLUMNS.find((c) => c.id === id);
  if (!def) throw new Error(`unknown column ${id}`);
  return def;
}

/** Every resizable column at a concrete width. */
export type ResolvedWidths = Record<ColumnId, number>;

/**
 * Clamps a width to what a column allows, rounded to a whole pixel.
 *
 * @param id - The column.
 * @param width - The requested width, in CSS pixels.
 * @returns The width the column will actually draw at.
 */
export function clampWidth(id: ColumnId, width: number): number {
  const { minWidth } = columnDef(id);
  if (!Number.isFinite(width)) return columnDef(id).defaultWidth;
  return Math.min(MAX_COLUMN_WIDTH, Math.max(minWidth, Math.round(width)));
}

/**
 * Fills in defaults for untouched columns and clamps the rest.
 *
 * Clamped here as well as in Rust because the minimum is per column and only
 * this side knows what a column draws; Rust only guards the file's range.
 *
 * @param saved - The persisted widths, `null` meaning the default.
 * @returns A width for every resizable column.
 */
export function resolveWidths(
  saved: Partial<ColumnWidths> | null | undefined,
): ResolvedWidths {
  const out = {} as ResolvedWidths;
  for (const c of COLUMNS) {
    const w = saved?.[c.id];
    out[c.id] = w == null ? c.defaultWidth : clampWidth(c.id, w);
  }
  return out;
}

/**
 * What to persist: `null` for a column at its default, so it keeps
 * following the default if a later release changes it.
 *
 * @param widths - Every column's current width.
 * @returns The value for `Settings.columns`.
 */
export function toSaved(widths: ResolvedWidths): ColumnWidths {
  const out = {} as ColumnWidths;
  for (const c of COLUMNS) {
    out[c.id] = widths[c.id] === c.defaultWidth ? null : widths[c.id];
  }
  return out;
}

/** The custom property a column's width is published under. */
export function widthVar(id: ColumnId): string {
  return `--flume-col-${id}`;
}

/**
 * The narrowest the whole row can be: every fixed column, the name at its
 * minimum, the gaps and the padding. Below it the list scrolls sideways
 * rather than squeezing the name to nothing.
 */
export function minRowWidth(widths: ResolvedWidths): number {
  const cells = COLUMNS.reduce((sum, c) => sum + widths[c.id], 0);
  const gaps = (COLUMNS.length + 1) * CELL_GAP; // + state dot and name
  return STATE_WIDTH + NAME_MIN_WIDTH + cells + gaps + 2 * ROW_PADDING;
}

/**
 * The custom properties for the list container.
 *
 * @param widths - Every column's current width.
 * @returns A style object setting each column's width and the row minimum.
 */
export function columnStyle(widths: ResolvedWidths): Record<string, string> {
  const style: Record<string, string> = {
    "--flume-list-min-w": `${minRowWidth(widths)}px`,
  };
  for (const c of COLUMNS) style[widthVar(c.id)] = `${widths[c.id]}px`;
  return style;
}
