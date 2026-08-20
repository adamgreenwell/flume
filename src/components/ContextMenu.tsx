"use client";

import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { Icon, type IconName } from "./Icon";

/** One entry in a context menu. */
export interface ContextMenuItem {
  /** Label shown to the user. */
  label: string;
  /** Optional leading glyph. */
  icon?: IconName;
  /** Invoked on selection. */
  run: () => void;
  /** Tints the entry as destructive. */
  destructive?: boolean;
}

/** Props for {@link ContextMenu}. */
export interface ContextMenuProps {
  /** Viewport coordinates where the menu should appear. */
  position: { x: number; y: number };
  /** Entries to show. */
  items: ContextMenuItem[];
  /** Called when the menu should close without a selection. */
  onClose: () => void;
}

/**
 * A right-click menu anchored to the pointer.
 *
 * Position is clamped to the viewport after mount: a click near the right or
 * bottom edge would otherwise open a menu that runs off-screen, which is worst
 * exactly where list rows tend to be.
 *
 * Items are `whitespace-nowrap` for a subtle reason. A fixed element opened
 * near the right edge is squeezed by the viewport, so its text wraps and it
 * measures *narrower* than its natural width — which makes the clamp
 * under-correct and leaves the menu wrapped even after repositioning. Refusing
 * to wrap keeps the measured width honest.
 *
 * Keyboard support is deliberate rather than incidental — arrow keys move,
 * Enter selects, Escape closes — because a context menu reachable only by
 * mouse fails the same users the visible row controls were designed for.
 *
 * @param props - See {@link ContextMenuProps}.
 * @returns The rendered menu.
 */
export function ContextMenu({ position, items, onClose }: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [placement, setPlacement] = useState(position);
  const [activeIndex, setActiveIndex] = useState(0);

  // Measure before paint so the menu never appears in the wrong place first.
  useLayoutEffect(() => {
    const menu = menuRef.current;
    if (!menu) return;
    const { width, height } = menu.getBoundingClientRect();
    const margin = 8;
    setPlacement({
      x: Math.min(position.x, window.innerWidth - width - margin),
      y: Math.min(position.y, window.innerHeight - height - margin),
    });
  }, [position]);

  useEffect(() => {
    menuRef.current?.focus();

    const onKeyDown = (event: KeyboardEvent) => {
      switch (event.key) {
        case "Escape":
          event.preventDefault();
          onClose();
          break;
        case "ArrowDown":
          event.preventDefault();
          setActiveIndex((i) => (i + 1) % items.length);
          break;
        case "ArrowUp":
          event.preventDefault();
          setActiveIndex((i) => (i - 1 + items.length) % items.length);
          break;
        case "Enter":
        case " ":
          event.preventDefault();
          items[activeIndex]?.run();
          onClose();
          break;
        default:
          break;
      }
    };

    // Any click elsewhere, or a scroll, dismisses the menu — a menu left
    // floating over content the user has moved on from is worse than none.
    const dismiss = () => onClose();

    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("mousedown", dismiss);
    window.addEventListener("resize", dismiss);
    window.addEventListener("scroll", dismiss, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("mousedown", dismiss);
      window.removeEventListener("resize", dismiss);
      window.removeEventListener("scroll", dismiss, true);
    };
  }, [items, activeIndex, onClose]);

  return (
    <div
      ref={menuRef}
      role="menu"
      aria-label="Torrent actions"
      tabIndex={-1}
      style={{ left: placement.x, top: placement.y }}
      className="border-border-subtle bg-surface fixed z-50 min-w-44 rounded-lg border p-1 shadow-2xl outline-none"
      // The menu's own mousedown must not reach the dismiss listener.
      onMouseDown={(e) => e.stopPropagation()}
    >
      {items.map((item, index) => (
        <button
          key={item.label}
          type="button"
          role="menuitem"
          onMouseEnter={() => setActiveIndex(index)}
          onClick={() => {
            item.run();
            onClose();
          }}
          className={`flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left text-sm whitespace-nowrap transition-colors ${
            index === activeIndex ? "bg-surface-raised" : ""
          } ${item.destructive ? "text-error" : "text-text"}`}
        >
          {item.icon ? (
            <Icon name={item.icon} size={14} className="shrink-0 opacity-70" />
          ) : null}
          {item.label}
        </button>
      ))}
    </div>
  );
}
