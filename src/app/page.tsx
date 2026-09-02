"use client";

import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  Fragment,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";

import { AddTorrentDialog } from "@/components/AddTorrentDialog";
import { ColumnHeader } from "@/components/ColumnHeader";
import { ConfirmRemoveDialog } from "@/components/ConfirmRemoveDialog";
import { ContextMenu, type ContextMenuItem } from "@/components/ContextMenu";
import { Dock } from "@/components/Dock";
import { EmptyState } from "@/components/EmptyState";
import { describeGuard } from "@/lib/egress";
import { FirstRun } from "@/components/FirstRun";
import { ExpandedRow } from "@/components/ExpandedRow";
import { LibraryToolbar, type SortId } from "@/components/LibraryToolbar";
import { Rail } from "@/components/Rail";
import { NoteCard } from "@/components/NoteCard";
import { SettingsDialog } from "@/components/SettingsDialog";
import { TitleBar } from "@/components/TitleBar";
import { TorrentDetail } from "@/components/TorrentDetail";
import { TorrentRow } from "@/components/TorrentRow";
import { useEgressGuard } from "@/hooks/useEgressGuard";
import { useTelemetry } from "@/hooks/useTelemetry";
import { useThroughputHistory } from "@/hooks/useThroughputHistory";
import { useTorrentDetail } from "@/hooks/useTorrentDetail";
import {
  useKeyboardShortcuts,
  type Shortcut,
} from "@/hooks/useKeyboardShortcuts";
import { useMagnetLinks } from "@/hooks/useMagnetLinks";
import { useMenuEvents } from "@/hooks/useMenuEvents";
import { useTorrentFileDrop } from "@/hooks/useTorrentFileDrop";
import {
  detectWindowControls,
  serverWindowControls,
  subscribeToWindowControls,
} from "@/lib/platform";
import {
  VIEWS,
  matchesQuery,
  matchesView,
  viewCounts,
  type ViewId,
} from "@/lib/views";
import {
  getSettings,
  isFirstRun,
  pauseTorrent,
  removeTorrent,
  resumeTorrent,
  updateSettings,
} from "@/lib/ipc/client";
import { applyDensity, applyRail, applyTheme } from "@/lib/theme";
import {
  isCommandError,
  type Settings,
  type TorrentSummary,
} from "@/lib/ipc/types";

/**
 * The main window: session status and the torrent list.
 *
 * @returns The rendered page.
 */
export default function Home() {
  const { telemetry, error, isLoading } = useTelemetry();
  const { status: guardStatus, held } = useEgressGuard();
  const collected = useThroughputHistory(telemetry);
  // Cleared with everything else while held. Telemetry stops when the engine
  // does, so the samples would otherwise sit in the chart describing a session
  // that no longer exists.
  const history = useMemo(() => (held ? [] : collected), [held, collected]);

  // Mean download rate over the samples collected so far. The add sheet
  // estimates a finish time against it, and says which window it is using —
  // the design asks for a 7-day rolling average, which Flume does not persist
  // across sessions yet.
  const averageDownBps = useMemo(
    () =>
      history.length === 0
        ? null
        : history.reduce((sum, s) => sum + s.downBps, 0) / history.length,
    [history],
  );
  const [isAdding, setIsAdding] = useState(false);
  const [pendingRemoval, setPendingRemoval] = useState<TorrentSummary | null>(
    null,
  );
  const [actionError, setActionError] = useState<string | null>(null);
  const [isConfiguring, setIsConfiguring] = useState(false);
  const [detailOf, setDetailOf] = useState<TorrentSummary | null>(null);
  const [droppedPath, setDroppedPath] = useState<string | undefined>(undefined);
  const [incomingMagnet, setIncomingMagnet] = useState<string | undefined>(
    undefined,
  );
  const [menu, setMenu] = useState<{
    torrent: TorrentSummary;
    at: { x: number; y: number };
  } | null>(null);
  const [view, setView] = useState<ViewId>("all");
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<SortId>("activity");
  // Mirrors the persisted `ui.density` setting. Held here so the toolbar chip
  // stays instant, and written back so the choice survives a restart.
  const [compact, setCompact] = useState(false);
  const [railCollapsed, setRailCollapsed] = useState(false);
  const [selectedHash, setSelectedHash] = useState<string | null>(null);
  // Read through useSyncExternalStore rather than an effect: the value differs
  // between the static export and the client, and this is the one pattern that
  // handles that without a hydration mismatch or a cascading render.
  const [downloadLimitBps, setDownloadLimitBps] = useState<number | null>(null);
  // `null` until the engine answers. The library is not rendered in the
  // meantime: flashing the empty state and then replacing it with a first-run
  // screen would be a worse first impression than a beat of nothing.
  const [firstRun, setFirstRun] = useState<boolean | null>(null);
  const controls = useSyncExternalStore(
    subscribeToWindowControls,
    detectWindowControls,
    serverWindowControls,
  );

  // Density lives on <html> so a row can read it as a CSS variable rather than
  // every row branching on a prop.
  useEffect(() => {
    applyDensity(compact ? "compact" : "comfortable");
  }, [compact]);

  // Same shape as density: the width lives on <html> as `--flume-rail-w`, so
  // the grid below reads it and nothing in the tree re-renders to move.
  useEffect(() => {
    applyRail(railCollapsed ? "collapsed" : "expanded");
  }, [railCollapsed]);

  // `update_settings` takes the whole object, so every writer has to read
  // first. Two of those interleaved lose one of the changes: toggle the rail
  // and hit the density chip in the same second and whichever read happened
  // first writes its stale copy last. Chaining them means the second read sees
  // what the first wrote.
  const settingsWrite = useRef<Promise<unknown>>(Promise.resolve());
  const patchSettings = useCallback((patch: Partial<Settings>) => {
    settingsWrite.current = settingsWrite.current
      // A failed write must not poison the queue for every write after it.
      .catch(() => {})
      .then(() => getSettings())
      .then((s) => updateSettings({ ...s, ...patch }))
      .catch(() => {
        // The change still took effect for this session; it just will not
        // survive a relaunch. Not worth an error banner.
      });
    return settingsWrite.current;
  }, []);

  const toggleRail = useCallback(() => {
    const next = !railCollapsed;
    // Applied to the layout on this render; persisted behind the queue.
    setRailCollapsed(next);
    void patchSettings({ rail: next ? "collapsed" : "expanded" });
  }, [railCollapsed, patchSettings]);

  // A magnet clicked in a browser, or passed on the command line, opens the
  // add dialog prefilled rather than adding silently -- the file-selection
  // step is the whole point of the add flow.
  useMagnetLinks(
    useCallback((uri: string) => {
      setIncomingMagnet(uri);
      setDroppedPath(undefined);
      setIsAdding(true);
    }, []),
  );

  // The native menu's Settings… and Add Torrent… items. They open the same
  // surfaces as the in-app controls rather than a second path to them.
  useMenuEvents(
    useCallback((action) => {
      if (action === "open_settings") setIsConfiguring(true);
      else setIsAdding(true);
    }, []),
  );

  // Dropping a .torrent anywhere on the window opens the add dialog with it.
  const { isDraggingTorrent } = useTorrentFileDrop(
    useCallback((path: string) => {
      setDroppedPath(path);
      setIsAdding(true);
    }, []),
  );

  // Apply the persisted theme once the engine can answer. Until then the
  // stylesheet's own `prefers-color-scheme` default is in force, so there is
  // no flash of the wrong palette.
  useEffect(() => {
    void isFirstRun()
      .then(setFirstRun)
      .catch(() => setFirstRun(false));
  }, []);

  useEffect(() => {
    void getSettings()
      .then((s) => {
        applyTheme(s.theme);
        setCompact(s.density === "compact");
        setRailCollapsed(s.rail === "collapsed");
        // Kept so the chart can draw the ceiling at the configured limit
        // rather than rescaling its axis as traffic varies.
        setDownloadLimitBps(s.downloadLimitBps);
      })
      .catch(() => {
        // Engine still starting; the system default remains in force.
      });
  }, []);

  // Cleared while held, for the same reason the torrent list is: there is no
  // session, so the DHT count, the listening port and the uptime are the last
  // reading taken before it stopped rather than facts about now. The rail
  // renders a null status as "not listening", which is exactly true.
  const status = held ? null : (telemetry?.core ?? null);
  const guardNote = useMemo(() => describeGuard(guardStatus), [guardStatus]);

  // Cleared while held rather than left frozen. Stopping the engine stops
  // telemetry, and `useTelemetry` has no staleness path -- so the last
  // snapshot would stay mounted showing live-looking rates, peer counts and a
  // green listening port for a session that no longer exists. An empty list
  // under a note that explains it is a true statement; frozen numbers are a
  // confident wrong one.
  const torrents = useMemo(
    () => (held ? [] : (telemetry?.torrents ?? [])),
    [telemetry, held],
  );

  const counts = useMemo(() => viewCounts(torrents), [torrents]);

  const visible = useMemo(() => {
    const filtered = torrents.filter(
      (t) => matchesView(t, view) && matchesQuery(t, query),
    );

    // Sorted into a copy: `torrents` is telemetry state, and sorting it in
    // place would mutate what the next tick diffs against.
    return [...filtered].sort((a, b) => {
      switch (sort) {
        case "size":
          return b.totalBytes - a.totalBytes;
        // Ids increase as torrents are added, so they are the arrival order.
        case "added":
          return b.id - a.id;
        // Busiest first, then by name so the order is stable when idle rather
        // than reshuffling on every tick.
        case "activity": {
          const byRate =
            b.downloadBps + b.uploadBps - (a.downloadBps + a.uploadBps);
          return byRate !== 0 ? byRate : a.name.localeCompare(b.name);
        }
      }
    });
  }, [torrents, view, query, sort]);

  // Only the open row is polled, and only while it is open. `null` stops the
  // polling entirely rather than fetching detail nobody is looking at.
  const expandedId =
    visible.find((t) => t.infoHash === selectedHash)?.id ?? null;
  const expanded = useTorrentDetail(expandedId);

  // The inspector polls independently of the expanded row: they can be open at
  // once, on different torrents.
  const inspected = useTorrentDetail(detailOf?.id ?? null);

  const report = useCallback((caught: unknown, fallback: string) => {
    setActionError(isCommandError(caught) ? caught.message : fallback);
  }, []);

  const toggle = useCallback(
    async (t: TorrentSummary) => {
      setActionError(null);
      try {
        if (t.state === "paused") await resumeTorrent(t.id);
        else await pauseTorrent(t.id);
      } catch (caught: unknown) {
        report(caught, "Could not change that torrent.");
      }
    },
    [report],
  );

  const reveal = useCallback(
    async (t: TorrentSummary) => {
      setActionError(null);
      try {
        await revealItemInDir(t.outputFolder);
      } catch (caught: unknown) {
        report(caught, "Could not open that folder.");
      }
    },
    [report],
  );

  const confirmRemoval = useCallback(
    async (deleteFiles: boolean) => {
      if (!pendingRemoval) return;
      setActionError(null);
      try {
        await removeTorrent(pendingRemoval.id, deleteFiles);
      } catch (caught: unknown) {
        report(caught, "Could not remove that torrent.");
      } finally {
        setPendingRemoval(null);
      }
    },
    [pendingRemoval, report],
  );

  const anyDialogOpen =
    isAdding || isConfiguring || detailOf !== null || pendingRemoval !== null;

  // Suspended while a dialog is open: those handle their own keys (Escape to
  // close), and a background shortcut firing behind a modal is disorienting.
  useKeyboardShortcuts(
    useMemo<Shortcut[]>(
      () => [
        {
          key: "n",
          meta: true,
          description: "Add a torrent",
          run: () => setIsAdding(true),
        },
        {
          key: ",",
          meta: true,
          description: "Open settings",
          run: () => setIsConfiguring(true),
        },
      ],
      [],
    ),
    !anyDialogOpen,
  );

  if (firstRun === null) {
    // One beat of the window's own background, rather than a flash of a
    // library that may be about to be replaced.
    return <div className="bg-bg-0 h-full" />;
  }

  if (firstRun) {
    return (
      <FirstRun
        onDone={() => {
          setFirstRun(false);
          // The screen has been writing settings as it went, so by this point
          // the file exists and the next launch will not show it again.
          void getSettings()
            .then((s) => {
              applyTheme(s.theme);
              setCompact(s.density === "compact");
              setRailCollapsed(s.rail === "collapsed");
              setDownloadLimitBps(s.downloadLimitBps);
            })
            .catch(() => {
              // The library renders from telemetry regardless.
            });
        }}
      />
    );
  }

  return (
    <div className="grid h-full grid-cols-[var(--flume-rail-w)_1fr] grid-rows-[44px_1fr] overflow-hidden">
      <TitleBar
        controls={controls}
        downloadBps={status?.downloadBps ?? 0}
        uploadBps={status?.uploadBps ?? 0}
      />

      <Rail
        view={view}
        onViewChange={setView}
        counts={counts}
        query={query}
        onQueryChange={setQuery}
        status={status}
        loading={isLoading}
        guard={guardStatus}
        searchDisabled={anyDialogOpen}
        collapsed={railCollapsed}
        onToggleCollapsed={toggleRail}
      />

      <div className="row-start-2 flex min-h-0 min-w-0 flex-col">
        <LibraryToolbar
          title={VIEWS.find((v) => v.id === view)?.name ?? "All torrents"}
          count={visible.length}
          sort={sort}
          onSortChange={setSort}
          compact={compact}
          onDensityToggle={() => {
            const next = !compact;
            setCompact(next);
            // Persisted so the choice survives a restart, and so the settings
            // screen and this chip cannot disagree about what density is.
            // Through the same queue as the rail toggle: both are whole-object
            // writes, and interleaving them loses one of the two changes.
            void patchSettings({
              density: next ? "compact" : "comfortable",
            });
          }}
          onAdd={() => setIsAdding(true)}
        />

        {/*
          Suppressed while held, because the empty state below is already
          showing this exact note -- holding clears the library, so the two
          rendered the same title and body one above the other, which reads as
          a bug rather than as emphasis. In Warn mode the library still has
          rows, so the banner is the only place the note can go.
        */}
        {guardNote && !held ? (
          <div className="border-line border-b px-[18px] py-2.5">
            <NoteCard note={guardNote} />
          </div>
        ) : null}

        {/*
          Suppressed while the guard holds. `get_telemetry` rejects with
          `engineNotReady`, whose message is "The torrent engine is still
          starting." -- true during startup and false for the whole duration of
          a hold, where Flume has deliberately refused to start it. Leaving it
          up would have the app contradict the note directly above it.
        */}
        {error && !held ? (
          <div
            className="border-line bg-warn/10 text-warn border-b px-[18px] py-2.5 text-xs"
            role="alert"
          >
            {error}
          </div>
        ) : null}

        {actionError ? (
          <div
            className="border-line bg-err/10 text-err flex items-start justify-between gap-3 border-b px-[18px] py-2.5 text-xs"
            role="alert"
          >
            <span>{actionError}</span>
            <button
              type="button"
              onClick={() => setActionError(null)}
              className="text-err/70 hover:text-err shrink-0"
              aria-label="Dismiss error"
            >
              ✕
            </button>
          </div>
        ) : null}

        <ColumnHeader />

        {visible.length === 0 ? (
          <div className="flex grow items-center justify-center">
            <EmptyState
              status={status}
              onAdd={() => setIsAdding(true)}
              filtered={torrents.length > 0}
              guardNote={held ? guardNote : null}
              onOpenSettings={() => setIsConfiguring(true)}
            />
          </div>
        ) : (
          <div
            role="grid"
            aria-label="Torrents"
            aria-rowcount={visible.length}
            className="flex grow flex-col overflow-y-auto"
          >
            {visible.map((t) => (
              <Fragment key={t.infoHash}>
                <TorrentRow
                  torrent={t}
                  selected={t.infoHash === selectedHash}
                  onSelect={(x) =>
                    setSelectedHash((current) =>
                      current === x.infoHash ? null : x.infoHash,
                    )
                  }
                  onOpen={setDetailOf}
                  onContextMenu={(x, at) => setMenu({ torrent: x, at })}
                />
                {t.infoHash === selectedHash ? (
                  <ExpandedRow
                    torrent={t}
                    detail={expanded.detail}
                    error={expanded.error}
                    onToggle={(x) => void toggle(x)}
                    onReveal={(x) => void reveal(x)}
                    onOpen={setDetailOf}
                  />
                ) : null}
              </Fragment>
            ))}
          </div>
        )}

        <Dock
          status={status}
          torrents={torrents}
          history={history}
          limitBps={downloadLimitBps}
          held={held}
        />
      </div>

      {isAdding ? (
        <AddTorrentDialog
          droppedPath={droppedPath}
          initialMagnet={incomingMagnet}
          rateBps={averageDownBps}
          onClose={() => {
            setIsAdding(false);
            setDroppedPath(undefined);
            setIncomingMagnet(undefined);
          }}
        />
      ) : null}

      {isDraggingTorrent ? (
        <div
          className="border-acc bg-acc/10 pointer-events-none fixed inset-4 z-50 flex items-center justify-center rounded-xl border-2 border-dashed"
          role="status"
        >
          <p className="text-acc text-sm font-medium">
            Drop to add this torrent
          </p>
        </div>
      ) : null}

      {menu ? (
        <ContextMenu
          position={menu.at}
          onClose={() => setMenu(null)}
          items={
            [
              {
                label: menu.torrent.state === "paused" ? "Resume" : "Pause",
                icon: menu.torrent.state === "paused" ? "play" : "pause",
                run: () => void toggle(menu.torrent),
              },
              {
                label: "Files and details",
                icon: "files",
                run: () => setDetailOf(menu.torrent),
              },
              {
                label: "Open containing folder",
                icon: "folder",
                run: () => void reveal(menu.torrent),
              },
              {
                label: "Remove",
                icon: "trash",
                destructive: true,
                run: () => setPendingRemoval(menu.torrent),
              },
            ] satisfies ContextMenuItem[]
          }
        />
      ) : null}

      {detailOf ? (
        <TorrentDetail
          torrent={
            // Re-read from telemetry each tick so the inspector's numbers move
            // while it is open, rather than freezing at whatever the row held
            // when it was clicked.
            torrents.find((t) => t.infoHash === detailOf.infoHash) ?? detailOf
          }
          detail={inspected.detail}
          tick={status?.uptimeSeconds ?? null}
          limitBps={downloadLimitBps}
          onClose={() => setDetailOf(null)}
        />
      ) : null}

      {isConfiguring ? (
        <SettingsDialog onClose={() => setIsConfiguring(false)} />
      ) : null}

      {pendingRemoval ? (
        <ConfirmRemoveDialog
          torrent={pendingRemoval}
          onConfirm={(deleteFiles) => void confirmRemoval(deleteFiles)}
          onCancel={() => setPendingRemoval(null)}
        />
      ) : null}
    </div>
  );
}
