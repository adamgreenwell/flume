"use client";

import { useCallback, useEffect, useMemo, useState } from "react";

import { useRateHistory } from "@/hooks/useThroughputHistory";
import { formatBytes, formatDuration, formatSpeed } from "@/lib/format";
import { getTorrentFiles, setOnlyFiles } from "@/lib/ipc/client";
import {
  isCommandError,
  type TorrentDetail as TorrentDetailData,
  type TorrentFileState,
  type TorrentSummary,
} from "@/lib/ipc/types";

import { Button } from "./Button";
import { Chip } from "./Chip";
import { FragmentStrip } from "./FragmentStrip";
import { NoteCard } from "./NoteCard";
import { PeerList } from "./PeerList";
import { BottleneckPanel } from "./BottleneckPanel";
import { PieceStrip } from "./PieceStrip";
import { Skeleton } from "./Skeleton";
import { StatCard } from "./StatCard";
import { ThroughputChart } from "./ThroughputChart";
import { TrackerList } from "./TrackerList";

/** The inspector's tabs, in order. */
const TABS = ["overview", "files", "peers", "trackers"] as const;

/** Which tab is showing. */
export type DetailTab = (typeof TABS)[number];

/** Props for {@link TorrentDetail}. */
export interface TorrentDetailProps {
  /** The torrent being inspected. */
  torrent: TorrentSummary;
  /** Its detail, or `null` while the first response is outstanding. */
  detail: TorrentDetailData | null;
  /** Session uptime, used as the per-tick key for the throughput chart. */
  tick: number | null;
  /** Configured download limit, for the chart's ceiling. */
  limitBps: number | null;
  /** Close the inspector. */
  onClose: () => void;
}

/**
 * The torrent inspector.
 *
 ## What is deliberately not here
 *
 * The bottleneck panel ranks only the constraints Flume can measure. Three of
 * the five factors the design names have no data behind them and are left out
 * rather than drawn with a plausible number — connection slots (librqbit's
 * `peer_limit` is unset, so there is no ceiling), disk writes (no write-queue
 * depth) and hash checking (no CPU accounting). See `engine/bottleneck.rs`.
 *
 * Still absent for the same reason: the trackers tab's plain-English verdict,
 * which needs per-tracker announce status librqbit does not expose. That is a
 * real feature waiting on real data, not an oversight.
 *
 * @param props - See {@link TorrentDetailProps}.
 * @returns The rendered inspector.
 */
export function TorrentDetail({
  torrent,
  detail,
  tick,
  limitBps,
  onClose,
}: TorrentDetailProps) {
  const [tab, setTab] = useState<DetailTab>("overview");
  const [files, setFiles] = useState<TorrentFileState[] | null>(null);
  const [selected, setSelected] = useState<ReadonlySet<number>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  const history = useRateHistory(torrent.downloadBps, torrent.uploadBps, tick);

  useEffect(() => {
    let active = true;
    getTorrentFiles(torrent.id)
      .then((next) => {
        if (!active) return;
        setFiles(next);
        setSelected(
          new Set(next.filter((f) => f.selected).map((f) => f.index)),
        );
      })
      .catch((caught: unknown) => {
        if (!active) return;
        setError(
          isCommandError(caught)
            ? caught.message
            : "Could not read this torrent's files.",
        );
      });
    return () => {
      active = false;
    };
  }, [torrent.id]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const saveSelection = useCallback(
    async (next: ReadonlySet<number>) => {
      setSelected(next);
      setIsSaving(true);
      setError(null);
      try {
        await setOnlyFiles(torrent.id, [...next]);
      } catch (caught: unknown) {
        setError(
          isCommandError(caught)
            ? caught.message
            : "Could not change the file selection.",
        );
      } finally {
        setIsSaving(false);
      }
    },
    [torrent.id],
  );

  const counts: Record<DetailTab, number | null> = {
    overview: null,
    files: files?.length ?? null,
    peers: detail?.peers.length ?? null,
    trackers: detail?.trackers.length ?? null,
  };

  const ratio =
    torrent.progressBytes === 0
      ? 0
      : torrent.uploadedBytes / torrent.progressBytes;

  const pieceLabel = useMemo(() => {
    if (!detail?.pieces) return "Pieces";
    const { totalPieces, piecesPerBucket } = detail.pieces;
    return `Pieces · ${totalPieces.toLocaleString()}${
      piecesPerBucket > 1 ? ` · ${piecesPerBucket} per column` : ""
    }`;
  }, [detail]);

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
        className="border-line bg-bg-0 flex h-[900px] max-h-[92vh] w-full max-w-[1440px] flex-col overflow-hidden rounded-lg border shadow-2xl"
      >
        <div className="border-line bg-bg-1 flex min-h-[68px] shrink-0 items-center gap-3.5 border-b px-6 py-2">
          <div className="min-w-0 grow">
            <h2
              className="truncate text-[17px] font-semibold tracking-[-0.02em]"
              title={torrent.name}
            >
              {torrent.name}
            </h2>
            <p className="text-fg-2 mt-0.5 truncate text-[11.5px]">
              {torrent.detail}
            </p>
          </div>
          <Chip onClick={onClose} aria-label="Close details">
            Close
          </Chip>
        </div>

        <div className="border-line bg-bg-1 flex min-h-[88px] shrink-0 border-b">
          <StatCard
            size="strip"
            label="Progress"
            value={`${Math.round((torrent.progressBytes / Math.max(torrent.totalBytes, 1)) * 100)}%`}
            hint={`${formatBytes(torrent.progressBytes)} of ${formatBytes(torrent.totalBytes)}`}
          />
          <StatCard
            size="strip"
            label="Down"
            value={formatSpeed(torrent.downloadBps)}
            hint={
              torrent.etaSeconds === null
                ? "no estimate at this rate"
                : `${formatDuration(torrent.etaSeconds)} left`
            }
          />
          <StatCard
            size="strip"
            label="Up"
            value={formatSpeed(torrent.uploadBps)}
            hint={`${formatBytes(torrent.uploadedBytes)} uploaded`}
          />
          <StatCard
            size="strip"
            label="Peers"
            value={
              torrent.knownPeers === 0
                ? "—"
                : `${torrent.livePeers} / ${torrent.knownPeers}`
            }
            hint="connected of known"
          />
          <StatCard
            size="strip"
            label="Ratio"
            value={ratio.toFixed(2)}
            hint={`${formatBytes(torrent.uploadedBytes)} up, ${formatBytes(torrent.progressBytes)} down`}
          />
          <StatCard
            size="strip"
            label="Pieces"
            value={
              detail?.pieces
                ? `${detail.pieces.piecesComplete.toLocaleString()} / ${detail.pieces.totalPieces.toLocaleString()}`
                : "—"
            }
            hint={detail?.pieces ? "verified of total" : "not running"}
          />
        </div>

        <div
          className="border-line flex h-[42px] shrink-0 items-end gap-1 border-b px-6"
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
              className={`-mb-px border-b-2 px-3 pb-2.5 text-[12.5px] capitalize transition-colors ${
                tab === name
                  ? "border-acc text-fg-0 font-medium"
                  : "text-fg-2 hover:text-fg-0 border-transparent"
              }`}
            >
              {name}
              {counts[name] !== null ? (
                <span className="flume-num text-fg-3 ml-1.5 text-[11px]">
                  {counts[name]}
                </span>
              ) : null}
            </button>
          ))}
        </div>

        {error ? (
          <p
            className="border-line bg-err/10 text-err shrink-0 border-b px-6 py-2 text-[12.5px]"
            role="alert"
          >
            {error}
          </p>
        ) : null}

        <div
          className="flex min-h-0 grow gap-[22px] overflow-y-auto px-6 py-[22px]"
          role="tabpanel"
        >
          {tab === "overview" ? (
            <>
              <div className="flex min-w-0 grow flex-col gap-[22px]">
                <section className="border-line bg-bg-1 rounded-lg border">
                  <ThroughputChart
                    history={history}
                    limitBps={limitBps}
                    label="This torrent"
                  />
                </section>

                <section className="border-line bg-bg-1 flex flex-col gap-3 rounded-lg border p-5">
                  <div className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
                    {pieceLabel}
                  </div>
                  {detail?.pieces ? (
                    <PieceStrip pieces={detail.pieces} />
                  ) : detail === null ? (
                    <Skeleton rows={1} label="Loading pieces" />
                  ) : (
                    <p className="text-fg-2 text-[11.5px]">
                      Piece detail appears once this torrent is running. A
                      torrent that is still starting up or has errored has no
                      piece state to read.
                    </p>
                  )}
                </section>

                {detail ? (
                  <BottleneckPanel bottleneck={detail.bottleneck} />
                ) : null}
              </div>

              <aside className="flex w-[352px] shrink-0 flex-col gap-[22px]">
                {detail ? <NoteCard note={detail.note} /> : null}

                <section className="border-line bg-bg-1 flex flex-col gap-3 rounded-lg border p-5">
                  <div className="text-fg-3 text-[10px] font-semibold tracking-[0.09em] uppercase">
                    Torrent
                  </div>
                  <dl className="flex flex-col gap-2.5 text-[11.5px]">
                    <div className="flex flex-col gap-0.5">
                      <dt className="text-fg-3">Info hash</dt>
                      <dd className="flume-num text-fg-1 selectable break-all">
                        {torrent.infoHash}
                      </dd>
                    </div>
                    <div className="flex flex-col gap-0.5">
                      <dt className="text-fg-3">Saving to</dt>
                      <dd
                        className="text-fg-1 selectable break-all"
                        title={torrent.outputFolder}
                      >
                        {torrent.outputFolder}
                      </dd>
                    </div>
                    <div className="flex flex-col gap-0.5">
                      <dt className="text-fg-3">Total size</dt>
                      <dd className="flume-num text-fg-1">
                        {formatBytes(torrent.totalBytes)}
                      </dd>
                    </div>
                  </dl>
                </section>
              </aside>
            </>
          ) : null}

          {tab === "files" ? (
            <div className="min-w-0 grow">
              {files === null ? (
                <Skeleton rows={4} label="Loading files" />
              ) : (
                <ul className="border-line bg-bg-1 overflow-hidden rounded-lg border">
                  {files.map((file) => {
                    const on = selected.has(file.index);
                    return (
                      <li
                        key={file.index}
                        className="border-line flex items-center gap-3 border-b px-4 py-2.5 last:border-b-0"
                      >
                        <input
                          type="checkbox"
                          checked={on}
                          disabled={isSaving}
                          aria-label={file.path}
                          onChange={() => {
                            const next = new Set(selected);
                            if (on) next.delete(file.index);
                            else next.add(file.index);
                            void saveSelection(next);
                          }}
                          className="accent-acc shrink-0"
                        />
                        <span className="min-w-0 grow">
                          <span
                            className="block truncate text-[12.5px]"
                            title={file.path}
                          >
                            {file.path}
                          </span>
                          <span className="mt-1 block">
                            <FragmentStrip
                              buckets={file.pieceBuckets}
                              label={`Downloaded parts of ${file.path}`}
                            />
                          </span>
                        </span>
                        <span className="flume-num text-fg-2 shrink-0 text-[11.5px]">
                          {formatBytes(file.progressBytes)} /{" "}
                          {formatBytes(file.length)}
                        </span>
                      </li>
                    );
                  })}
                </ul>
              )}
            </div>
          ) : null}

          {tab === "peers" ? (
            <div className="min-w-0 grow">
              {detail === null ? (
                <Skeleton rows={4} label="Loading peers" />
              ) : (
                <PeerList peers={detail.peers} />
              )}
            </div>
          ) : null}

          {tab === "trackers" ? (
            <div className="min-w-0 grow">
              {detail === null ? (
                <Skeleton rows={3} label="Loading trackers" />
              ) : (
                <TrackerList trackers={detail.trackers} />
              )}
            </div>
          ) : null}
        </div>

        <div className="border-line bg-bg-1 flex h-[62px] shrink-0 items-center gap-3 border-t px-6">
          <span className="text-fg-2 text-[11.5px]">
            {isSaving
              ? "Saving the file selection…"
              : "File selection applies immediately."}
          </span>
          <span className="grow" />
          <Button size="dialog" onClick={onClose}>
            Done
          </Button>
        </div>
      </div>
    </div>
  );
}
