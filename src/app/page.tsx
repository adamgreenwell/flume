"use client";

import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useState } from "react";

import { AddTorrentDialog } from "@/components/AddTorrentDialog";
import { Button } from "@/components/Button";
import { ConfirmRemoveDialog } from "@/components/ConfirmRemoveDialog";
import { EmptyState } from "@/components/EmptyState";
import { SettingsDialog } from "@/components/SettingsDialog";
import { TorrentDetail } from "@/components/TorrentDetail";
import { StatusPill } from "@/components/StatusPill";
import { TorrentRow } from "@/components/TorrentRow";
import { useTelemetry } from "@/hooks/useTelemetry";
import { useTorrentFileDrop } from "@/hooks/useTorrentFileDrop";
import { formatSpeed } from "@/lib/format";
import {
  getSettings,
  pauseTorrent,
  removeTorrent,
  resumeTorrent,
} from "@/lib/ipc/client";
import { applyTheme } from "@/lib/theme";
import { isCommandError, type TorrentSummary } from "@/lib/ipc/types";

/**
 * The main window: session status and the torrent list.
 *
 * @returns The rendered page.
 */
export default function Home() {
  const { telemetry, error, isLoading } = useTelemetry();
  const [isAdding, setIsAdding] = useState(false);
  const [pendingRemoval, setPendingRemoval] = useState<TorrentSummary | null>(
    null,
  );
  const [actionError, setActionError] = useState<string | null>(null);
  const [isConfiguring, setIsConfiguring] = useState(false);
  const [detailOf, setDetailOf] = useState<TorrentSummary | null>(null);
  const [droppedPath, setDroppedPath] = useState<string | undefined>(undefined);

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
      .then((s) => applyTheme(s.theme))
      .catch(() => {
        // Engine still starting; the system default remains in force.
      });
  }, []);

  const status = telemetry?.core ?? null;
  const torrents = telemetry?.torrents ?? [];

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

  return (
    <main className="mx-auto flex min-h-full w-full max-w-4xl flex-col gap-6 px-8 py-10">
      <header className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-text text-2xl font-semibold tracking-tight">
            Flume
          </h1>
          <p className="text-muted mt-0.5 text-sm">
            {status ? (
              <>
                <span className="font-mono tabular-nums">
                  {formatSpeed(status.downloadBps)}
                </span>{" "}
                down ·{" "}
                <span className="font-mono tabular-nums">
                  {formatSpeed(status.uploadBps)}
                </span>{" "}
                up · {status.livePeers} peers
              </>
            ) : (
              "A beautiful, cross-platform BitTorrent client."
            )}
          </p>
        </div>
        <div className="flex items-center gap-3">
          <StatusPill
            health={status?.health ?? "starting"}
            pulse={isLoading || status?.health === "connecting"}
          />
          <Button variant="ghost" onClick={() => setIsConfiguring(true)}>
            Settings
          </Button>
          <Button variant="primary" onClick={() => setIsAdding(true)}>
            Add torrent
          </Button>
        </div>
      </header>

      {error ? (
        <div
          className="border-warn/30 bg-warn/10 text-warn rounded-lg border px-4 py-3 text-sm"
          role="alert"
        >
          {error}
        </div>
      ) : null}

      {actionError ? (
        <div
          className="border-error/30 bg-error/10 text-error flex items-start justify-between gap-3 rounded-lg border px-4 py-3 text-sm"
          role="alert"
        >
          <span>{actionError}</span>
          <button
            type="button"
            onClick={() => setActionError(null)}
            className="text-error/70 hover:text-error shrink-0"
            aria-label="Dismiss error"
          >
            ✕
          </button>
        </div>
      ) : null}

      {torrents.length === 0 ? (
        <EmptyState status={status} onAdd={() => setIsAdding(true)} />
      ) : (
        <ul className="flex flex-col gap-2.5">
          {torrents.map((t) => (
            <TorrentRow
              key={t.infoHash}
              torrent={t}
              onToggle={(x) => void toggle(x)}
              onRemove={setPendingRemoval}
              onReveal={(x) => void reveal(x)}
              onOpenDetail={setDetailOf}
            />
          ))}
        </ul>
      )}

      {isAdding ? (
        <AddTorrentDialog
          droppedPath={droppedPath}
          onClose={() => {
            setIsAdding(false);
            setDroppedPath(undefined);
          }}
        />
      ) : null}

      {isDraggingTorrent ? (
        <div
          className="border-accent bg-accent/10 pointer-events-none fixed inset-4 z-50 flex items-center justify-center rounded-xl border-2 border-dashed"
          role="status"
        >
          <p className="text-accent text-sm font-medium">
            Drop to add this torrent
          </p>
        </div>
      ) : null}

      {detailOf ? (
        <TorrentDetail torrent={detailOf} onClose={() => setDetailOf(null)} />
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
    </main>
  );
}
