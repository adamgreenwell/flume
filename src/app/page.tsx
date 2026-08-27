"use client";

import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  Fragment,
  useCallback,
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
} from "react";

import { AddTorrentDialog } from "@/components/AddTorrentDialog";
import { ColumnHeader } from "@/components/ColumnHeader";
import { ConfirmRemoveDialog } from "@/components/ConfirmRemoveDialog";
import { ContextMenu, type ContextMenuItem } from "@/components/ContextMenu";
import { Dock } from "@/components/Dock";
import { EmptyState } from "@/components/EmptyState";
import { ExpandedRow } from "@/components/ExpandedRow";
import { LibraryToolbar, type SortId } from "@/components/LibraryToolbar";
import { Rail } from "@/components/Rail";
import { SettingsDialog } from "@/components/SettingsDialog";
import { TitleBar } from "@/components/TitleBar";
import { TorrentDetail } from "@/components/TorrentDetail";
import { TorrentRow } from "@/components/TorrentRow";
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
  pauseTorrent,
  removeTorrent,
  resumeTorrent,
  updateSettings,
} from "@/lib/ipc/client";
import { applyDensity, applyTheme } from "@/lib/theme";
import { isCommandError, type TorrentSummary } from "@/lib/ipc/types";

/**
 * The main window: session status and the torrent list.
 *
 * @returns The rendered page.
 */
export default function Home() {
  const { telemetry, error, isLoading } = useTelemetry();
  const history = useThroughputHistory(telemetry);

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
  const [selectedHash, setSelectedHash] = useState<string | null>(null);
  // Read through useSyncExternalStore rather than an effect: the value differs
  // between the static export and the client, and this is the one pattern that
  // handles that without a hydration mismatch or a cascading render.
  const [downloadLimitBps, setDownloadLimitBps] = useState<number | null>(null);
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
    void getSettings()
      .then((s) => {
        applyTheme(s.theme);
        setCompact(s.density === "compact");
        // Kept so the chart can draw the ceiling at the configured limit
        // rather than rescaling its axis as traffic varies.
        setDownloadLimitBps(s.downloadLimitBps);
      })
      .catch(() => {
        // Engine still starting; the system default remains in force.
      });
  }, []);

  const status = telemetry?.core ?? null;
  const torrents = useMemo(() => telemetry?.torrents ?? [], [telemetry]);

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

  return (
    <div className="grid h-full grid-cols-[248px_1fr] grid-rows-[44px_1fr] overflow-hidden">
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
        searchDisabled={anyDialogOpen}
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
            void getSettings()
              .then((s) =>
                updateSettings({
                  ...s,
                  density: next ? "compact" : "comfortable",
                }),
              )
              .catch(() => {
                // A density that failed to persist still applied locally;
                // it is not worth an error banner over the list.
              });
          }}
          onAdd={() => setIsAdding(true)}
        />

        {error ? (
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
