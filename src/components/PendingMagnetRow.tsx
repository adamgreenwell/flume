"use client";

import type { PendingMagnetStatus } from "@/lib/ipc/types";
import { describePendingMagnet, pendingMagnetName } from "@/lib/pendingMagnet";

import { Button } from "./Button";
import { Icon } from "./Icon";
import { IconButton } from "./IconButton";

/** Props for {@link PendingMagnetRow}. */
export interface PendingMagnetRowProps {
  /** The magnet to show. */
  magnet: PendingMagnetStatus;
  /** Opens file selection for a magnet whose file list has arrived. */
  onChooseFiles: (magnet: PendingMagnetStatus) => void;
  /** Removes the magnet from the list. */
  onRemove: (magnet: PendingMagnetStatus) => void;
}

/**
 * A magnet added before its file list arrived (#148).
 *
 * Laid out on the torrent row's grid so the columns line up, but honest
 * about what it is not: there is no size, no progress and no rate to show,
 * because there is no torrent yet -- so those cells say nothing rather than
 * a zero that would read as a stalled download. The status is a glyph, a
 * word and a sentence, never colour alone.
 *
 * @param props - See {@link PendingMagnetRowProps}.
 * @returns The rendered row.
 */
export function PendingMagnetRow({
  magnet,
  onChooseFiles,
  onRemove,
}: PendingMagnetRowProps) {
  const name = pendingMagnetName(magnet);
  const { word, sentence } = describePendingMagnet(magnet);
  const ready = magnet.state === "ready";

  return (
    <div
      role="row"
      onDoubleClick={() => {
        if (ready) onChooseFiles(magnet);
      }}
      className="border-line hover:bg-bg-1 flex h-[var(--flume-row-h)] shrink-0 items-center gap-4 border-b px-[18px] transition-colors"
    >
      <span
        role="gridcell"
        className={`relative flex h-[18px] w-[18px] shrink-0 items-center justify-center ${ready ? "text-acc" : "text-fg-3"}`}
      >
        <span
          className="absolute inset-0 rounded-full border border-dashed border-current opacity-[0.5]"
          aria-hidden="true"
        />
        <Icon name={ready ? "files" : "search"} size={11} />
        <span className="sr-only">{word}</span>
      </span>

      <span role="gridcell" className="flex min-w-0 grow flex-col gap-0.5">
        <span
          className="truncate text-[13px] font-medium tracking-[-0.005em]"
          title={name}
        >
          {name}
        </span>
        <span className="text-fg-2 flex h-[var(--flume-meta-h)] items-center gap-[7px] overflow-hidden text-[11px] opacity-[var(--flume-meta-op)]">
          <span className="shrink-0">Magnet</span>
          <span className="text-fg-3 shrink-0">·</span>
          <span className="truncate" title={sentence}>
            {sentence}
          </span>
        </span>
      </span>

      <span
        role="gridcell"
        className="text-fg-2 flex w-[var(--flume-col-progress)] shrink-0 items-center gap-2 overflow-hidden text-[11.5px]"
      >
        {ready ? (
          <Button onClick={() => onChooseFiles(magnet)}>Choose files…</Button>
        ) : (
          <span aria-hidden="true">No file list yet</span>
        )}
      </span>

      {/* Download, upload and peers: nothing to measure without a torrent. */}
      <span
        role="gridcell"
        className="text-fg-3 w-[var(--flume-col-down)] shrink-0 text-right text-[11.5px]"
        aria-hidden="true"
      >
        —
      </span>
      <span
        role="gridcell"
        className="text-fg-3 w-[var(--flume-col-up)] shrink-0 text-right text-[11.5px]"
        aria-hidden="true"
      >
        —
      </span>
      <span
        role="gridcell"
        className="text-fg-3 w-[var(--flume-col-peers)] shrink-0 text-right text-[11.5px]"
        aria-hidden="true"
      >
        —
      </span>

      <span
        role="gridcell"
        className="flex w-[var(--flume-col-health)] shrink-0 items-center justify-between gap-1 pl-[14px]"
      >
        <span
          className={`truncate text-[11.5px] ${ready ? "text-acc" : "text-fg-2"}`}
        >
          {word}
        </span>
        <IconButton
          icon="trash"
          label={`Remove ${name}`}
          destructive
          onClick={() => onRemove(magnet)}
        />
      </span>
    </div>
  );
}
