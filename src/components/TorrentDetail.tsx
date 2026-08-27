"use client";

import { useCallback, useEffect, useState } from "react";

import { formatBytes } from "@/lib/format";
import {
  getTorrentDetail,
  getTorrentFiles,
  setOnlyFiles,
} from "@/lib/ipc/client";
import {
  isCommandError,
  type TorrentDetail as TorrentDetailData,
  type TorrentFileState,
  type TorrentSummary,
} from "@/lib/ipc/types";

import { Button } from "./Button";
import { FragmentStrip } from "./FragmentStrip";
import { PeerList } from "./PeerList";
import { Skeleton } from "./Skeleton";
import { SwarmSummary } from "./SwarmSummary";
import { PieceHeatmap } from "./PieceHeatmap";
import { ProgressBar } from "./ProgressBar";
import { TrackerList } from "./TrackerList";

/** Tabs available in the detail panel. */
const TABS = ["files", "peers", "trackers", "pieces"] as const;

/** One of {@link TABS}. */
export type DetailTab = (typeof TABS)[number];

/** How often peers and piece data refresh while the panel is open. */
const DETAIL_REFRESH_MS = 2000;

/** Props for {@link TorrentDetail}. */
export interface TorrentDetailProps {
  /** The torrent being inspected. */
  torrent: TorrentSummary;
  /** Called when the panel should close. */
  onClose: () => void;
}

/**
 * Per-torrent detail: files, peers, trackers, and a piece map.
 *
 * All of this is fetched on demand rather than streamed in telemetry. It is
 * per-torrent and only interesting while the panel is open, so pushing it to
 * every client every second would grow the telemetry payload with the torrent
 * count for no benefit.
 *
 * Peers and pieces refresh on their own slower interval while open, since they
 * change continuously; the file list only refreshes after an edit.
 *
 * @param props - See {@link TorrentDetailProps}.
 * @returns The rendered panel.
 */
export function TorrentDetail({ torrent, onClose }: TorrentDetailProps) {
  const [tab, setTab] = useState<DetailTab>("files");
  const [files, setFiles] = useState<TorrentFileState[] | null>(null);
  const [detail, setDetail] = useState<TorrentDetailData | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  const describe = (caught: unknown, fallback: string) =>
    isCommandError(caught) ? caught.message : fallback;

  const applyLoaded = useCallback((loaded: TorrentFileState[]) => {
    setFiles(loaded);
    setSelected(new Set(loaded.filter((f) => f.selected).map((f) => f.index)));
    setError(null);
  }, []);

  // Files load once per torrent; they only change when the user edits them.
  useEffect(() => {
    let active = true;
    getTorrentFiles(torrent.id)
      .then((loaded) => {
        if (active) applyLoaded(loaded);
      })
      .catch((caught: unknown) => {
        if (active) {
          setError(describe(caught, "Could not read this torrent's files."));
        }
      });
    return () => {
      active = false;
    };
  }, [torrent.id, applyLoaded]);

  // Peers and pieces move constantly, so poll them while the panel is open.
  // Chained timeouts rather than an interval, so a slow call cannot stack.
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;

    const tick = () => {
      getTorrentDetail(torrent.id)
        .then((next) => {
          if (active) setDetail(next);
        })
        .catch(() => {
          // Transient while a torrent restarts; the next tick recovers.
        })
        .finally(() => {
          if (active) timer = setTimeout(tick, DETAIL_REFRESH_MS);
        });
    };
    tick();

    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [torrent.id]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const originalSelection = new Set(
    (files ?? []).filter((f) => f.selected).map((f) => f.index),
  );
  const isDirty =
    files !== null &&
    (selected.size !== originalSelection.size ||
      [...selected].some((i) => !originalSelection.has(i)));

  const save = async () => {
    setIsSaving(true);
    setError(null);
    try {
      await setOnlyFiles(
        torrent.id,
        [...selected].sort((a, b) => a - b),
      );
      // Re-read rather than assuming the write took: the engine may normalise
      // the selection, and progress has moved on since the panel opened.
      applyLoaded(await getTorrentFiles(torrent.id));
    } catch (caught: unknown) {
      setError(describe(caught, "Could not change the file selection."));
    } finally {
      setIsSaving(false);
    }
  };

  const toggle = (index: number) => {
    const next = new Set(selected);
    if (next.has(index)) next.delete(index);
    else next.add(index);
    setSelected(next);
  };

  const counts: Record<DetailTab, number | null> = {
    files: files?.length ?? null,
    peers: detail?.peers.length ?? null,
    trackers: detail?.trackers.length ?? null,
    pieces: null,
  };

  return (
    <div
      className="fixed inset-0 z-40 flex items-center justify-center bg-black/60 p-6"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={`Details for ${torrent.name}`}
        className="border-line bg-bg-1 flex max-h-[82vh] w-full max-w-2xl flex-col gap-4 rounded-xl border p-5 shadow-2xl"
      >
        <div className="min-w-0">
          <h2
            className="text-fg-0 truncate text-base font-semibold"
            title={torrent.name}
          >
            {torrent.name}
          </h2>
          <p className="text-fg-2 mt-0.5 text-xs">
            <span className="font-mono tabular-nums">
              {formatBytes(torrent.progressBytes)} /{" "}
              {formatBytes(torrent.totalBytes)}
            </span>
          </p>
        </div>

        <div
          className="border-line flex gap-1 border-b"
          role="tablist"
          aria-label="Torrent details"
        >
          {TABS.map((name) => (
            <button
              key={name}
              type="button"
              role="tab"
              aria-selected={tab === name}
              onClick={() => setTab(name)}
              className={`-mb-px border-b-2 px-3 py-2 text-sm capitalize transition-colors ${
                tab === name
                  ? "border-acc text-fg-0"
                  : "text-fg-2 hover:text-fg-0 border-transparent"
              }`}
            >
              {name}
              {counts[name] !== null ? (
                <span className="text-fg-3 ml-1.5 font-mono text-xs">
                  {counts[name]}
                </span>
              ) : null}
            </button>
          ))}
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto" role="tabpanel">
          {tab === "files" ? (
            files === null ? (
              <Skeleton rows={3} label="Loading files" />
            ) : (
              <ul className="border-line bg-bg-0 rounded-md border">
                {files.map((file) => (
                  <li
                    key={file.index}
                    className="border-line border-b px-3 py-2.5 last:border-b-0"
                  >
                    <label className="flex cursor-pointer items-center gap-3">
                      <input
                        type="checkbox"
                        checked={selected.has(file.index)}
                        onChange={() => toggle(file.index)}
                        className="accent-acc h-4 w-4 shrink-0"
                      />
                      <span
                        className={`min-w-0 flex-1 truncate text-sm ${selected.has(file.index) ? "text-fg-0" : "text-fg-3"}`}
                        title={file.path}
                      >
                        {file.path}
                      </span>
                      <span className="text-fg-2 shrink-0 font-mono text-xs tabular-nums">
                        {formatBytes(file.progressBytes)} /{" "}
                        {formatBytes(file.length)}
                      </span>
                    </label>
                    <div className="mt-1.5 flex flex-col gap-1 pl-7">
                      <ProgressBar
                        value={
                          file.length === 0
                            ? 1
                            : file.progressBytes / file.length
                        }
                        state={torrent.state}
                        label={`${file.path} progress`}
                      />
                      {/* Overall progress says how much; the fragment strip
                          says which parts, which is what matters when a
                          download stalls against a partial swarm. */}
                      <FragmentStrip
                        buckets={file.pieceBuckets}
                        label={file.path}
                      />
                      {file.pieceBuckets.length > 0 ? (
                        <p className="text-fg-3 text-[10px]">
                          pieces {file.firstPiece.toLocaleString()}–
                          {(file.lastPiece - 1).toLocaleString()}
                        </p>
                      ) : null}
                    </div>
                  </li>
                ))}
              </ul>
            )
          ) : null}

          {tab === "peers" ? (
            <div className="flex flex-col gap-3">
              {detail ? <SwarmSummary swarm={detail.swarm} /> : null}
              <PeerList peers={detail?.peers ?? []} />
            </div>
          ) : null}

          {tab === "trackers" ? (
            <TrackerList trackers={detail?.trackers ?? []} />
          ) : null}

          {tab === "pieces" ? (
            detail?.pieces ? (
              <PieceHeatmap pieces={detail.pieces} />
            ) : (
              <p className="text-fg-3 py-6 text-center text-xs">
                Piece information appears once the torrent is running or paused.
              </p>
            )
          ) : null}
        </div>

        {error ? (
          <p
            className="border-err/30 bg-err/10 text-err rounded-md border px-3 py-2 text-xs"
            role="alert"
          >
            {error}
          </p>
        ) : null}

        <div className="flex items-center justify-between gap-2">
          <p className="text-fg-3 text-xs">
            {tab === "files" && isDirty
              ? "Deselected files stop downloading. Existing data is kept."
              : ""}
          </p>
          <div className="flex gap-2">
            <Button variant="ghost" onClick={onClose}>
              Close
            </Button>
            {tab === "files" ? (
              <Button
                variant="primary"
                onClick={() => void save()}
                disabled={!isDirty || selected.size === 0 || isSaving}
              >
                {isSaving ? "Saving…" : "Save selection"}
              </Button>
            ) : null}
          </div>
        </div>
      </div>
    </div>
  );
}
