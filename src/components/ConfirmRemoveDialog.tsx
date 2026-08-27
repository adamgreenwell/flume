"use client";

import { useEffect, useState } from "react";

import type { TorrentSummary } from "@/lib/ipc/types";

import { Button } from "./Button";

/** Props for {@link ConfirmRemoveDialog}. */
export interface ConfirmRemoveDialogProps {
  /** The torrent being removed. */
  torrent: TorrentSummary;
  /** Called with the user's choice about deleting files on disk. */
  onConfirm: (deleteFiles: boolean) => void;
  /** Called when the user backs out. */
  onCancel: () => void;
}

/**
 * Confirms removal, and separately whether to delete downloaded files.
 *
 * "Delete files" starts unchecked and is styled as the destructive path.
 * Deleting a mostly-downloaded ISO by accident is a genuinely bad afternoon,
 * so this is never a single click and never the default.
 *
 * @param props - See {@link ConfirmRemoveDialogProps}.
 * @returns The rendered dialog.
 */
export function ConfirmRemoveDialog({
  torrent,
  onConfirm,
  onCancel,
}: ConfirmRemoveDialogProps) {
  const [deleteFiles, setDeleteFiles] = useState(false);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onCancel]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6"
      onClick={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
    >
      <div
        role="alertdialog"
        aria-modal="true"
        aria-label={`Remove ${torrent.name}`}
        className="border-line bg-bg-1 flex w-full max-w-md flex-col gap-4 rounded-xl border p-5 shadow-2xl"
      >
        <div>
          <h2 className="text-fg-0 text-base font-semibold">Remove torrent?</h2>
          <p className="text-fg-2 mt-1 truncate text-sm" title={torrent.name}>
            {torrent.name}
          </p>
        </div>

        <label className="border-line hover:border-err/40 flex cursor-pointer items-start gap-3 rounded-md border p-3">
          <input
            type="checkbox"
            checked={deleteFiles}
            onChange={(e) => setDeleteFiles(e.target.checked)}
            className="accent-err mt-0.5 h-4 w-4 shrink-0"
          />
          <span className="text-sm">
            <span className="text-fg-0 block">
              Also delete downloaded files
            </span>
            <span className="text-fg-3 mt-0.5 block text-xs">
              This permanently deletes the data from disk. It cannot be undone.
            </span>
          </span>
        </label>

        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onCancel} autoFocus>
            Cancel
          </Button>
          <Button
            variant={deleteFiles ? "danger" : "secondary"}
            onClick={() => onConfirm(deleteFiles)}
          >
            {deleteFiles ? "Remove and delete files" : "Remove"}
          </Button>
        </div>
      </div>
    </div>
  );
}
