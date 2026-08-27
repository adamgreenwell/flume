"use client";

import { useMemo, useState } from "react";

import {
  buildTree,
  checkState,
  flatten,
  toggleNode,
  type TreeNode,
} from "@/lib/filetree";
import { formatBytes } from "@/lib/format";
import type { TorrentFile } from "@/lib/ipc/types";

import { Checkbox } from "./Checkbox";
import { Icon } from "./Icon";

/** How far each level indents, in pixels. */
const INDENT = 18;

/**
 * Picks a glyph from the file extension.
 *
 * A rough guess deliberately: the point is to make the shape of a torrent
 * scannable, not to be an authority on file types. Anything unrecognised gets
 * the generic document, which is honest about not knowing.
 */
function iconFor(name: string) {
  const ext = name.slice(name.lastIndexOf(".") + 1).toLowerCase();
  if (["mp4", "mkv", "mov", "avi", "webm", "m4v"].includes(ext)) return "files";
  if (["zip", "gz", "xz", "bz2", "7z", "rar", "iso"].includes(ext))
    return "folder";
  return "files";
}

/** Props for {@link FileTree}. */
export interface FileTreeProps {
  /** Every file in the torrent, in torrent order. */
  files: TorrentFile[];
  /** Indices currently selected for download. */
  selected: ReadonlySet<number>;
  /** Called with the next selection whenever it changes. */
  onChange: (next: Set<number>) => void;
  /** Indices already present on disk at full length. */
  onDisk: ReadonlySet<number>;
}

/**
 * The torrent's contents, as a tri-state tree.
 *
 * Folders carry a real mixed state rather than a checked/unchecked guess, so a
 * folder with one file deselected is visibly different from an empty one. That
 * is the difference between a tree you can trust at a glance and one you have
 * to open every branch of.
 *
 * Files already on disk are tagged. They are deselected by whoever owns the
 * selection, not here — this component renders a selection, it does not decide
 * one.
 *
 * @param props - See {@link FileTreeProps}.
 * @returns The rendered tree.
 */
export function FileTree({ files, selected, onChange, onDisk }: FileTreeProps) {
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());

  const tree = useMemo(() => buildTree(files), [files]);
  const rows = useMemo(() => flatten(tree, collapsed), [tree, collapsed]);

  const toggleCollapse = (node: TreeNode) => {
    const next = new Set(collapsed);
    if (next.has(node.id)) next.delete(node.id);
    else next.add(node.id);
    setCollapsed(next);
  };

  return (
    <div className="min-h-0 grow overflow-y-auto" role="tree">
      {rows.map((node) => {
        const state = checkState(node, selected);
        const present = node.index !== null && onDisk.has(node.index);

        return (
          <div
            key={node.id}
            role="treeitem"
            aria-selected={state === "on"}
            aria-expanded={node.isFolder ? node.expanded : undefined}
            tabIndex={0}
            onClick={() => onChange(toggleNode(node, selected))}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onChange(toggleNode(node, selected));
              }
            }}
            className="hover:bg-bg-2 flex h-7 cursor-pointer items-center gap-2 pr-3.5"
            style={{ paddingLeft: 14 + node.depth * INDENT }}
          >
            <Checkbox
              state={state}
              label={node.name}
              onChange={() => onChange(toggleNode(node, selected))}
            />

            {node.isFolder ? (
              <button
                type="button"
                className="text-fg-3 hover:text-fg-1 shrink-0"
                aria-label={
                  node.expanded
                    ? `Collapse ${node.name}`
                    : `Expand ${node.name}`
                }
                onClick={(event) => {
                  event.stopPropagation();
                  toggleCollapse(node);
                }}
              >
                <Icon
                  name={node.expanded ? "chevron-down" : "chevron-right"}
                  size={12}
                />
              </button>
            ) : (
              <span className="w-3 shrink-0" />
            )}

            <span className="text-fg-2 shrink-0">
              <Icon
                name={node.isFolder ? "folder" : iconFor(node.name)}
                size={14}
              />
            </span>

            <span className="truncate text-[12.5px]" title={node.name}>
              {node.name}
            </span>

            {present ? (
              <span className="text-ok bg-ok/15 shrink-0 rounded-sm px-1.5 py-0.5 text-[10px] font-medium">
                already on disk
              </span>
            ) : null}

            <span className="flume-num text-fg-2 ml-auto shrink-0 text-[11.5px]">
              {formatBytes(node.bytes)}
            </span>
          </div>
        );
      })}
    </div>
  );
}
