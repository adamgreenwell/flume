/**
 * TypeScript mirrors of the Rust types in `src-tauri/src/engine/status.rs`.
 *
 * These definitions are hand-maintained. If you change a `serde` struct on the
 * Rust side, change it here in the same commit — the compiler cannot check
 * across the IPC boundary for us.
 *
 * The Rust structs use `#[serde(rename_all = "camelCase")]`, so field names
 * here are camelCase.
 */

/** Health of the DHT subsystem. */
export interface DhtStatus {
  /** Whether the DHT was enabled in configuration at all. */
  enabled: boolean;
  /** Number of IPv4 nodes in the routing table. Zero while bootstrapping. */
  nodesV4: number;
  /** Number of IPv6 nodes in the routing table. */
  nodesV6: number;
  /** DHT queries currently awaiting a response. */
  outstandingRequests: number;
}

/**
 * Coarse, user-facing readiness of the engine.
 *
 * Mirrors the Rust `EngineHealth` enum, which serialises as a camelCase string.
 */
export type EngineHealth = "starting" | "connecting" | "ready" | "degraded";

/** A point-in-time snapshot of engine state. */
export interface CoreStatus {
  /** The librqbit client string, e.g. `"Flume 0.1.0"`. */
  clientVersion: string;
  /** Port bound for incoming peer connections, or `null` if not listening. */
  listenPort: number | null;
  /** Port announced to trackers and peers, or `null`. */
  announcePort: number | null;
  /** DHT subsystem health. */
  dht: DhtStatus;
  /** Absolute path downloads are written to. */
  downloadDir: string;
  /** Seconds since the session started. */
  uptimeSeconds: number;
  /** Current aggregate download rate in bytes per second. */
  downloadBps: number;
  /** Current aggregate upload rate in bytes per second. */
  uploadBps: number;
  /** Peers currently connected across all torrents. */
  livePeers: number;
  /** Derived readiness indicator. */
  health: EngineHealth;
}

/**
 * User-facing lifecycle state of a torrent. Mirrors Rust `TorrentState`.
 *
 * Deliberately coarser than librqbit's internal states: the engine
 * distinguishes "live" from "finished", but a user thinks in terms of
 * downloading versus seeding.
 */
export type TorrentState =
  "checking" | "downloading" | "seeding" | "paused" | "error";

/**
 * A verdict on whether a torrent will actually finish. Mirrors Rust
 * `SwarmHealth`.
 *
 * A different question from {@link TorrentState}, which only says whether work
 * is happening — a torrent can be `downloading` and never finish.
 *
 * `"unknown"` is not a gap in the plumbing. Telling a thin swarm from a healthy
 * one needs piece availability, which librqbit 9.0.0 does not expose, so
 * anything that would need it says so rather than guessing. See issue #79.
 */
export type SwarmHealth = "seeding" | "none" | "idle" | "unknown";

/** A snapshot of one torrent. Mirrors Rust `TorrentSummary`. */
export interface TorrentSummary {
  /** Session-local id. Stable while the app runs, not across restarts. */
  id: number;
  /** Hex info hash. Stable across restarts, unlike {@link TorrentSummary.id}. */
  infoHash: string;
  /** Display name, or the info hash if metadata has not resolved yet. */
  name: string;
  /** Coarse lifecycle state. */
  state: TorrentState;
  /** Bytes downloaded and verified. */
  progressBytes: number;
  /** Total size of the selected files, in bytes. */
  totalBytes: number;
  /** Bytes uploaded to peers this session. */
  uploadedBytes: number;
  /** Download rate in bytes per second; zero when not live. */
  downloadBps: number;
  /** Upload rate in bytes per second; zero when not live. */
  uploadBps: number;
  /** Peers currently connected to this torrent. */
  livePeers: number;
  /**
   * Peers this torrent has ever seen, connected or not.
   *
   * The denominator in the list's "24 / 118". A large gap between this and
   * {@link TorrentSummary.livePeers} separates "the swarm is small" from "the
   * swarm is big but nobody will talk to us" — different problems, different
   * fixes.
   */
  knownPeers: number;
  /** Whether this torrent will actually finish. */
  health: SwarmHealth;
  /**
   * The one-line explanation shown under the name. Derived in Rust.
   *
   * Never a bare state word: the row already draws the state as an icon, so
   * this line is the only one that can say why.
   */
  detail: string;
  /** Estimated seconds to completion, or `null` when not estimable. */
  etaSeconds: number | null;
  /** Whether all selected files are complete. */
  finished: boolean;
  /** Failure message when {@link TorrentSummary.state} is `"error"`. */
  error: string | null;
  /** Absolute directory the files are written to. */
  outputFolder: string;
}

/** Everything the UI needs for one render tick. Mirrors Rust `TelemetrySnapshot`. */
export interface TelemetrySnapshot {
  /** Session-wide status. */
  core: CoreStatus;
  /** One entry per torrent, ordered by id. */
  torrents: TorrentSummary[];
}

/**
 * Where a torrent is being added from. Mirrors Rust `TorrentSource`.
 *
 * The `file` variant carries a *path*, not bytes: the engine does the reading,
 * so the frontend needs no filesystem permission and no file contents cross
 * the IPC boundary.
 */
export type TorrentSource =
  { kind: "magnet"; uri: string } | { kind: "file"; path: string };

/** One file inside a torrent. Mirrors Rust `TorrentFile`. */
export interface TorrentFile {
  /** Index within the torrent; this is what selection refers to. */
  index: number;
  /** Path relative to the torrent root, using forward slashes. */
  path: string;
  /** Size in bytes. */
  length: number;
}

/**
 * Resolved metadata for a torrent that has not started. Mirrors Rust
 * `TorrentPreview`.
 *
 * Deliberately carries no `.torrent` bytes: the engine holds those and looks
 * them up by `infoHash` on confirm, so a magnet's metadata is fetched from the
 * DHT exactly once.
 */
export interface TorrentPreview {
  /** Hex info hash; also the key used to confirm the add. */
  infoHash: string;
  /** Display name from the metadata. */
  name: string;
  /** Combined size of every file. */
  totalBytes: number;
  /** Every file, in torrent order. */
  files: TorrentFile[];
  /** Whether this torrent is already in the session. */
  alreadyAdded: boolean;
  /** Where these files would be written if added now. */
  savePath: string;
  /**
   * Free space on that volume right now, or `null` if it cannot be read.
   *
   * `null` rather than zero — the sheet renders it as "unknown", and zero free
   * bytes is a specific and alarming claim to make by accident.
   */
  freeBytes: number | null;
  /**
   * Peers that answered while the metadata was being fetched.
   *
   * Not a tracker scrape: there is no seeds/leechers split here, only the
   * peers librqbit actually heard from. A real measurement rather than an
   * estimate, which is why it is worth showing.
   */
  seenPeers: number;
  /**
   * Per file, whether a file of that name and length is already there.
   *
   * Parallel to {@link TorrentPreview.files}. Length is checked, not content —
   * hashing 46 GB to answer a question asked before the download starts would
   * take longer than the download, and every piece is verified on arrival
   * anyway.
   */
  alreadyOnDisk: boolean[];
}

/**
 * One file inside an *added* torrent, with progress. Mirrors Rust
 * `TorrentFileState`.
 *
 * Distinct from {@link TorrentFile}, which describes a torrent that has not
 * been added and so has no progress or selection.
 */
export interface TorrentFileState {
  /** Index within the torrent; what selection refers to. */
  index: number;
  /** Path relative to the torrent root. */
  path: string;
  /** Total size in bytes. */
  length: number;
  /** Bytes downloaded and verified for this file. */
  progressBytes: number;
  /** Whether this file is currently selected for download. */
  selected: boolean;
  /** First piece index covering this file. */
  firstPiece: number;
  /** Piece index just past the last one covering this file. */
  lastPiece: number;
  /**
   * Downsampled completion across this file's own piece range, `0..=255`.
   *
   * Empty when piece state is unavailable. This is what answers "which parts
   * of this file do I have" — overall progress says 60%, this says *which* 60%.
   */
  pieceBuckets: number[];
}

/**
 * Health of the peer pool for one torrent. Mirrors Rust `SwarmStats`.
 *
 * These are *pool* counts, not seeds versus leechers. librqbit v9 knows
 * whether a peer holds the whole torrent, but does not expose it through the
 * public per-peer snapshot, so that split is unavailable.
 */
export interface SwarmStats {
  /** Peers with an established connection right now. */
  live: number;
  /** Peers currently being connected to. */
  connecting: number;
  /** Known peers waiting for a connection slot. */
  queued: number;
  /** Distinct peers discovered for this torrent, ever. */
  seen: number;
  /** Peers that failed and were dropped. */
  dead: number;
  /** Live peers connected over TCP. */
  liveTcp: number;
  /** Live peers connected over uTP. */
  liveUtp: number;
}

/** One connected peer. Mirrors Rust `PeerInfo`. */
export interface PeerInfo {
  /** Remote socket address, as `host:port`. */
  address: string;
  /** Client software the peer reports, if it identified itself. */
  client: string | null;
  /** Transport in use: `tcp`, `utp`, or `socks`. */
  transport: string | null;
  /** The engine's state label for this peer. */
  state: string;
  /** Bytes downloaded from this peer. */
  downloadedBytes: number;
  /** Bytes uploaded to this peer. */
  uploadedBytes: number;
  /**
   * Pieces this peer supplied that passed verification.
   *
   * The most honest measure of whether a peer is helping — bytes can arrive
   * and then fail their hash check.
   */
  piecesContributed: number;
  /** Errors encountered on this connection. */
  errors: number;
}

/**
 * A downsampled view of which pieces are present. Mirrors Rust `PieceMap`.
 *
 * Each bucket summarises a run of pieces as a level from 0 (none) to 255
 * (all), so the payload stays small and fixed-size no matter how many pieces
 * the torrent has.
 */
export interface PieceMap {
  /** Total pieces in the torrent. */
  totalPieces: number;
  /**
   * How many are downloaded and verified.
   *
   * Counted from the bitfield, not inferred from the buckets — a bucket holds
   * an averaged level, so summing them back would give an estimate where an
   * exact number is free.
   */
  piecesComplete: number;
  /** How many pieces each bucket represents. */
  piecesPerBucket: number;
  /** Completion level per bucket, `0..=255`. */
  buckets: number[];
}

/** Detail-view data beyond the file list. Mirrors Rust `TorrentDetail`. */
/**
 * How much attention a {@link Note} wants. Mirrors Rust `NoteSeverity`.
 *
 * `"neutral"` is not an absence of severity. It is the deliberate statement
 * that nothing is wrong, which a paused torrent needs to make loudly enough
 * that the user does not think something broke.
 */
export type NoteSeverity = "ok" | "warn" | "err" | "neutral";

/**
 * What a torrent is actually doing, in words. Mirrors Rust `Note`.
 *
 * The expanded row's reason for existing, and the design's third principle in
 * one object: never a bare adjective. "Stalled" is not a status; "12 peers are
 * known and none of them is answering" is.
 */
export interface Note {
  /** How much attention this wants. */
  severity: NoteSeverity;
  /** The headline. A claim about this torrent, never a state word. */
  title: string;
  /** Two or three sentences: what is happening, and what to do about it. */
  body: string;
}

export interface TorrentDetail {
  /** Connected peers; empty when the torrent is not live. */
  peers: PeerInfo[];
  /**
   * Tracker announce URLs.
   *
   * URLs only — librqbit v9 exposes the configured tracker list but not
   * per-tracker announce status, so there is no last-announce or peer count
   * to show.
   */
  trackers: string[];
  /** Piece completion, or `null` when the torrent is not live or paused. */
  pieces: PieceMap | null;
  /** Peer pool health. */
  swarm: SwarmStats;
  /**
   * What this torrent is actually doing, in words.
   *
   * Carried here rather than on the 1 Hz summary because only an expanded row
   * shows it, and a three-sentence string per torrent per second is a lot of
   * IPC for something nobody is reading.
   */
  note: Note;
}

/** UI colour scheme preference. Mirrors Rust `Theme`. */
export type Theme = "system" | "light" | "dark";

/** Everything the user can configure. Mirrors Rust `Settings`. */
export interface Settings {
  /** Where downloads are written. Changing this restarts the session. */
  downloadDir: string;
  /** TCP port for incoming peers. Changing this restarts the session. */
  listenPort: number;
  /** Whether the DHT runs. Required for magnet links. Restarts the session. */
  enableDht: boolean;
  /** Whether to request a UPnP port mapping. Restarts the session. */
  enableUpnp: boolean;
  /** Global download limit in bytes/sec; `null` is unlimited. Applies live. */
  downloadLimitBps: number | null;
  /** Global upload limit in bytes/sec; `null` is unlimited. Applies live. */
  uploadLimitBps: number | null;
  /**
   * SOCKS5 proxy for outgoing peer connections; `null` connects directly.
   *
   * Format: `socks5://[user:password@]host:port`. Requires a session restart.
   */
  proxyUrl: string | null;
  /** UI colour scheme. */
  theme: Theme;
}

/** An error returned by a Tauri command. Mirrors Rust `CommandError`. */
export interface CommandError {
  /** Stable identifier for the error class, e.g. `"engineNotReady"`. */
  kind: string;
  /** Human-readable description, safe to show in the UI. */
  message: string;
}

/**
 * Narrows an unknown thrown value to a {@link CommandError}.
 *
 * Tauri rejects `invoke` with whatever the Rust side serialised, so the value
 * arrives as `unknown` and must be checked before use.
 *
 * @param value - The value caught from a rejected `invoke` call.
 * @returns `true` if `value` has the shape of a `CommandError`.
 */
export function isCommandError(value: unknown): value is CommandError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    "message" in value &&
    typeof (value as CommandError).kind === "string" &&
    typeof (value as CommandError).message === "string"
  );
}
