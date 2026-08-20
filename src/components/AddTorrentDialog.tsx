"use client";

import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";

import { formatBytes } from "@/lib/format";
import { confirmAdd, discardPreview, previewTorrent } from "@/lib/ipc/client";
import { isCommandError, type TorrentPreview } from "@/lib/ipc/types";

import { Button } from "./Button";
import { FileTree } from "./FileTree";

/** Props for {@link AddTorrentDialog}. */
export interface AddTorrentDialogProps {
  /** Called when the dialog should close, after any cleanup. */
  onClose: () => void;
  /** Optional magnet URI to prefill, e.g. detected from the clipboard. */
  initialMagnet?: string;
}

/**
 * Two-step add flow: resolve metadata, then choose files, then download.
 *
 * Nothing is downloaded until the user confirms. That is the whole point for
 * the ISO use case — distro torrents routinely bundle several images plus
 * checksums when the user wants one.
 *
 * Resolving a magnet fetches metadata over the DHT and can take seconds, so
 * the resolving state is a real state with its own affordances, not a spinner
 * bolted onto the input.
 *
 * @param props - See {@link AddTorrentDialogProps}.
 * @returns The rendered dialog.
 */
export function AddTorrentDialog({
  onClose,
  initialMagnet = "",
}: AddTorrentDialogProps) {
  const [magnet, setMagnet] = useState(initialMagnet);
  const [preview, setPreview] = useState<TorrentPreview | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [isResolving, setIsResolving] = useState(false);
  const [isAdding, setIsAdding] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const dialogRef = useRef<HTMLDivElement>(null);
  // Kept in a ref so the unmount cleanup does not need `preview` as a dep,
  // which would re-run cleanup on every preview change.
  const pendingHashRef = useRef<string | null>(null);

  const describe = (caught: unknown, fallback: string) =>
    isCommandError(caught) ? caught.message : fallback;

  const resolve = useCallback(
    async (source: Parameters<typeof previewTorrent>[0]) => {
      setError(null);
      setIsResolving(true);
      try {
        const resolved = await previewTorrent(source);
        setPreview(resolved);
        pendingHashRef.current = resolved.infoHash;
        // Everything selected by default: the common case is wanting the whole
        // torrent, and starting from nothing selected makes the primary action
        // dead on arrival.
        setSelected(new Set(resolved.files.map((f) => f.index)));
      } catch (caught: unknown) {
        setError(describe(caught, "Could not read that torrent."));
      } finally {
        setIsResolving(false);
      }
    },
    [],
  );

  const pickFile = async () => {
    const path = await open({
      multiple: false,
      filters: [{ name: "Torrent", extensions: ["torrent"] }],
    });
    if (typeof path !== "string") return;

    // The path goes to Rust, which reads the file. The webview never touches
    // the filesystem.
    await resolve({ kind: "file", path });
  };

  const confirm = async () => {
    if (!preview) return;
    setIsAdding(true);
    setError(null);
    try {
      const all = selected.size === preview.files.length;
      await confirmAdd(preview.infoHash, all ? null : [...selected]);
      pendingHashRef.current = null;
      onClose();
    } catch (caught: unknown) {
      setError(describe(caught, "Could not start that torrent."));
      setIsAdding(false);
    }
  };

  // Release any preview the user abandoned, so its resolved metadata is not
  // retained in the engine for the life of the process.
  useEffect(() => {
    return () => {
      const hash = pendingHashRef.current;
      if (hash) void discardPreview(hash).catch(() => {});
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    dialogRef.current?.focus();
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const selectedBytes = preview
    ? preview.files
        .filter((f) => selected.has(f.index))
        .reduce((sum, f) => sum + f.length, 0)
    : 0;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label="Add a torrent"
        tabIndex={-1}
        className="border-border-subtle bg-surface flex max-h-[80vh] w-full max-w-xl flex-col gap-4 rounded-xl border p-5 shadow-2xl outline-none"
      >
        <h2 className="text-text text-lg font-semibold">Add a torrent</h2>

        {preview === null ? (
          <>
            <div className="flex flex-col gap-2">
              <label
                htmlFor="magnet-input"
                className="text-faint text-[11px] font-medium tracking-wider uppercase"
              >
                Magnet link
              </label>
              <div className="flex gap-2">
                <input
                  id="magnet-input"
                  value={magnet}
                  onChange={(e) => setMagnet(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && magnet.trim() && !isResolving) {
                      void resolve({ kind: "magnet", uri: magnet.trim() });
                    }
                  }}
                  placeholder="magnet:?xt=urn:btih:…"
                  spellCheck={false}
                  autoFocus
                  className="border-border-subtle bg-bg text-text placeholder:text-faint selectable min-w-0 flex-1 rounded-md border px-3 py-2 font-mono text-sm"
                />
                <Button
                  variant="primary"
                  disabled={!magnet.trim() || isResolving}
                  onClick={() =>
                    void resolve({ kind: "magnet", uri: magnet.trim() })
                  }
                >
                  {isResolving ? "Resolving…" : "Resolve"}
                </Button>
              </div>
            </div>

            <div className="flex items-center gap-3">
              <span className="bg-border-subtle h-px flex-1" />
              <span className="text-faint text-xs">or</span>
              <span className="bg-border-subtle h-px flex-1" />
            </div>

            <Button onClick={() => void pickFile()} disabled={isResolving}>
              Choose a .torrent file…
            </Button>

            {isResolving ? (
              <p className="text-muted text-xs" role="status">
                Fetching metadata from the DHT. This can take a few seconds for
                a magnet link.
              </p>
            ) : null}
          </>
        ) : (
          <>
            <div className="min-w-0">
              <p
                className="text-text truncate text-sm font-medium"
                title={preview.name}
              >
                {preview.name}
              </p>
              <p className="text-muted mt-0.5 text-xs">
                {preview.files.length} file
                {preview.files.length === 1 ? "" : "s"} ·{" "}
                <span className="font-mono">
                  {formatBytes(preview.totalBytes)}
                </span>
              </p>
            </div>

            {preview.alreadyAdded ? (
              <p
                className="border-warn/30 bg-warn/10 text-warn rounded-md border px-3 py-2 text-xs"
                role="status"
              >
                This torrent is already in your list. Adding it again will
                update its file selection.
              </p>
            ) : null}

            <FileTree
              files={preview.files}
              selected={selected}
              onChange={setSelected}
            />
          </>
        )}

        {error ? (
          <p
            className="border-error/30 bg-error/10 text-error rounded-md border px-3 py-2 text-xs"
            role="alert"
          >
            {error}
          </p>
        ) : null}

        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          {preview ? (
            <Button
              variant="primary"
              onClick={() => void confirm()}
              disabled={selected.size === 0 || isAdding}
            >
              {isAdding
                ? "Starting…"
                : `Download ${formatBytes(selectedBytes)}`}
            </Button>
          ) : null}
        </div>
      </div>
    </div>
  );
}
