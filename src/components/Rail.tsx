"use client";

import { useMemo, useRef } from "react";

import {
  useKeyboardShortcuts,
  type Shortcut,
} from "@/hooks/useKeyboardShortcuts";

import type { CoreStatus, EngineHealth } from "@/lib/ipc/types";
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
  /** Suspends the `/` shortcut, e.g. while a dialog is open. */
  searchDisabled?: boolean;
}

/**
 * The 248px sidebar: identity, search, views, and what the network is doing.
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
  searchDisabled = false,
}: RailProps) {
  const search = useRef<HTMLInputElement>(null);

  // The search field owns its own shortcut rather than the page reaching in
  // through the DOM. `/` is the design's binding; it is safe as a bare key
  // because the shortcut hook already ignores keystrokes inside text controls.
  useKeyboardShortcuts(
    useMemo<Shortcut[]>(
      () => [
        {
          key: "/",
          description: "Search the library",
          run: () => search.current?.select(),
        },
      ],
      [],
    ),
    !searchDisabled,
  );

  const dhtNodes = status ? status.dht.nodesV4 + status.dht.nodesV6 : 0;
  const health = HEALTH_DOT[status?.health ?? "starting"];
  const pulse = loading || status?.health === "connecting";

  return (
    <div className="bg-bg-1 border-line row-start-2 flex min-h-0 flex-col border-r">
      <div className="flex flex-col gap-3 px-3.5 pt-4 pb-2.5">
        <div className="flex items-center gap-[9px]">
          <Mark />
          <span className="text-[15px] font-semibold tracking-[-0.015em]">
            Flume
          </span>
          <span
            className={`ml-0.5 h-1.5 w-1.5 rounded-full ${health.tone} ${pulse ? "animate-pulse" : ""}`}
            role="status"
            aria-label={health.label}
            title={health.label}
          />
        </div>

        <div className="relative flex items-center">
          <span className="text-fg-3 pointer-events-none absolute left-[9px]">
            <Icon name="search" size={14} />
          </span>
          <input
            ref={search}
            type="text"
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
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
      </div>

      <nav
        className="flex grow flex-col gap-0.5 overflow-y-auto px-2 py-1.5"
        aria-label="Views"
      >
        <div className="text-fg-3 px-1.5 pt-3.5 pb-1.5 text-[10px] font-semibold tracking-[0.09em] uppercase">
          Views
        </div>
        {VIEWS.map((v) => {
          const active = v.id === view;
          return (
            <button
              key={v.id}
              type="button"
              aria-current={active ? "page" : undefined}
              onClick={() => onViewChange(v.id)}
              className={`flex h-[var(--flume-h-control)] items-center gap-[9px] rounded-sm px-2 text-left transition-colors ${
                active
                  ? "bg-bg-3 text-fg-0"
                  : "text-fg-1 hover:bg-bg-2 hover:text-fg-0"
              }`}
            >
              <span className="shrink-0 opacity-85">
                <Icon name={v.icon} size={16} />
              </span>
              <span className="grow text-[12.5px] font-normal">{v.name}</span>
              <span
                className={`flume-num text-[11px] ${active ? "text-fg-1" : "text-fg-3"}`}
              >
                {counts[v.id]}
              </span>
            </button>
          );
        })}
      </nav>

      <div className="border-line flex flex-col gap-2.5 border-t px-3.5 pt-3 pb-3.5">
        <NetRow ok={dhtNodes > 0}>
          DHT · <span className="flume-num">{dhtNodes.toLocaleString()}</span>{" "}
          nodes
        </NetRow>
        <NetRow ok={status?.listenPort != null}>
          {status?.listenPort == null ? (
            "Not listening for peers"
          ) : (
            <>
              Port <span className="flume-num">{status.listenPort}</span> open
            </>
          )}
        </NetRow>
      </div>
    </div>
  );
}
