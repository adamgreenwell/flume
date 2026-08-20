"use client";

import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useCallback, useState } from "react";

import { AddTorrentDialog } from "@/components/AddTorrentDialog";
import { Button } from "@/components/Button";
import { ConfirmRemoveDialog } from "@/components/ConfirmRemoveDialog";
import { StatusPill } from "@/components/StatusPill";
import { TorrentRow } from "@/components/TorrentRow";
import { useTelemetry } from "@/hooks/useTelemetry";
import { formatSpeed } from "@/lib/format";
import { pauseTorrent, removeTorrent, resumeTorrent } from "@/lib/ipc/client";
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
        <div className="border-border-subtle flex flex-1 flex-col items-center justify-center gap-3 rounded-xl border border-dashed py-20 text-center">
          <p className="text-text text-sm font-medium">No torrents yet</p>
          <p className="text-muted max-w-sm text-xs">
            Paste a magnet link or choose a <code>.torrent</code> file. Flume
            shows you the file list first, so you only download what you want.
          </p>
          <Button
            variant="secondary"
            onClick={() => setIsAdding(true)}
            className="mt-1"
          >
            Add your first torrent
          </Button>
        </div>
      ) : (
        <ul className="flex flex-col gap-2.5">
          {torrents.map((t) => (
            <TorrentRow
              key={t.infoHash}
              torrent={t}
              onToggle={(x) => void toggle(x)}
              onRemove={setPendingRemoval}
              onReveal={(x) => void reveal(x)}
            />
          ))}
        </ul>
      )}

      {isAdding ? (
        <AddTorrentDialog onClose={() => setIsAdding(false)} />
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
