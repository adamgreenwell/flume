"use client";

import { Button } from "./Button";
import { Chip } from "./Chip";
import { Icon } from "./Icon";

/** How the list is ordered. */
export type SortId = "activity" | "added" | "size";

const SORTS: ReadonlyArray<{ id: SortId; name: string }> = [
  { id: "activity", name: "Activity" },
  { id: "added", name: "Recently added" },
  { id: "size", name: "Size" },
];

/** Props for {@link LibraryToolbar}. */
export interface LibraryToolbarProps {
  /** Name of the active view. */
  title: string;
  /** How many rows the list is showing after filtering. */
  count: number;
  /** The active sort. */
  sort: SortId;
  /** Change the sort. */
  onSortChange: (s: SortId) => void;
  /** Whether rows are drawn at compact density. */
  compact: boolean;
  /** Toggle row density. */
  onDensityToggle: () => void;
  /** Open the add-torrent flow. */
  onAdd: () => void;
}

/**
 * The 56px bar above the list: what you are looking at, and how.
 *
 * The count is of what is on screen, not of everything — with a search active
 * the honest number is the one the user can see.
 *
 * @param props - See {@link LibraryToolbarProps}.
 * @returns The rendered toolbar.
 */
export function LibraryToolbar({
  title,
  count,
  sort,
  onSortChange,
  compact,
  onDensityToggle,
  onAdd,
}: LibraryToolbarProps) {
  return (
    <div className="border-line flex h-14 shrink-0 items-center gap-3 border-b px-[18px]">
      <h1 className="text-[17px] font-semibold tracking-[-0.02em]">{title}</h1>
      <span className="flume-num text-fg-3 ml-px text-xs">
        {count} {count === 1 ? "item" : "items"}
      </span>

      <span className="grow" />

      <div className="flex items-center gap-1.5" role="group" aria-label="Sort">
        {SORTS.map((s) => (
          <Chip
            key={s.id}
            selected={s.id === sort}
            onClick={() => onSortChange(s.id)}
          >
            {s.name}
          </Chip>
        ))}
        <Chip
          selected={compact}
          onClick={onDensityToggle}
          title="Row density"
          aria-label="Compact rows"
        >
          <Icon name="settings" size={14} />
        </Chip>
      </div>

      <Button variant="primary" onClick={onAdd} title="Add a torrent (⌘N)">
        <Icon name="plus" size={15} />
        Add torrent
      </Button>
    </div>
  );
}
