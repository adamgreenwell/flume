import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { MAX_COLUMN_WIDTH, resolveWidths } from "@/lib/columns";
import type { SortState } from "@/lib/ipc/types";

import { ColumnHeader } from "./ColumnHeader";

function setup(sort: SortState = { by: "name", direction: "asc" }) {
  const onSort = vi.fn();
  const onResizing = vi.fn();
  const onResized = vi.fn();
  render(
    <ColumnHeader
      sort={sort}
      onSort={onSort}
      widths={resolveWidths(null)}
      onResizing={onResizing}
      onResized={onResized}
    />,
  );
  return { onSort, onResizing, onResized };
}

describe("sorting from the header", () => {
  it("sorts by the column whose header was clicked", () => {
    const { onSort } = setup();
    fireEvent.click(screen.getByRole("button", { name: "Down" }));
    expect(onSort).toHaveBeenCalledWith("downloadBps");
    fireEvent.click(screen.getByRole("button", { name: "Swarm health" }));
    expect(onSort).toHaveBeenCalledWith("health");
  });

  it("marks only the sorted column, with its direction", () => {
    setup({ by: "progress", direction: "desc" });
    const headers = screen.getAllByRole("columnheader");
    const sorted = headers.filter((h) => h.hasAttribute("aria-sort"));
    expect(sorted).toHaveLength(1);
    expect(sorted[0]?.textContent).toContain("Progress");
    expect(sorted[0]?.getAttribute("aria-sort")).toBe("descending");
  });

  it("marks no column when the list is sorted by something without one", () => {
    setup({ by: "activity", direction: "desc" });
    const sorted = screen
      .getAllByRole("columnheader")
      .filter((h) => h.hasAttribute("aria-sort"));
    expect(sorted).toHaveLength(0);
  });
});

describe("resizing from the keyboard", () => {
  it("widens with Left and narrows with Right, since the divider is the column's left edge", () => {
    const { onResized } = setup();
    const handle = screen.getByRole("separator", {
      name: "Resize Progress column",
    });
    fireEvent.keyDown(handle, { key: "ArrowLeft" });
    expect(onResized).toHaveBeenLastCalledWith("progress", 188);
    fireEvent.keyDown(handle, { key: "ArrowRight", shiftKey: true });
    expect(onResized).toHaveBeenLastCalledWith("progress", 148);
  });

  it("stops at the column's minimum and the global maximum", () => {
    const { onResized } = setup();
    const handle = screen.getByRole("separator", {
      name: "Resize Down column",
    });
    fireEvent.keyDown(handle, { key: "Home" });
    expect(onResized).toHaveBeenLastCalledWith("down", 68);
    fireEvent.keyDown(handle, { key: "End" });
    expect(onResized).toHaveBeenLastCalledWith("down", MAX_COLUMN_WIDTH);
  });

  it("resets to the default on Enter and on double-click", () => {
    const { onResized } = setup();
    const handle = screen.getByRole("separator", {
      name: "Resize Peers column",
    });
    fireEvent.keyDown(handle, { key: "Enter" });
    expect(onResized).toHaveBeenLastCalledWith("peers", 78);
    onResized.mockClear();
    fireEvent.doubleClick(handle);
    expect(onResized).toHaveBeenLastCalledWith("peers", 78);
  });

  it("does not sort when the divider is clicked", () => {
    const { onSort } = setup();
    fireEvent.click(
      screen.getByRole("separator", { name: "Resize Up column" }),
    );
    expect(onSort).not.toHaveBeenCalled();
  });
});

describe("resizing by dragging", () => {
  it("repaints while dragging and keeps the width only on release", () => {
    const { onResizing, onResized } = setup();
    const handle = screen.getByRole("separator", {
      name: "Resize Progress column",
    });
    // jsdom has no pointer capture.
    handle.setPointerCapture = () => {};
    handle.hasPointerCapture = () => false;

    fireEvent.pointerDown(handle, { button: 0, clientX: 500, pointerId: 1 });
    fireEvent.pointerMove(handle, { clientX: 460, pointerId: 1 });
    expect(onResizing).toHaveBeenLastCalledWith("progress", 220);
    expect(onResized).not.toHaveBeenCalled();

    fireEvent.pointerUp(handle, { clientX: 460, pointerId: 1 });
    expect(onResized).toHaveBeenCalledWith("progress", 220);
  });
});
