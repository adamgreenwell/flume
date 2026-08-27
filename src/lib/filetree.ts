import type { TorrentFile } from "@/lib/ipc/types";

/** A folder or a file in the review sheet's tree. */
export interface TreeNode {
  /** Stable identity: the full path for a file, the folder path for a folder. */
  id: string;
  /** Just this level's name, not the whole path. */
  name: string;
  /** How deep this sits. Top level is 0. */
  depth: number;
  /** Children, empty for a file. */
  children: TreeNode[];
  /** Torrent file index, or `null` for a folder. */
  index: number | null;
  /** Own size for a file; the sum of the subtree for a folder. */
  bytes: number;
  /** File indices at or under this node. */
  fileIndices: number[];
}

/** Whether a node is fully, partly or not selected. */
export type CheckState = "on" | "off" | "partial";

/** A node flattened for rendering, with its visibility already decided. */
export interface FlatNode extends TreeNode {
  /** Whether this node has children to show. */
  isFolder: boolean;
  /** Whether its children are currently shown. */
  expanded: boolean;
}

/**
 * Builds a nested tree from the torrent's flat, forward-slashed paths.
 *
 * Torrent paths are the only structure a torrent has — there is no directory
 * metadata — so the tree is inferred from the separators. A single-file
 * torrent produces one root node and no folders, which is the common case for
 * a distro ISO and should not be dressed up as a hierarchy.
 *
 * @param files - Every file, in torrent order.
 * @returns Root-level nodes, folders before files, each alphabetical.
 */
export function buildTree(files: readonly TorrentFile[]): TreeNode[] {
  const roots: TreeNode[] = [];
  const folders = new Map<string, TreeNode>();

  for (const file of files) {
    const parts = file.path.split("/").filter((p) => p !== "");
    if (parts.length === 0) continue;

    let siblings = roots;
    let prefix = "";

    // Every part but the last is a folder that may need creating.
    for (let depth = 0; depth < parts.length - 1; depth++) {
      prefix = prefix === "" ? parts[depth] : `${prefix}/${parts[depth]}`;

      let folder = folders.get(prefix);
      if (!folder) {
        folder = {
          id: prefix,
          name: parts[depth],
          depth,
          children: [],
          index: null,
          bytes: 0,
          fileIndices: [],
        };
        folders.set(prefix, folder);
        siblings.push(folder);
      }

      // Sizes and indices roll up as each file is placed, so no second pass
      // over the tree is needed to total a folder.
      folder.bytes += file.length;
      folder.fileIndices.push(file.index);
      siblings = folder.children;
    }

    siblings.push({
      id: file.path,
      name: parts[parts.length - 1],
      depth: parts.length - 1,
      children: [],
      index: file.index,
      bytes: file.length,
      fileIndices: [file.index],
    });
  }

  return sort(roots);
}

/**
 * Orders folders before files, then alphabetically within each.
 *
 * Torrent order is arbitrary and often reflects nothing a user recognises.
 * Alphabetical is at least predictable, and folders first means the shape of
 * the torrent is visible before its contents.
 *
 * Collation is numeric, so "part2" precedes "part10" rather than following it.
 * Torrents are full of numbered parts and episodes, and lexicographic order
 * gets every one of them wrong.
 *
 * @param nodes - Nodes to order, mutated in place along with their children.
 * @returns The same array, ordered.
 */
function sort(nodes: TreeNode[]): TreeNode[] {
  nodes.sort((a, b) => {
    const aFolder = a.index === null;
    const bFolder = b.index === null;
    if (aFolder !== bFolder) return aFolder ? -1 : 1;
    return a.name.localeCompare(b.name, undefined, { numeric: true });
  });
  for (const node of nodes) sort(node.children);
  return nodes;
}

/**
 * Flattens the tree into the rows to render, honouring collapsed folders.
 *
 * @param nodes - Root nodes from {@link buildTree}.
 * @param collapsed - Ids of folders whose children are hidden.
 * @returns Rows in display order.
 */
export function flatten(
  nodes: readonly TreeNode[],
  collapsed: ReadonlySet<string>,
): FlatNode[] {
  const out: FlatNode[] = [];

  const walk = (list: readonly TreeNode[]) => {
    for (const node of list) {
      const isFolder = node.index === null;
      const expanded = isFolder && !collapsed.has(node.id);
      out.push({ ...node, isFolder, expanded });
      if (expanded) walk(node.children);
    }
  };

  walk(nodes);
  return out;
}

/**
 * Whether a node is fully, partly or not selected.
 *
 * A folder is `partial` when some of its files are chosen. That third state is
 * the whole reason folder checkboxes are usable — without it, a folder with
 * one file deselected looks identical to one with everything deselected.
 *
 * @param node - The node to test.
 * @param selected - Currently selected file indices.
 * @returns The node's check state.
 */
export function checkState(
  node: TreeNode,
  selected: ReadonlySet<number>,
): CheckState {
  if (node.fileIndices.length === 0) return "off";

  let on = 0;
  for (const index of node.fileIndices) if (selected.has(index)) on += 1;

  if (on === 0) return "off";
  if (on === node.fileIndices.length) return "on";
  return "partial";
}

/**
 * Selects or clears everything under a node.
 *
 * A partly-selected folder selects the rest rather than clearing. Clicking a
 * folder that is half on almost always means "I want all of this"; the other
 * reading costs the user the selection they had just built.
 *
 * @param node - The node clicked.
 * @param selected - The current selection.
 * @returns The next selection. The input is not modified.
 */
export function toggleNode(
  node: TreeNode,
  selected: ReadonlySet<number>,
): Set<number> {
  const next = new Set(selected);
  const turnOn = checkState(node, selected) !== "on";

  for (const index of node.fileIndices) {
    if (turnOn) next.add(index);
    else next.delete(index);
  }
  return next;
}

/**
 * Total bytes of the selected files.
 *
 * @param files - Every file in the torrent.
 * @param selected - Selected indices.
 * @returns The sum, in bytes.
 */
export function selectedBytes(
  files: readonly TorrentFile[],
  selected: ReadonlySet<number>,
): number {
  return files
    .filter((f) => selected.has(f.index))
    .reduce((sum, f) => sum + f.length, 0);
}
