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
 * `"unknown"` is not a gap in the plumbing. It means there were no peer
 * bitfields to judge from — no live peers yet, or metadata that has not
 * resolved far enough to know the piece count — so the verdict is withheld
 * rather than guessed.
 *
 * Telling `"thin"` from `"healthy"` needs piece availability, which upstream
 * librqbit does not expose; Flume carries a patched build for it. See issue
 * #79 and `ikatson/rqbit#643`.
 */
export type SwarmHealth =
  "seeding" | "none" | "idle" | "unknown" | "healthy" | "thin";

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
  /**
   * Connected peers holding every piece.
   *
   * `null` when there were no bitfields to judge from — not the same claim as
   * zero seeds, and must not be rendered as it.
   */
  seeds: number | null;
  /**
   * Mean copies of each piece across the connected peers.
   *
   * The figure other clients label "availability". A sense of depth only: it
   * cannot stand in for {@link SwarmStats.rarest}, since a swarm averaging
   * four copies can still be missing a piece outright.
   */
  availability: number | null;
  /**
   * Copies of the least-held piece.
   *
   * Zero means the torrent cannot finish from this swarm however deep the
   * average is. This is the number the health verdict is built on.
   */
  rarest: number | null;
}

/**
 * One candidate constraint on a download. Mirrors Rust `LimitFactor`.
 *
 * The design names five factors; Flume populates the three it can measure.
 * Connection slots, disk writes and hash checking are absent because librqbit
 * exposes no ceiling, no write-queue depth and no CPU accounting respectively —
 * omitted rather than filled with a plausible number.
 */
export interface LimitFactor {
  /** Display name, in the user's terms rather than the protocol's. */
  name: string;
  /**
   * How constrained the torrent is by this factor, 0–100.
   *
   * `null` where the ceiling is not measurable — the row renders without a bar
   * rather than with an invented one. A full bar always means "at its limit".
   */
  utilisation: number | null;
  /** Preformatted for display: `"6.6 MB/s"`, `"rarest piece on 4 peers"`. */
  value: string;
  /** Whether this is *the* constraint. At most one factor is ever `true`. */
  binding: boolean;
}

/** What is limiting a download. Mirrors Rust `Bottleneck`. */
export interface Bottleneck {
  /** Every measurable factor, most-constrained first. */
  factors: LimitFactor[];
  /** Two sentences: what is binding, and whether a setting would help. */
  explanation: string;
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
  /**
   * Copies of the *least-held* piece in each bucket, same bucketing as
   * {@link PieceMap.buckets} so the two strips line up column for column.
   *
   * The minimum rather than the mean: this strip exists to show where a
   * torrent is about to stall, and a region averaging eight copies while
   * containing one piece nobody holds is exactly what a mean would hide.
   *
   * `null` when there were no peer bitfields to judge from — not the same as a
   * region nobody holds, and not rendered as one.
   */
  availability: number[] | null;
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
   * What is limiting this download, or `null` when the question does not apply
   * — a paused or seeding torrent is not being limited.
   */
  bottleneck: Bottleneck | null;
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

/**
 * How tall a torrent row is drawn. Mirrors Rust `Density`.
 *
 * `compact` does not shrink the row's second line, it removes it — at 40px
 * there is no room, and a squeezed sentence is the first thing to become
 * unreadable.
 */
export type Density = "comfortable" | "compact";

/**
 * How wide the sidebar is drawn. Mirrors Rust `RailState`.
 *
 * Two states and no third. `collapsed` is an icon rail, never zero width: the
 * rail's network footer carries the egress guard's held state, and a rail that
 * can hide it turns a deliberate hold into unexplained silence.
 */
export type RailState = "expanded" | "collapsed";

/** A BitTorrent client Flume can import from. Mirrors Rust `ClientKind`. */
export type ClientKind = "transmission" | "qBittorrent" | "deluge";

/**
 * Another client found on this machine. Mirrors Rust `DetectedClient`.
 *
 * Only the torrent store and download directory are read. Categories and
 * seeding rules are deliberately not: Flume has no model for either, so there
 * is nowhere to put them and offering to bring them across would be a lie.
 */
export interface DetectedClient {
  /** Which client this is. */
  kind: ClientKind;
  /** Its name, ready to show. */
  name: string;
  /** How many `.torrent` files are in its store. */
  torrentCount: number;
  /**
   * Where it saves downloads, or `null` if its config could not be read.
   *
   * `null` means unreadable, not absent — the UI says so rather than implying
   * the client has no download folder.
   */
  downloadDir: string | null;
  /** The directory its `.torrent` files live in. */
  torrentsDir: string;
}

/**
 * What an import actually did. Mirrors Rust `ImportOutcome`.
 *
 * Three numbers rather than a success flag: all three happen in a normal run
 * and they mean different things. `skipped` is a torrent Flume already had;
 * `failed` is a file it could not read.
 */
export interface ImportOutcome {
  /** Torrents taken over. Their files are being verified in place. */
  added: number;
  /** Torrents Flume already had. */
  skipped: number;
  /** Files that could not be read or parsed. */
  failed: number;
}

/**
 * What Flume does when traffic is not leaving through a tunnel. Mirrors Rust
 * `EgressGuard`.
 *
 * Three states rather than a boolean, and the middle one is the point: wanting
 * to know is not the same as wanting transfer stopped.
 */
export type EgressGuard = "off" | "warn" | "hold";

/** What kind of interface something is. Mirrors Rust `InterfaceKind`. */
export type InterfaceKind = "tunnel" | "ordinary" | "unknown";

/** The interface one address family leaves by. Mirrors Rust `Hop`. */
export interface Hop {
  /**
   * The interface name as the operating system gives it — `utun6`, `wg0`, or
   * on Windows the adapter's friendly name.
   */
  interface: string;
  /** What kind of interface that is. */
  kind: InterfaceKind;
}

/**
 * Where each address family would leave from. Mirrors Rust `EgressPath`.
 *
 * Either half may be `null`: a network with no working IPv6 has no IPv6 route,
 * which is ordinary and not a fault.
 */
export interface EgressPath {
  /** Where IPv4 leaves from. */
  v4: Hop | null;
  /** Where IPv6 leaves from. */
  v6: Hop | null;
}

/**
 * Whether transfer is allowed, and why not if not. Mirrors Rust `Verdict`.
 *
 * `tunnelled` and `pinned` permit transfer; nothing else does — a guard the
 * user switched on to hold traffic has to fail closed, or it is decoration.
 *
 * The two are kept apart so the UI never overclaims. `tunnelled` is Flume's
 * own finding. `pinned` means traffic leaves by the interface the user named
 * and Flume could not confirm it is a tunnel — true, and weaker, and it must
 * read that way on screen.
 */
export type Verdict =
  | {
      verdict: "tunnelled";
      /** The interface traffic leaves by. */
      interface: string;
      /**
       * Whether the *other* address family leaves outside that tunnel.
       *
       * Reported rather than enforced against: a v4-only tunnel beside a
       * working IPv6 default route is still doing what the user asked for over
       * v4, and they are entitled to be told rather than blocked.
       */
      otherFamilyOutside: boolean;
    }
  | {
      verdict: "pinned";
      /** The interface traffic leaves by, which the user named in settings. */
      interface: string;
      /** Whether the other address family leaves outside that interface. */
      otherFamilyOutside: boolean;
    }
  | { verdict: "direct"; interface: string }
  | {
      verdict: "wrongTunnel";
      /** The interface it actually leaves by. */
      interface: string;
      /** The interface pinned in settings. */
      expected: string;
    }
  | { verdict: "unknown" };

/**
 * The current egress path and what Flume makes of it. Mirrors Rust
 * `EgressReport`.
 *
 * Both halves travel together because the verdict is derived from the path
 * *and* the user's pin, and deriving it here would put the decision in two
 * places. The path comes along so the UI can name the interface without
 * re-deriving anything.
 */
export interface EgressReport {
  /** Where each address family leaves from. */
  path: EgressPath;
  /** Whether that permits transfer, and why not if not. */
  verdict: Verdict;
}

/**
 * Everything the UI needs to explain the guard. Mirrors Rust `GuardStatus`.
 *
 * Published once per second by the guard loop, which is the only thing that
 * probes — a second prober would read the routing table at a different instant
 * and disagree, and a guard that contradicts itself on screen is worse than no
 * guard.
 */
export interface GuardStatus {
  /** The mode the user chose. */
  guard: EgressGuard;
  /** Where traffic leaves, and what Flume makes of it. */
  report: EgressReport;
  /**
   * Whether transfer is being held right now.
   *
   * Always `false` unless {@link GuardStatus.guard} is `"hold"` — `"warn"`
   * says so and stops nothing.
   */
  held: boolean;
  /**
   * Seconds until transfer resumes, while a settle window is running.
   *
   * `null` when transfer is running, and when it is held with no prospect of
   * resuming. The difference between "held, and counting down" and "held, and
   * waiting for you" is the difference between a status and an unexplained
   * pause.
   */
  resumesInSeconds: number | null;
}

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
  /**
   * Row height in the library.
   *
   * Frontend-only, like {@link Settings.theme}, and persisted for the same
   * reason: a preference the user re-sets on every launch is not a preference.
   */
  density: Density;
  /** Sidebar width. Frontend-only, persisted for the same reason as theme. */
  rail: RailState;
  /**
   * Whether to require that traffic leaves through a tunnel, and what to do
   * when it does not.
   *
   * Defaults to `off`. Not because the check is expensive — it sends nothing —
   * but because a general-purpose client that greets every new user with a
   * warning about their VPN has made an assumption about what they are
   * downloading.
   */
  egressGuard: EgressGuard;
  /**
   * The one interface the user will accept traffic leaving by, or `null` to
   * accept any tunnel.
   *
   * Pinning is stricter and more brittle: macOS hands out `utun` numbers
   * dynamically, so a VPN that reconnects can land elsewhere and trip the
   * guard.
   */
  egressInterface: string | null;
  /**
   * Whether anonymous usage counts may be sent.
   *
   * Three states, not two. `null` means *not yet asked*, which is what the
   * first-run consent step keys off; a decline is `false` and must not be
   * re-asked. Only `true` sends anything.
   */
  usageReporting: boolean | null;
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
