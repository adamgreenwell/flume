"use client";

import { useEffect } from "react";

/** A keyboard shortcut and what it does. */
export interface Shortcut {
  /** `event.key` to match, compared case-insensitively. */
  key: string;
  /** Require the platform's primary modifier (⌘ on macOS, Ctrl elsewhere). */
  meta?: boolean;
  /** What to run. */
  run: () => void;
  /** Human-readable description, for a future shortcuts list. */
  description: string;
}

/**
 * Whether an event originated inside a text-entry control.
 *
 * Shortcuts must not fire while someone is typing a magnet link — "n" would
 * otherwise open a dialog mid-word, and a bare-key shortcut scheme is
 * unusable without this check.
 */
function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName;
  return (
    tag === "INPUT" ||
    tag === "TEXTAREA" ||
    tag === "SELECT" ||
    target.isContentEditable
  );
}

/**
 * Binds application-level keyboard shortcuts.
 *
 * Handlers are looked up per keystroke rather than pre-indexed, because the
 * list is tiny and re-binding the listener whenever a callback identity
 * changed would be more expensive than the lookup.
 *
 * @param shortcuts - The shortcuts to bind.
 * @param enabled - Set false to suspend them, e.g. while a modal is open.
 */
export function useKeyboardShortcuts(
  shortcuts: Shortcut[],
  enabled = true,
): void {
  useEffect(() => {
    if (!enabled) return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (isTyping(event.target)) return;

      // macOS uses ⌘, everything else uses Ctrl. `metaKey` on Windows is the
      // Windows key, which should not trigger app shortcuts.
      const isMac = navigator.platform.toLowerCase().includes("mac");
      const primary = isMac ? event.metaKey : event.ctrlKey;

      for (const shortcut of shortcuts) {
        if (event.key.toLowerCase() !== shortcut.key.toLowerCase()) continue;
        if (Boolean(shortcut.meta) !== primary) continue;
        event.preventDefault();
        shortcut.run();
        return;
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [shortcuts, enabled]);
}
