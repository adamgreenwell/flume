"use client";

import type { SortState } from "@/lib/ipc/types";
import { SORTS, defaultDirectionFor } from "@/lib/sort";

import { Button } from "./Button";
import { Chip } from "./Chip";
import { Icon } from "./Icon";

/** Props for {@link LibraryToolbar}. */
export interface LibraryToolbarProps {
  /** Name of the active view. */
  title: string;
  /** How many rows the list is showing after filtering. */
  count: number;
  /** The active sort key and direction. */
  sort: SortState;
  /** Change the sort key, its direction, or both. */
  onSortChange: (s: SortState) => void;
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
        {/*
          A select rather than the chips this used to be. Three keys fitted
          across a toolbar; ten do not, and the overflow would have to be a
          menu anyway. Native, because a custom listbox would have to
          reimplement type-ahead, Home/End and the platform's own touch
          handling to be no better than this.
        */}
        <select
          aria-label="Sort by"
          value={sort.by}
          onChange={(event) => {
            const by = event.target.value as SortState["by"];
            // The direction resets to the one the key is useful in. Choosing
            // "Size" and getting the smallest torrent first is a small,
            // constant annoyance, and carrying the previous key's direction
            // over is how that happens.
            onSortChange({ by, direction: defaultDirectionFor(by) });
          }}
          className="border-line bg-bg-2 text-fg-1 hover:text-fg-0 h-[var(--flume-h-control)] rounded-md border px-2 text-[12.5px]"
        >
          {SORTS.map((s) => (
            <option key={s.by} value={s.by}>
              {s.name}
            </option>
          ))}
        </select>
        {/*
          The direction is a word in the accessible name, not a rotation of
          the glyph alone. An arrow that only differs by which way it points
          is still a shape rather than a colour, so it passes -- but a screen
          reader gets nothing from it, and this control has exactly two
          states that must be told apart.
        */}
        <Chip
          selected={false}
          onClick={() =>
            onSortChange({
              by: sort.by,
              direction: sort.direction === "asc" ? "desc" : "asc",
            })
          }
          aria-label={
            sort.direction === "asc"
              ? "Sorted ascending, press to reverse"
              : "Sorted descending, press to reverse"
          }
        >
          <Icon
            name={sort.direction === "asc" ? "arrow-up" : "arrow-down"}
            size={14}
          />
        </Chip>
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
