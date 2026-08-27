import { describe, expect, it } from "vitest";

import type { TorrentFile } from "@/lib/ipc/types";

import {
  buildTree,
  checkState,
  flatten,
  selectedBytes,
  toggleNode,
} from "./filetree";

/** Files in deliberately arbitrary torrent order, as a real torrent gives them. */
const FILES: TorrentFile[] = [
  { index: 0, path: "02 Production/textures.zip", length: 3_100_000_000 },
  { index: 1, path: "01 Final/sprite-fright-4k.mov", length: 41_800_000_000 },
  { index: 2, path: "readme.txt", length: 4_000 },
  { index: 3, path: "01 Final/sprite-fright-1080p.mp4", length: 6_400_000_000 },
  { index: 4, path: "02 Production/scenes.blend", length: 4_200_000_000 },
];

describe("buildTree", () => {
  it("infers folders from the path separators", () => {
    const roots = buildTree(FILES);

    expect(roots.map((n) => n.name)).toEqual([
      "01 Final",
      "02 Production",
      "readme.txt",
    ]);
  });

  it("puts folders before files and sorts each alphabetically", () => {
    // Torrent order is arbitrary and usually reflects nothing a user
    // recognises. Alphabetical is at least predictable.
    const roots = buildTree(FILES);

    expect(roots[0].index).toBeNull();
    expect(roots[2].index).toBe(2);
    // Numeric collation, so "4k" precedes "1080p" — 4 really is less than
    // 1080. The same rule is what puts "part2" before "part10", which is the
    // case that actually matters in a torrent.
    expect(roots[0].children.map((c) => c.name)).toEqual([
      "sprite-fright-4k.mov",
      "sprite-fright-1080p.mp4",
    ]);
  });

  it("orders embedded numbers numerically, not lexicographically", () => {
    const roots = buildTree([
      { index: 0, path: "part10.bin", length: 1 },
      { index: 1, path: "part2.bin", length: 1 },
      { index: 2, path: "part1.bin", length: 1 },
    ]);

    expect(roots.map((n) => n.name)).toEqual([
      "part1.bin",
      "part2.bin",
      "part10.bin",
    ]);
  });

  it("rolls sizes up into folders", () => {
    const roots = buildTree(FILES);

    expect(roots[0].bytes).toBe(41_800_000_000 + 6_400_000_000);
    expect(roots[1].bytes).toBe(3_100_000_000 + 4_200_000_000);
  });

  it("collects every descendant index on a folder", () => {
    const roots = buildTree(FILES);
    expect([...roots[0].fileIndices].sort()).toEqual([1, 3]);
  });

  it("leaves a single-file torrent flat rather than inventing a hierarchy", () => {
    // The common case for a distro ISO. Dressing one file up as a tree is
    // chrome, not help.
    const roots = buildTree([
      { index: 0, path: "debian-13.2.0-amd64-DVD-1.iso", length: 100 },
    ]);

    expect(roots).toHaveLength(1);
    expect(roots[0].index).toBe(0);
    expect(roots[0].children).toEqual([]);
  });

  it("handles nesting deeper than one level", () => {
    const roots = buildTree([{ index: 0, path: "a/b/c/deep.bin", length: 10 }]);

    expect(roots[0].name).toBe("a");
    expect(roots[0].children[0].name).toBe("b");
    expect(roots[0].children[0].children[0].name).toBe("c");
    expect(roots[0].children[0].children[0].children[0].index).toBe(0);
    // Sizes roll all the way up.
    expect(roots[0].bytes).toBe(10);
  });

  it("survives an empty file list", () => {
    expect(buildTree([])).toEqual([]);
  });
});

describe("flatten", () => {
  it("shows every row when nothing is collapsed", () => {
    const rows = flatten(buildTree(FILES), new Set());
    expect(rows).toHaveLength(7); // 2 folders + 5 files
  });

  it("hides the children of a collapsed folder", () => {
    const rows = flatten(buildTree(FILES), new Set(["01 Final"]));

    expect(rows.map((r) => r.name)).not.toContain("sprite-fright-4k.mov");
    expect(rows.map((r) => r.name)).toContain("01 Final");
    // Its sibling folder is untouched.
    expect(rows.map((r) => r.name)).toContain("scenes.blend");
  });

  it("marks folders and their expansion state", () => {
    const rows = flatten(buildTree(FILES), new Set(["01 Final"]));
    const folder = rows.find((r) => r.name === "01 Final");

    expect(folder?.isFolder).toBe(true);
    expect(folder?.expanded).toBe(false);
  });
});

describe("checkState", () => {
  it("reports a folder with everything chosen as on", () => {
    const roots = buildTree(FILES);
    expect(checkState(roots[0], new Set([1, 3]))).toBe("on");
  });

  it("reports a folder with nothing chosen as off", () => {
    const roots = buildTree(FILES);
    expect(checkState(roots[0], new Set())).toBe("off");
  });

  it("reports a folder with some chosen as partial", () => {
    // The state that makes folder checkboxes usable at all: without it a
    // folder missing one file looks the same as an empty one.
    const roots = buildTree(FILES);
    expect(checkState(roots[0], new Set([1]))).toBe("partial");
  });
});

describe("toggleNode", () => {
  it("selects everything under a folder that was off", () => {
    const roots = buildTree(FILES);
    expect([...toggleNode(roots[0], new Set())].sort()).toEqual([1, 3]);
  });

  it("clears everything under a folder that was fully on", () => {
    const roots = buildTree(FILES);
    expect([...toggleNode(roots[0], new Set([1, 3]))]).toEqual([]);
  });

  it("completes a partly-selected folder rather than clearing it", () => {
    // Clicking a half-on folder almost always means "I want all of this".
    // The other reading throws away the selection the user just built.
    const roots = buildTree(FILES);
    expect([...toggleNode(roots[0], new Set([1]))].sort()).toEqual([1, 3]);
  });

  it("leaves files outside the node alone", () => {
    const roots = buildTree(FILES);
    const next = toggleNode(roots[0], new Set([0, 1]));

    expect(next.has(0)).toBe(true);
  });

  it("does not modify the selection it was given", () => {
    const roots = buildTree(FILES);
    const original = new Set([1]);
    toggleNode(roots[0], original);

    expect([...original]).toEqual([1]);
  });
});

describe("selectedBytes", () => {
  it("totals only the chosen files", () => {
    expect(selectedBytes(FILES, new Set([1, 3]))).toBe(48_200_000_000);
  });

  it("is zero for an empty selection", () => {
    expect(selectedBytes(FILES, new Set())).toBe(0);
  });
});
