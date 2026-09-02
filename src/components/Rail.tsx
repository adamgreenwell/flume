"use client";

import { useEffect, useMemo, useRef } from "react";

import {
  useKeyboardShortcuts,
  type Shortcut,
} from "@/hooks/useKeyboardShortcuts";

import { guardRailLabel } from "@/lib/egress";
import type { CoreStatus, EngineHealth, GuardStatus } from "@/lib/ipc/types";
import { VIEWS, type ViewId } from "@/lib/views";

import { Icon } from "./Icon";

/** The three-wave mark. A flume carries things downstream. */
function Mark() {
  return (
    <svg
      className="text-acc h-[22px] w-[22px] shrink-0"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M3 7.5c2.2-1.9 4.4-1.9 6.6 0s4.4 1.9 6.6 0 4.4-1.9 4.8 0" />
      <path
        d="M3 13c2.2-1.9 4.4-1.9 6.6 0s4.4 1.9 6.6 0 4.4-1.9 4.8 0"
        opacity="0.62"
      />
      <path
        d="M3 18.5c2.2-1.9 4.4-1.9 6.6 0s4.4 1.9 6.6 0 4.4-1.9 4.8 0"
        opacity="0.32"
      />
    </svg>
  );
}

/**
 * Engine readiness, as a dot beside the wordmark.
 *
 * A dot on its own would be status by colour alone. It is allowed here because
 * it is a summary of the two lines in the footer below, which say the same
 * thing in words — and it still carries an accessible name so it is not
 * silent to a screen reader.
 */
const HEALTH_DOT: Record<EngineHealth, { tone: string; label: string }> = {
  starting: { tone: "bg-fg-3", label: "Engine starting" },
  connecting: { tone: "bg-warn", label: "Engine connecting" },
  ready: { tone: "bg-ok", label: "Engine ready" },
  degraded: { tone: "bg-warn", label: "Engine degraded" },
};

/** One line in the rail's network footer. */
function NetRow({ ok, children }: { ok: boolean; children: React.ReactNode }) {
  return (
    <div className="text-fg-2 flex items-center gap-[7px] text-[11px]">
      <span
        className={`h-[5px] w-[5px] shrink-0 rounded-full ${ok ? "bg-ok" : "bg-warn"}`}
        aria-hidden="true"
      />
      {children}
    </div>
  );
}

/** Props for {@link Rail}. */
export interface RailProps {
  /** The active view. */
  view: ViewId;
  /** Change the active view. */
  onViewChange: (v: ViewId) => void;
  /** How many torrents each view holds. */
  counts: Record<ViewId, number>;
  /** The current search text. */
  query: string;
  /** Search text changed. */
  onQueryChange: (q: string) => void;
  /** Session status, or `null` before the engine answers. */
  status: CoreStatus | null;
  /** Whether the first telemetry tick is still outstanding. */
  loading: boolean;
  /**
   * The egress guard's latest status, or `null` before one arrives.
   *
   * Rendered as a third line in the network footer, which is already the "what
   * is the network doing" block and already pairs a dot with words — so the
   * state stays legible without relying on colour, as the design rules require.
   */
  guard?: GuardStatus | null;
  /** Suspends the `/` shortcut, e.g. while a dialog is open. */
  searchDisabled?: boolean;
  /**
   * Whether the rail is drawn as an icon rail.
   *
   * Never a hidden rail. The network footer carries the egress guard's held
   * state, and a rail that can disappear turns a deliberate hold into
   * unexplained silence — so collapsed keeps the view icons and one status
   * dot whose accessible name carries what the footer would have said.
   */
  collapsed?: boolean;
  /** Toggle between expanded and collapsed. Omit to hide the toggle. */
  onToggleCollapsed?: () => void;
}

/**
 * The sidebar: identity, search, views, and what the network is doing.
 *
 * 248px expanded, 56px as an icon rail. The width itself lives on the page
 * grid as `--flume-rail-w`, set from `data-rail` on the document root, so
 * collapsing repaints the layout without this component owning a pixel figure.
 *
 * The footer is the part worth keeping. A BitTorrent client that cannot say
 * whether its port is reachable or how many DHT nodes it knows leaves the user
 * guessing at exactly the moment a download will not start.
 *
 * @param props - See {@link RailProps}.
 * @returns The rendered rail.
 */
export function Rail({
  view,
  onViewChange,
  counts,
  query,
  onQueryChange,
  status,
  loading,
  guard = null,
  searchDisabled = false,
  collapsed = false,
  onToggleCollapsed,
}: RailProps) {
  const search = useRef<HTMLInputElement>(null);
  const toggleRef = useRef<HTMLButtonElement>(null);
  // Set when `/` had to expand the rail first. The input does not exist until
  // the next render, so the focus waits for it rather than being lost.
  const focusWhenExpanded = useRef(false);
  // Whether the search field holds focus, maintained by the input's own focus
  // handlers. Collapsing unmounts that input, and on WKWebView -- the macOS
  // Tauri runtime -- a click does not move focus, so a user typing in the
  // field who then clicks the chevron would be left on <body> with no focus
  // ring anywhere.
  const searchHadFocus = useRef(false);

  // `select()` on its own moves focus in browsers but not everywhere, and the
  // shortcut's contract is that the caret ends up in the field. Focus first,
  // then select the existing text so typing replaces a previous query.
  const focusSearch = () => {
    search.current?.focus();
    search.current?.select();
  };

  useEffect(() => {
    if (!collapsed && focusWhenExpanded.current) {
      focusWhenExpanded.current = false;
      focusSearch();
      return;
    }
    // The field the user was in has just been unmounted. The toggle is the
    // nearest thing that can bring it back, so focus lands there rather than
    // on the document.
    if (collapsed && searchHadFocus.current) {
      searchHadFocus.current = false;
      toggleRef.current?.focus();
    }
  }, [collapsed]);

  const guardLabel = guardRailLabel(guard);
  // "Accepted" rather than "on": a pinned interface the classifier could not
  // vouch for still counts, because the user said so.
  const guardOk =
    guard?.report.verdict.verdict === "tunnelled" ||
    guard?.report.verdict.verdict === "pinned";

  // The search field owns its own shortcut rather than the page reaching in
  // through the DOM. `/` is the design's binding; it is safe as a bare key
  // because the shortcut hook already ignores keystrokes inside text controls.
  //
  // A collapsed rail has no search field, and the one thing a shortcut must
  // never do is nothing. So `/` expands the rail and focuses the field once it
  // exists, rather than swallowing the key.
  useKeyboardShortcuts(
    useMemo<Shortcut[]>(
      () => [
        {
          key: "/",
          description: "Search the library",
          run: () => {
            if (collapsed) {
              focusWhenExpanded.current = true;
              onToggleCollapsed?.();
            } else {
              focusSearch();
            }
          },
        },
      ],
      [collapsed, onToggleCollapsed],
    ),
    !searchDisabled,
  );

  const dhtNodes = status ? status.dht.nodesV4 + status.dht.nodesV6 : 0;
  const health = HEALTH_DOT[status?.health ?? "starting"];
  const pulse = loading || status?.health === "connecting";

  // Each footer line once, as a fact and as its rendering. Expanded draws the
  // renderings; collapsed joins the facts into one status dot's accessible
  // name, so the rail never carries less information than it did — only less
  // width.
  const netRows: {
    id: string;
    ok: boolean;
    text: string;
    node: React.ReactNode;
  }[] = [
    ...(guardLabel
      ? [
          {
            id: "guard",
            ok: guard?.held !== true && guardOk,
            text: guardLabel,
            node: guardLabel,
          },
        ]
      : []),
    {
      id: "dht",
      ok: dhtNodes > 0,
      text: `DHT · ${dhtNodes.toLocaleString()} nodes`,
      node: (
        <>
          DHT · <span className="flume-num">{dhtNodes.toLocaleString()}</span>{" "}
          nodes
        </>
      ),
    },
    {
      id: "port",
      ok: status?.listenPort != null,
      text:
        status?.listenPort == null
          ? "Not listening for peers"
          : `Port ${status.listenPort} open`,
      node:
        status?.listenPort == null ? (
          "Not listening for peers"
        ) : (
          <>
            Port <span className="flume-num">{status.listenPort}</span> open
          </>
        ),
    },
  ];
  const netSummary = netRows.map((row) => row.text).join(". ");
  const netOk = netRows.every((row) => row.ok);

  // One position in the tree, never wrapped. Alternating between a <button>
  // and a <span> at the same child index makes React unmount and remount it,
  // which drops the focus riding on it -- so a keyboard user who Tabs to the
  // chevron and presses Enter lands on <body> and has to start again. The
  // margin moves onto the button's own class instead of onto a wrapper.
  const toggle = onToggleCollapsed ? (
    <button
      ref={toggleRef}
      type="button"
      onClick={onToggleCollapsed}
      aria-expanded={!collapsed}
      aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
      title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
      className={`text-fg-3 hover:bg-bg-2 hover:text-fg-0 flex h-7 w-7 shrink-0 items-center justify-center rounded-sm ${collapsed ? "" : "ml-auto"}`}
    >
      <Icon name={collapsed ? "chevron-right" : "chevron-left"} size={14} />
    </button>
  ) : null;

  return (
    <div className="bg-bg-1 border-line row-start-2 flex min-h-0 flex-col border-r">
      <div
        className={`flex flex-col gap-3 pt-4 pb-2.5 ${collapsed ? "items-center px-2" : "px-3.5"}`}
      >
        <div
          className={`flex items-center gap-[9px] ${collapsed ? "flex-col gap-2" : ""}`}
        >
          <span className="flex items-center gap-[9px]">
            <Mark />
            {collapsed ? null : (
              <span className="text-[15px] font-semibold tracking-[-0.015em]">
                Flume
              </span>
            )}
            <span
              className={`ml-0.5 h-1.5 w-1.5 rounded-full ${health.tone} ${pulse ? "animate-pulse" : ""}`}
              role="status"
              aria-label={health.label}
              title={health.label}
            />
          </span>
          {toggle}
        </div>

        {collapsed ? null : (
          <div className="relative flex items-center">
            <span className="text-fg-3 pointer-events-none absolute left-[9px]">
              <Icon name="search" size={14} />
            </span>
            <input
              ref={search}
              type="text"
              value={query}
              onChange={(event) => onQueryChange(event.target.value)}
              // Reported by the input rather than sampled in an effect:
              // focus moves between renders, so a render-time check never
              // sees it. If the field is unmounted by a collapse no blur
              // fires, which leaves the flag set -- exactly the case the
              // effect below needs to catch.
              onFocus={() => {
                searchHadFocus.current = true;
              }}
              onBlur={() => {
                searchHadFocus.current = false;
              }}
              placeholder="Search library"
              aria-label="Search library"
              className="border-line bg-bg-2 text-fg-0 placeholder:text-fg-3 focus:border-acc-dim focus:bg-bg-3 h-[var(--flume-h-control)] w-full rounded-sm border pr-[46px] pl-[30px] text-[12.5px] outline-none"
            />
            <span
              className="flume-num border-line-2 text-fg-3 pointer-events-none absolute right-[7px] rounded-[3px] border px-[5px] py-0.5 text-[10px]"
              aria-hidden="true"
            >
              /
            </span>
          </div>
        )}
      </div>

      <nav
        className="flex grow flex-col gap-0.5 overflow-y-auto px-2 py-1.5"
        aria-label="Views"
      >
        {collapsed ? null : (
          <div className="text-fg-3 px-1.5 pt-3.5 pb-1.5 text-[10px] font-semibold tracking-[0.09em] uppercase">
            Views
          </div>
        )}
        {VIEWS.map((v) => {
          const active = v.id === view;
          return (
            <button
              key={v.id}
              type="button"
              aria-current={active ? "page" : undefined}
              // Once the text is gone the icon needs a name, and the count is
              // part of what the row said. A `title` alone is a tooltip, not
              // an accessible name.
              aria-label={collapsed ? `${v.name}, ${counts[v.id]}` : undefined}
              title={collapsed ? v.name : undefined}
              onClick={() => onViewChange(v.id)}
              className={`flex h-[var(--flume-h-control)] items-center rounded-sm transition-colors ${
                collapsed ? "justify-center" : "gap-[9px] px-2 text-left"
              } ${
                active
                  ? "bg-bg-3 text-fg-0"
                  : "text-fg-1 hover:bg-bg-2 hover:text-fg-0"
              }`}
            >
              <span className="shrink-0 opacity-85">
                <Icon name={v.icon} size={16} />
              </span>
              {collapsed ? null : (
                <>
                  <span className="grow text-[12.5px] font-normal">
                    {v.name}
                  </span>
                  <span
                    className={`flume-num text-[11px] ${active ? "text-fg-1" : "text-fg-3"}`}
                  >
                    {counts[v.id]}
                  </span>
                </>
              )}
            </button>
          );
        })}
      </nav>

      {collapsed ? (
        // Differentiated by shape, not hue. `--flume-ok` against
        // `--flume-warn` is 1.09:1 -- the two states are the same luminance
        // and differ only in colour, so a dot alone says nothing to a
        // protanope, on a greyscale display, or to anyone glancing past it.
        // That matters more here than anywhere: the whole reason this rail
        // collapses to 56px rather than to zero is to keep reporting a held
        // tunnel check, and a colour-only signal collapses it to zero for
        // exactly the users who can least afford it.
        //
        // So the healthy state stays a plain dot and anything else becomes a
        // stroked glyph, following the pattern `TorrentRow` already uses.
        <div className="border-line flex flex-col items-center gap-1 border-t py-3">
          <span
            className={netOk ? "text-ok" : "text-warn"}
            role="img"
            aria-label={netSummary}
            title={netSummary}
          >
            {netOk ? (
              <span className="bg-ok block h-2 w-2 rounded-full" />
            ) : (
              <Icon name="alert-triangle" size={14} />
            )}
          </span>
          <span className="text-fg-3 text-[9px] leading-none tracking-tight">
            {netOk ? "Net" : "Held"}
          </span>
        </div>
      ) : (
        <div className="border-line flex flex-col gap-2.5 border-t px-3.5 pt-3 pb-3.5">
          {netRows.map((row) => (
            // Keyed by which row it is, not by the text -- the DHT count
            // changes every second, and a text key would remount the row on
            // every telemetry tick.
            <NetRow key={row.id} ok={row.ok}>
              {row.node}
            </NetRow>
          ))}
        </div>
      )}
    </div>
  );
}
