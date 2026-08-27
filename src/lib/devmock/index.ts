/**
 * A fake Tauri IPC layer for working on the UI in a plain browser.
 *
 * # Why this exists
 *
 * Flume's UI only receives data through Tauri `invoke` and `listen`, so in a
 * browser every screen is empty and every dialog shows an error. That makes
 * iterating on layout, theming, and empty states needlessly slow: each change
 * means rebuilding the Rust binary and clicking through a real torrent.
 *
 * With this harness, `npm run dev:mock` opens the full interface populated
 * with representative data.
 *
 * # It cannot reach production
 *
 * Activation requires `NEXT_PUBLIC_FLUME_MOCK=1` at build time. Next inlines
 * that constant, so in a normal build the `install()` call is unreachable and
 * the whole module is dropped by tree-shaking.
 */

import type {
  CoreStatus,
  Settings,
  TorrentDetail,
  TorrentFileState,
  TorrentPreview,
  TorrentSummary,
} from "@/lib/ipc/types";

/** Session status resembling a healthy, busy client. */
const CORE: CoreStatus = {
  clientVersion: "Flume 0.1.0",
  listenPort: 42221,
  announcePort: 42221,
  dht: { enabled: true, nodesV4: 214, nodesV6: 88, outstandingRequests: 3 },
  downloadDir: "/Users/you/Downloads",
  uptimeSeconds: 1847,
  downloadBps: 5_400_000,
  uploadBps: 820_000,
  livePeers: 34,
  health: "ready",
};

/**
 * The design's own fixture set, translated into the IPC shape.
 *
 * Reused deliberately rather than invented: it was built to exercise the
 * states that are easy to forget and ugly to render — a torrent nobody will
 * seed, one that ran out of disk mid-piece, one re-hashing after a power cut,
 * one queued behind a full slot list. A mock containing three healthy
 * downloads makes every layout look finished.
 *
 * Content is all freely distributable (Debian, Arch, Fedora, Ubuntu, Blender
 * open movies, NASA, OpenStreetMap); keep it that way, including in
 * screenshots.
 *
 * `detail` and `health` are normally derived in Rust. The strings here are
 * what that derivation produces for the same inputs.
 */
const TORRENTS: TorrentSummary[] = [
  {
    id: 1,
    infoHash: "d160b8d8ea35a5b4e52837468fc8f03d55cef1f7",
    name: "debian-13.2.0-amd64-DVD-1.iso",
    state: "downloading",
    progressBytes: 19_700_000_000,
    totalBytes: 46_100_000_000,
    uploadedBytes: 394_000_000,
    downloadBps: 6_600_000,
    uploadBps: 900_000,
    livePeers: 41,
    knownPeers: 206,
    health: "unknown",
    detail: "1 h 07 min left",
    etaSeconds: 4020,
    finished: false,
    error: null,
    outputFolder: "/Volumes/Media/Linux",
  },
  {
    id: 2,
    infoHash: "0a1b2c3d4e5f60718293a4b5c6d7e8f901234567",
    name: "OpenStreetMap Planet Dump 2026-08-17",
    state: "downloading",
    progressBytes: 5_090_000_000,
    totalBytes: 84_900_000_000,
    uploadedBytes: 0,
    downloadBps: 0,
    uploadBps: 0,
    livePeers: 0,
    knownPeers: 3,
    health: "none",
    detail: "none of the 3 known peers are answering",
    etaSeconds: null,
    finished: false,
    error: null,
    outputFolder: "/Volumes/Media/Datasets",
  },
  {
    id: 3,
    infoHash: "aa60b8d8ea35a5b4e52837468fc8f03d55ce0000",
    name: "archlinux-2026.08.01-x86_64.iso",
    state: "seeding",
    progressBytes: 1_190_000_000,
    totalBytes: 1_190_000_000,
    uploadedBytes: 5_735_800_000,
    downloadBps: 0,
    uploadBps: 600_000,
    livePeers: 9,
    knownPeers: 61,
    health: "seeding",
    detail: "seeding to 9 of 61 peers · ratio 4.82",
    etaSeconds: null,
    finished: true,
    error: null,
    outputFolder: "/Volumes/Media/Linux",
  },
  {
    id: 4,
    infoHash: "1122334455667788990011223344556677889900",
    name: "NASA Apollo 17 Mission Photography Archive",
    state: "downloading",
    progressBytes: 27_700_000_000,
    totalBytes: 231_000_000_000,
    uploadedBytes: 2_310_000_000,
    downloadBps: 1_100_000,
    uploadBps: 0,
    livePeers: 6,
    knownPeers: 11,
    health: "unknown",
    detail: "51 h 24 min left",
    etaSeconds: 185_045,
    finished: false,
    error: null,
    outputFolder: "/Volumes/Archive/NASA",
  },
  {
    id: 5,
    infoHash: "bb70b8d8ea35a5b4e52837468fc8f03d55ce1111",
    name: "Ubuntu 26.04 LTS Desktop amd64",
    state: "paused",
    progressBytes: 4_870_000_000,
    totalBytes: 5_940_000_000,
    uploadedBytes: 1_509_700_000,
    downloadBps: 0,
    uploadBps: 0,
    livePeers: 0,
    knownPeers: 0,
    health: "idle",
    detail: "paused — everything downloaded is verified on disk",
    etaSeconds: null,
    finished: false,
    error: null,
    outputFolder: "/Volumes/Media/Linux",
  },
  {
    id: 6,
    infoHash: "cc80b8d8ea35a5b4e52837468fc8f03d55ce2222",
    name: "MusicBrainz Database Dump 2026-08-20",
    state: "downloading",
    progressBytes: 7_330_000_000,
    totalBytes: 7_800_000_000,
    uploadedBytes: 3_078_600_000,
    downloadBps: 3_100_000,
    uploadBps: 400_000,
    livePeers: 12,
    knownPeers: 44,
    health: "unknown",
    detail: "2 min 30 s left",
    etaSeconds: 150,
    finished: false,
    error: null,
    outputFolder: "/Volumes/Media/Datasets",
  },
  {
    id: 7,
    infoHash: "dd90b8d8ea35a5b4e52837468fc8f03d55ce3333",
    name: "Wikipedia English Dump 2026-08-01 (multistream)",
    state: "checking",
    progressBytes: 9_270_000_000,
    totalBytes: 22_600_000_000,
    uploadedBytes: 741_600_000,
    downloadBps: 0,
    uploadBps: 0,
    livePeers: 0,
    knownPeers: 0,
    health: "idle",
    detail: "re-checking data already on disk",
    etaSeconds: null,
    finished: false,
    error: null,
    outputFolder: "/Volumes/Archive/Wikipedia",
  },
  {
    id: 8,
    infoHash: "ee10b8d8ea35a5b4e52837468fc8f03d55ce4444",
    name: "Big Buck Bunny 60fps 4K — Blender Foundation",
    state: "error",
    progressBytes: 4_100_000_000,
    totalBytes: 9_310_000_000,
    uploadedBytes: 205_000_000,
    downloadBps: 0,
    uploadBps: 0,
    livePeers: 0,
    knownPeers: 0,
    health: "idle",
    detail: "stopped — /Volumes/Scratch has 0 B free",
    etaSeconds: null,
    finished: false,
    error: "/Volumes/Scratch has 0 B free",
    outputFolder: "/Volumes/Scratch/Film",
  },
];

/**
 * A ragged fragment pattern: a solid head, a working edge, then nothing.
 * Exercises all three visual regions of a per-file strip.
 */
function fragments(count: number, completeUpTo: number): number[] {
  return Array.from({ length: count }, (_, i) => {
    if (i < completeUpTo) return 255;
    if (i < completeUpTo + 6) return (i * 53) % 255;
    return 0;
  });
}

const FILES: TorrentFileState[] = [
  {
    index: 0,
    path: "ubuntu-24.04.3-desktop-amd64.iso",
    length: 6_100_000_000,
    progressBytes: 3_100_000_000,
    selected: true,
    firstPiece: 0,
    lastPiece: 23_270,
    pieceBuckets: fragments(120, 60),
  },
  {
    index: 1,
    path: "SHA256SUMS",
    length: 2048,
    progressBytes: 2048,
    selected: true,
    firstPiece: 23_270,
    lastPiece: 23_271,
    pieceBuckets: [255],
  },
  {
    index: 2,
    path: "SHA256SUMS.gpg",
    length: 833,
    progressBytes: 0,
    selected: false,
    firstPiece: 23_271,
    lastPiece: 23_272,
    pieceBuckets: [0],
  },
];

const SETTINGS: Settings = {
  downloadDir: "/Users/you/Downloads",
  listenPort: 42221,
  enableDht: true,
  enableUpnp: true,
  downloadLimitBps: null,
  uploadLimitBps: 2_097_152,
  proxyUrl: null,
  theme: "system",
  density: "comfortable",
};

/**
 * A partly-downloaded piece map: a solid completed head, a ragged working
 * edge, then empty. Exercises all three visual regions of the heatmap.
 */
function pieceMap(): TorrentDetail["pieces"] {
  const buckets = Array.from({ length: 320 }, (_, i) => {
    if (i < 150) return 255;
    if (i < 175) return Math.floor(((i * 37) % 255) / 1);
    return 0;
  });
  // 150 full buckets plus a ragged 25, at 73 pieces each.
  const complete = 150 * 73 + Math.round(25 * 73 * 0.5);
  return {
    totalPieces: 23280,
    piecesComplete: complete,
    piecesPerBucket: 73,
    buckets,
  };
}

/**
 * Detail for one torrent, with the parts that must agree with its row
 * recomputed from that row.
 *
 * A fixed fixture would show "28 of 214 peers" in the note under a row reading
 * "41 / 206" — and the whole point of the note is that it explains the numbers
 * beside it. In the real engine both come off the same handle and cannot
 * disagree; a mock that does disagree teaches the wrong thing while you build
 * against it.
 *
 * @param id - Which torrent to describe.
 * @returns Detail matching that torrent's summary.
 */
function detailFor(id: number): TorrentDetail {
  const torrent = TORRENTS.find((t) => t.id === id) ?? TORRENTS[0];
  const remaining = Math.max(torrent.totalBytes - torrent.progressBytes, 0);

  return {
    ...DETAIL,
    // Live peers only exist while something is running.
    peers: torrent.livePeers === 0 ? [] : DETAIL.peers,
    pieces: torrent.livePeers === 0 && !torrent.finished ? null : DETAIL.pieces,
    swarm: {
      ...DETAIL.swarm,
      live: torrent.livePeers,
      seen: torrent.knownPeers,
    },
    note: {
      severity:
        torrent.state === "error"
          ? "err"
          : torrent.health === "none"
            ? "err"
            : torrent.state === "paused" || torrent.state === "checking"
              ? "neutral"
              : "ok",
      title:
        torrent.state === "error"
          ? "The disk this is saving to is full"
          : torrent.health === "none"
            ? "Nobody reachable has the rest of this"
            : torrent.state === "paused"
              ? "Paused, nothing lost"
              : torrent.state === "checking"
                ? "Checking what is already on disk"
                : torrent.state === "seeding"
                  ? `Serving ${torrent.livePeers} of ${torrent.knownPeers} known peers`
                  : `Pulling from ${torrent.livePeers} of ${torrent.knownPeers} known peers`,
      body:
        torrent.state === "error"
          ? `${torrent.error ?? ""} Free some space, then press Resume — everything already verified is kept and will not be downloaded again.`
          : torrent.health === "none"
            ? `${torrent.knownPeers} peers are known for this torrent and none of them is answering right now. Flume keeps asking the DHT and the trackers every few minutes.`
            : torrent.state === "paused"
              ? "Your data is verified on disk. Resuming reconnects to the swarm and picks up from there — nothing is downloaded twice."
              : torrent.state === "checking"
                ? "Flume is re-hashing to find out what survived. Anything that verifies is kept; only pieces that fail are downloaded again."
                : `${remaining.toLocaleString()} bytes to go at the current rate.`,
    },
  };
}

/**
 * The design's own add-sheet fixture.
 *
 * Chosen because it exercises the states a single-ISO torrent never reaches:
 * nested folders, a mix of file types, a 41.8 GB file beside a 24 MB one, and
 * one file already on disk so the deselect-and-explain path is visible.
 */
const PREVIEW: TorrentPreview = {
  infoHash: "9f2c4a1d3b5e7f9a0c2e4b6d8f0a1c3e5b7d9e17",
  name: "Sprite Fright — Blender Open Movie (4K ProRes master)",
  // Exactly the sum of the file lengths below. The engine computes this the
  // same way, so a mock that disagreed would only ever mislead.
  totalBytes: 62_304_022_200,
  alreadyAdded: false,
  savePath: "/Volumes/Media/Film",
  freeBytes: 1_420_000_000_000,
  seenPeers: 6,
  files: [
    {
      index: 0,
      path: "01 Final Renders/sprite-fright-4k-prores.mov",
      length: 41_800_000_000,
    },
    {
      index: 1,
      path: "01 Final Renders/sprite-fright-1080p-h264.mp4",
      length: 6_400_000_000,
    },
    {
      index: 2,
      path: "01 Final Renders/sprite-fright-4k-hdr.mkv",
      length: 3_900_000_000,
    },
    {
      index: 3,
      path: "02 Production Files/scenes.blend",
      length: 4_200_000_000,
    },
    {
      index: 4,
      path: "02 Production Files/textures.zip",
      length: 3_100_000_000,
    },
    { index: 5, path: "02 Production Files/rigs.blend", length: 1_900_000_000 },
    {
      index: 6,
      path: "02 Production Files/linked-assets.blend",
      length: 600_000_000,
    },
    { index: 7, path: "03 Extras/making-of-1080p.mp4", length: 380_000_000 },
    { index: 8, path: "03 Extras/poster-artwork.png", length: 24_000_000 },
    { index: 9, path: "03 Extras/credits.txt", length: 18_000 },
    { index: 10, path: "README.md", length: 4_200 },
  ],
  // The 1080p cut, matching the design's fixture: it is the one the sheet
  // deselects and explains in the footer.
  alreadyOnDisk: [
    false,
    true,
    false,
    false,
    false,
    false,
    false,
    false,
    false,
    false,
    false,
  ],
};

const DETAIL: TorrentDetail = {
  peers: [
    {
      address: "185.125.190.59:6881",
      client: "Transmission 4.0.6",
      transport: "tcp",
      state: "live",
      downloadedBytes: 940_000_000,
      uploadedBytes: 12_000_000,
      piecesContributed: 448,
      errors: 0,
    },
    {
      address: "91.189.91.157:6889",
      client: "libtorrent 2.0.10",
      transport: "tcp",
      state: "live",
      downloadedBytes: 610_000_000,
      uploadedBytes: 4_100_000,
      piecesContributed: 291,
      errors: 1,
    },
    {
      address: "[2001:67c:1360:8001::23]:51413",
      client: "qBittorrent 4.6.5",
      transport: "utp",
      state: "live",
      downloadedBytes: 305_000_000,
      uploadedBytes: 900_000,
      piecesContributed: 145,
      errors: 0,
    },
    {
      address: "203.0.113.44:6881",
      client: null,
      transport: "tcp",
      state: "live",
      downloadedBytes: 88_000_000,
      uploadedBytes: 0,
      piecesContributed: 0,
      errors: 7,
    },
  ],
  trackers: [
    "https://ipv6.torrent.ubuntu.com/announce",
    "https://torrent.ubuntu.com/announce",
  ],
  pieces: pieceMap(),
  swarm: {
    live: 28,
    connecting: 4,
    queued: 61,
    seen: 214,
    dead: 19,
    liveTcp: 26,
    liveUtp: 2,
  },
  // What the Rust derivation produces for a healthy download with these
  // numbers. Kept in step with `describe` in `src-tauri/src/engine/note.rs`.
  note: {
    severity: "ok",
    title: "Pulling from 28 of 214 known peers",
    body: "19.7 GB verified so far, 26.4 GB to go, arriving at 6.6 MB/s. About 1 h 07 min left at this rate.",
  },
};

/**
 * Installs the fake IPC layer on `window`.
 *
 * Safe to call more than once; subsequent calls are ignored.
 */
export function install(): void {
  const w = window as unknown as Record<string, never>;
  if ((w as Record<string, unknown>).__FLUME_MOCK_INSTALLED__) return;
  (w as Record<string, unknown>).__FLUME_MOCK_INSTALLED__ = true;

  // `?mock=empty` renders the empty state, which is otherwise unreachable
  // once the harness is populated. Empty and error states are exactly the
  // screens that get neglected, so they need to be easy to look at.
  const scenario = new URLSearchParams(window.location.search).get("mock");
  const torrents = scenario === "empty" ? [] : TORRENTS;
  // Mutable, so a written setting stays written for the life of the page.
  let live: Settings = { ...SETTINGS };

  const callbacks = new Map<number, (data: unknown) => void>();
  const listeners = new Map<string, number>();

  const internals = {
    transformCallback(cb: (data: unknown) => void) {
      const id = Math.floor(Math.random() * 1e9);
      callbacks.set(id, cb);
      return id;
    },
    async invoke(cmd: string, args?: Record<string, unknown>) {
      switch (cmd) {
        case "plugin:event|listen": {
          const { event, handler } = args as { event: string; handler: number };
          listeners.set(event, handler);
          return handler;
        }
        case "plugin:event|unlisten":
          return null;
        case "get_telemetry":
          return { core: CORE, torrents };
        case "get_core_status":
          return CORE;
        case "get_settings":
          return live;
        case "update_settings":
          // Echoes the written settings back and keeps them, as the real
          // command does. Returning a fixed object instead made every control
          // snap back the moment it was touched — a mock that cannot hold a
          // value cannot exercise a settings screen at all.
          live = ((args as { settings?: Settings } | undefined)?.settings ??
            live) as Settings;
          return live;
        case "preview_torrent":
          return PREVIEW;
        case "discard_preview":
          return null;
        case "confirm_add":
          return null;
        case "get_torrent_files":
          return FILES;
        case "get_torrent_detail":
          return detailFor(
            (args as { id?: number } | undefined)?.id ?? TORRENTS[0].id,
          );
        default:
          return null;
      }
    },
    runCallback(id: number, data: unknown) {
      callbacks.get(id)?.(data);
    },
  };

  Object.assign(
    ((w as Record<string, unknown>).__TAURI_INTERNALS__ ??= {}),
    internals,
  );
  Object.assign(
    ((w as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__ ??= {}),
    { unregisterListener: () => {} },
  );

  // Drive the telemetry stream at the same 1 Hz the backend uses.
  //
  // Each tick advances uptime and jitters the session rates. A mock that
  // resent one frozen snapshot would leave anything reading a history — the
  // dock's throughput chart — with a single sample and a flat line, which is
  // exactly the case least worth looking at while building it.
  let tick = 0;
  setInterval(() => {
    const handler = listeners.get("flume://telemetry");
    if (handler === undefined) return;

    tick += 1;
    // Two sine waves at different periods, so the trace has the uneven shape
    // of real traffic rather than an obvious repeating pattern.
    const wobble = (period: number, phase: number) =>
      Math.sin((tick / period) * Math.PI * 2 + phase);
    const downloadBps = Math.max(
      0,
      Math.round(
        CORE.downloadBps * (1 + 0.28 * wobble(17, 0) + 0.12 * wobble(5, 1.7)),
      ),
    );
    const uploadBps = Math.max(
      0,
      Math.round(
        CORE.uploadBps * (1 + 0.34 * wobble(11, 2.4) + 0.15 * wobble(4, 0.6)),
      ),
    );

    callbacks.get(handler)?.({
      event: "flume://telemetry",
      id: handler,
      payload: {
        core: {
          ...CORE,
          uptimeSeconds: CORE.uptimeSeconds + tick,
          downloadBps,
          uploadBps,
        },
        torrents,
      },
    });
  }, 1000);
}
