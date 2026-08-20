import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useKeyboardShortcuts, type Shortcut } from "./useKeyboardShortcuts";

/** Pretends the platform is macOS, so ⌘ is the primary modifier. */
function asMac() {
  Object.defineProperty(navigator, "platform", {
    value: "MacIntel",
    configurable: true,
  });
}

/** Dispatches a keydown, optionally from inside a given element. */
function press(
  key: string,
  options: { meta?: boolean; ctrl?: boolean; from?: HTMLElement } = {},
) {
  const event = new KeyboardEvent("keydown", {
    key,
    metaKey: options.meta ?? false,
    ctrlKey: options.ctrl ?? false,
    bubbles: true,
    cancelable: true,
  });
  (options.from ?? window).dispatchEvent(event);
  return event;
}

afterEach(() => {
  document.body.innerHTML = "";
});

describe("useKeyboardShortcuts", () => {
  it("runs a matching shortcut", () => {
    asMac();
    const run = vi.fn();
    const shortcuts: Shortcut[] = [
      { key: "n", meta: true, run, description: "Add" },
    ];

    renderHook(() => useKeyboardShortcuts(shortcuts));
    press("n", { meta: true });

    expect(run).toHaveBeenCalledOnce();
  });

  it("requires the modifier when one is specified", () => {
    asMac();
    const run = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([{ key: "n", meta: true, run, description: "Add" }]),
    );

    press("n");

    expect(run).not.toHaveBeenCalled();
  });

  it("does not fire while the user is typing", () => {
    asMac();
    const run = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([{ key: "n", meta: true, run, description: "Add" }]),
    );

    // A magnet link contains no "n"-with-modifier, but a bare-key scheme would
    // be unusable without this guard, so it is enforced for all shortcuts.
    const input = document.createElement("input");
    document.body.append(input);
    press("n", { meta: true, from: input });

    expect(run).not.toHaveBeenCalled();
  });

  it("ignores keystrokes in a contenteditable region", () => {
    asMac();
    const run = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([{ key: "n", meta: true, run, description: "Add" }]),
    );

    const editable = document.createElement("div");
    editable.contentEditable = "true";
    // jsdom does not implement isContentEditable from the attribute alone.
    Object.defineProperty(editable, "isContentEditable", { value: true });
    document.body.append(editable);
    press("n", { meta: true, from: editable });

    expect(run).not.toHaveBeenCalled();
  });

  it("can be suspended, so shortcuts do not fire behind a modal", () => {
    asMac();
    const run = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts(
        [{ key: "n", meta: true, run, description: "Add" }],
        false,
      ),
    );

    press("n", { meta: true });

    expect(run).not.toHaveBeenCalled();
  });

  it("prevents the default action when it handles a key", () => {
    asMac();
    renderHook(() =>
      useKeyboardShortcuts([
        { key: "n", meta: true, run: () => {}, description: "Add" },
      ]),
    );

    const event = press("n", { meta: true });

    expect(event.defaultPrevented).toBe(true);
  });

  it("uses Ctrl rather than Meta off macOS", () => {
    Object.defineProperty(navigator, "platform", {
      value: "Win32",
      configurable: true,
    });
    const run = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([{ key: "n", meta: true, run, description: "Add" }]),
    );

    // The Windows key must not act as the primary modifier.
    press("n", { meta: true });
    expect(run).not.toHaveBeenCalled();

    press("n", { ctrl: true });
    expect(run).toHaveBeenCalledOnce();
  });
});
