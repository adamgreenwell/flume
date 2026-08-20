"use client";

import { formatBytes } from "@/lib/format";
import type { TorrentFile } from "@/lib/ipc/types";

import { Button } from "./Button";

/** Props for {@link FileTree}. */
export interface FileTreeProps {
  /** Every file in the torrent, in torrent order. */
  files: TorrentFile[];
  /** Indices currently selected for download. */
  selected: ReadonlySet<number>;
  /** Called with the next selection whenever it changes. */
  onChange: (next: Set<number>) => void;
}

/**
 * A flat, checkbox-per-file selector.
 *
 * Deliberately flat rather than a nested folder tree: distro torrents are
 * usually a handful of files in one directory, and a collapsible tree would be
 * more chrome than help. Nesting can come when a torrent that needs it exists.
 *
 * @param props - See {@link FileTreeProps}.
 * @returns The rendered file list.
 */
export function FileTree({ files, selected, onChange }: FileTreeProps) {
  const selectedBytes = files
    .filter((f) => selected.has(f.index))
    .reduce((sum, f) => sum + f.length, 0);

  const toggle = (index: number) => {
    const next = new Set(selected);
    if (next.has(index)) next.delete(index);
    else next.add(index);
    onChange(next);
  };

  return (
    <div className="flex min-h-0 flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <p className="text-muted text-xs">
          {selected.size} of {files.length} selected ·{" "}
          <span className="font-mono">{formatBytes(selectedBytes)}</span>
        </p>
        <div className="flex gap-1">
          <Button
            variant="ghost"
            onClick={() => onChange(new Set(files.map((f) => f.index)))}
            disabled={selected.size === files.length}
          >
            All
          </Button>
          <Button
            variant="ghost"
            onClick={() => onChange(new Set())}
            disabled={selected.size === 0}
          >
            None
          </Button>
        </div>
      </div>

      <ul className="border-border-subtle bg-bg min-h-0 flex-1 overflow-y-auto rounded-md border">
        {files.map((file) => {
          const isSelected = selected.has(file.index);
          return (
            <li key={file.index}>
              <label className="border-border-subtle hover:bg-surface-raised flex cursor-pointer items-center gap-3 border-b px-3 py-2 last:border-b-0">
                <input
                  type="checkbox"
                  checked={isSelected}
                  onChange={() => toggle(file.index)}
                  className="accent-accent h-4 w-4 shrink-0"
                />
                <span
                  className={`min-w-0 flex-1 truncate text-sm ${isSelected ? "text-text" : "text-faint"}`}
                  title={file.path}
                >
                  {file.path}
                </span>
                <span className="text-muted shrink-0 font-mono text-xs tabular-nums">
                  {formatBytes(file.length)}
                </span>
              </label>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
