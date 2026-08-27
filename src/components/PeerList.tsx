"use client";

import { formatBytes } from "@/lib/format";
import type { PeerInfo } from "@/lib/ipc/types";

/** Props for {@link PeerList}. */
export interface PeerListProps {
  /** Peers currently connected to the torrent. */
  peers: PeerInfo[];
}

/**
 * Connected peers, one row each.
 *
 * @param props - See {@link PeerListProps}.
 * @returns The rendered list.
 */
export function PeerList({ peers }: PeerListProps) {
  if (peers.length === 0) {
    return (
      <p className="text-fg-3 py-6 text-center text-xs">
        No connected peers. This is normal while a torrent is starting, paused,
        or fully seeded with nobody asking for it.
      </p>
    );
  }

  return (
    <table className="w-full text-left text-xs">
      <thead className="text-fg-3">
        <tr className="border-line border-b">
          <th scope="col" className="py-2 font-medium">
            Address
          </th>
          <th scope="col" className="py-2 font-medium">
            Client
          </th>
          <th scope="col" className="py-2 text-right font-medium">
            Down
          </th>
          <th scope="col" className="py-2 text-right font-medium">
            Up
          </th>
          <th
            scope="col"
            className="py-2 text-right font-medium"
            title="Pieces this peer supplied that passed verification"
          >
            Pieces
          </th>
        </tr>
      </thead>
      <tbody className="text-fg-2">
        {peers.map((peer) => (
          <tr
            key={peer.address}
            className="border-line border-b last:border-b-0"
          >
            <td className="text-fg-0 py-2 font-mono">
              {peer.address}
              {peer.transport ? (
                <span className="text-fg-3 ml-1.5 uppercase">
                  {peer.transport}
                </span>
              ) : null}
            </td>
            <td
              className="max-w-[14rem] truncate py-2"
              title={peer.client ?? undefined}
            >
              {peer.client ?? "—"}
            </td>
            <td className="py-2 text-right font-mono tabular-nums">
              {formatBytes(peer.downloadedBytes)}
            </td>
            <td className="py-2 text-right font-mono tabular-nums">
              {formatBytes(peer.uploadedBytes)}
            </td>
            <td className="py-2 text-right font-mono tabular-nums">
              <span
                className={
                  peer.piecesContributed > 0 ? "text-fg-0" : "text-fg-3"
                }
              >
                {peer.piecesContributed}
              </span>
              {peer.errors > 0 ? (
                <span
                  className="text-err ml-1.5"
                  title={`${peer.errors} connection error${peer.errors === 1 ? "" : "s"}`}
                >
                  !{peer.errors}
                </span>
              ) : null}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
