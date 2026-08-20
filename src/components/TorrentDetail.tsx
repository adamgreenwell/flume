"use client";

import { useCallback, useEffect, useState } from "react";

import { formatBytes } from "@/lib/format";
import { getTorrentFiles, setOnlyFiles } from "@/lib/ipc/client";
import {
  isCommandError,
  type TorrentFileState,
  type TorrentSummary,
} from "@/lib/ipc/types";

import { Button } from "./Button";
import { ProgressBar } from "./ProgressBar";

/** Props for {@link TorrentDetail}. */
export interface TorrentDetailProps {
  /** The torrent whose files are shown. */
  torrent: TorrentSummary;
  /** Called when the panel should close. */
  onClose: () => void;
}

/**
 * Per-torrent detail: the file list, with selection editable after the fact.
 *
 * File progress is fetched on open rather than streamed in telemetry. A file
 * list is per-torrent and only visible while this panel is open, so pushing it
 * to every client every second would be exactly the kind of unbounded payload
 * the telemetry design avoids.
 *
 * @param props - See {@link TorrentDetailProps}.
 * @returns The rendered panel.
 */
export function TorrentDetail({ torrent, onClose }: TorrentDetailProps) {
  const [files, setFiles] = useState<TorrentFileState[] | null>(null);
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

  // Fetch on open, and whenever the panel is pointed at a different torrent.
  // The `active` guard matters: closing the panel or switching torrents while
  // a fetch is in flight must not apply the stale result.
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
        aria-label={`Files in ${torrent.name}`}
        className="border-border-subtle bg-surface flex max-h-[80vh] w-full max-w-2xl flex-col gap-4 rounded-xl border p-5 shadow-2xl"
      >
        <div className="min-w-0">
          <h2
            className="text-text truncate text-base font-semibold"
            title={torrent.name}
          >
            {torrent.name}
          </h2>
          <p className="text-muted mt-0.5 text-xs">
            {files?.length ?? 0} file{files?.length === 1 ? "" : "s"} ·{" "}
            <span className="font-mono">{formatBytes(torrent.totalBytes)}</span>
          </p>
        </div>

        {files === null ? (
          <p className="text-muted text-sm">Loading files…</p>
        ) : (
          <ul className="border-border-subtle bg-bg min-h-0 flex-1 overflow-y-auto rounded-md border">
            {files.map((file) => {
              const fraction =
                file.length === 0 ? 1 : file.progressBytes / file.length;
              return (
                <li
                  key={file.index}
                  className="border-border-subtle border-b px-3 py-2.5 last:border-b-0"
                >
                  <label className="flex cursor-pointer items-center gap-3">
                    <input
                      type="checkbox"
                      checked={selected.has(file.index)}
                      onChange={() => toggle(file.index)}
                      className="accent-accent h-4 w-4 shrink-0"
                    />
                    <span
                      className={`min-w-0 flex-1 truncate text-sm ${selected.has(file.index) ? "text-text" : "text-faint"}`}
                      title={file.path}
                    >
                      {file.path}
                    </span>
                    <span className="text-muted shrink-0 font-mono text-xs tabular-nums">
                      {formatBytes(file.progressBytes)} /{" "}
                      {formatBytes(file.length)}
                    </span>
                  </label>
                  <div className="mt-1.5 pl-7">
                    <ProgressBar
                      value={fraction}
                      state={torrent.state}
                      label={`${file.path} progress`}
                    />
                  </div>
                </li>
              );
            })}
          </ul>
        )}

        {error ? (
          <p
            className="border-error/30 bg-error/10 text-error rounded-md border px-3 py-2 text-xs"
            role="alert"
          >
            {error}
          </p>
        ) : null}

        <div className="flex items-center justify-between gap-2">
          <p className="text-faint text-xs">
            {isDirty
              ? "Deselected files stop downloading. Existing data is kept."
              : ""}
          </p>
          <div className="flex gap-2">
            <Button variant="ghost" onClick={onClose}>
              Close
            </Button>
            <Button
              variant="primary"
              onClick={() => void save()}
              disabled={!isDirty || selected.size === 0 || isSaving}
            >
              {isSaving ? "Saving…" : "Save selection"}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
