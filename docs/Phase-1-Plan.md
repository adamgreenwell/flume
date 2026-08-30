# Phase 1 Plan — Core Torrent Lifecycle (MVP)

The phase that turns Flume from a proof of the IPC path into a usable client.

**Goal:** a user can add a Linux ISO by magnet link or `.torrent` file, choose
which files they want, watch it download, and find it on disk afterwards — and
all of that survives a restart.

## Product decisions (confirmed)

| Question         | Decision                               | Consequence                                                                     |
| ---------------- | -------------------------------------- | ------------------------------------------------------------------------------- |
| Add flow         | **Show file picker first, then start** | Adding is a two-step flow. Nothing downloads until the user confirms selection. |
| Layout           | **Single list + detail panel**         | One route. Selecting a row opens a detail panel rather than navigating away.    |
| Settings storage | **`tauri-plugin-store` (JSON)**        | No SQLite dependency. Settings are an inspectable JSON file in app-data.        |

### Why the add flow matters most

Choosing "file picker first" is the right call for the ISO use case and it
shapes the engine work. Distro torrents routinely bundle several images plus
checksums, and downloading 4 GB of the wrong image is the exact frustration
Flume exists to avoid.

It does mean the add path must resolve metadata **before** starting transfer.
For a `.torrent` file the file list is available immediately. For a magnet link
it is not — metadata must be fetched over the DHT first. So the add dialog needs
a genuine resolving state, and that state can fail or take a while.

This is the single most important interaction in the app. It should feel
deliberate, not like a modal in the way.

## Build order

Each step ends green-tested and committed. Issue numbers link the tracker.

### 1. Event-based telemetry ([#5][5] groundwork)

Replace the Phase 0 polling hook before anything depends on it. Polling one
status card is fine; polling a list of torrents is not.

- Rust: a 1 Hz task emitting a batched `TorrentsUpdate` payload via `emit`
- Frontend: `listen()` subscription replacing `useCoreStatus`'s timer
- Keep `get_core_status` as a command for initial paint

**Why first:** every later feature consumes this stream. Building it after the
list means rewriting the list.

### 2. Engine: add, control, remove ([#3][3], [#4][4], [#5][5])

Engine layer, still Tauri-free:

- `add_torrent(source, options) -> TorrentId` where source is magnet or bytes
- `list_torrents() -> Vec<TorrentSummary>`
- `pause`, `resume`, `remove(id, delete_files: bool)`
- Magnet URI validation **in Rust**, not just the UI — it is untrusted input

Integration tests against a real session using a well-seeded Linux ISO magnet,
`#[ignore]`d like the DHT test.

### 3. Add flow UI ([#3][3], [#4][4])

- Add dialog with magnet paste box and `.torrent` picker
  (`tauri-plugin-dialog`, permission `dialog:allow-open` only)
- Clipboard magnet detection on window focus
- **Resolving state** for magnets, with a cancel affordance
- File tree with checkboxes, size per file, select-all/none
- Confirm starts the download with the chosen selection

### 4. Torrent list ([#5][5])

- Row: name, progress, down/up speed, ETA, peers, ratio, state
- Pause/resume/remove per row
- Remove confirmation with an explicit "also delete files" checkbox,
  **unchecked by default**
- Empty state that tells a first-time user what to do

### 5. Detail panel ([#5][5], groundwork for [#11][11])

Selecting a row opens a panel showing file list with per-file progress, and
basic peer/tracker counts. The richer detail view (piece heatmap) is Phase 2.

### 6. File selection changes after add ([#6][6])

`Session::update_only_files` wired to the detail panel's file tree, so a user
can change their mind mid-download.

### 7. Settings ([#7][7])

- `tauri-plugin-store`, settings JSON in app-data
- Download directory, global rate limits, max active torrents, listen port,
  UPnP toggle, DHT toggle, theme
- Port and DHT changes restart the session cleanly rather than needing an app
  relaunch
- Settings validated on load; a corrupt file falls back to defaults with a
  warning rather than refusing to start

### 8. Persistence verification ([#8][8])

Not new code so much as proof: add a large torrent, kill the app mid-download,
relaunch, confirm it resumes without a full re-hash. Automate what can be
automated.

### 9. Windows seeding investigation ([#9][9])

Confirm whether the file-locking problem reproduces on librqbit v9 **before**
writing any patch. Document either way.

## New dependencies

| Crate / package       | Why                      |
| --------------------- | ------------------------ |
| `tauri-plugin-dialog` | `.torrent` file picker   |
| `tauri-plugin-store`  | Settings persistence     |
| `tauri-plugin-opener` | "Open containing folder" |

Each adds exactly one capability permission. No shell plugin.

## Risks

**Magnet metadata resolution can be slow or fail.** The add dialog must handle
a magnet that never resolves — a timeout with a clear message, not a spinner
forever.

**Rate limiting is per-session in librqbit.** Per-torrent limits need
verification against the v9 API before promising them in settings; if they are
not supported, say so rather than shipping a control that does nothing.

**Event volume.** One batched update per second regardless of torrent count.
Resist the temptation to emit per-torrent events.

## Definition of done

- [ ] Add a Linux ISO by magnet and by `.torrent`, choosing files first
- [ ] Pause, resume, and remove work, with delete-files confirmation
- [ ] Settings persist and take effect without a relaunch
- [x] Kill and relaunch mid-download resumes without full re-hash
- [x] Telemetry is event-based and batched at ~1 Hz
- [x] All CI gates green; new engine logic covered by tests
- [x] Wiki User Guide updated to describe what actually shipped

### What a real build has shown so far

A real torrent has been added, completed, seeded, and survived both a clean
restart and a mid-download kill, resuming correctly each time. That is most of
the first and fourth boxes, but neither is ticked yet and the reasons are worth
keeping:

**The first box asks for a Linux ISO by magnet _and_ by `.torrent`.** The
`.torrent` route is done — a Debian ISO added, completed, and seeded. The magnet
route is not, and it is not a formality.

Both converge on the same `list_only: true` preview, but they reach it
differently: a `.torrent` is `AddTorrent::from_bytes` and has its metadata in
hand, so the listing returns near-instantly. A magnet is `AddTorrent::from_url`
and has to fetch metadata **from peers over the DHT** before any file listing
can exist.

The engine side of that is already covered by
`magnet_resolves_real_metadata_over_the_dht` (`#[ignore]`d — it needs real
peers). What no test covers is the UI across a preview that takes seconds
rather than milliseconds. Worth watching when it is run:

- what the add dialog shows while metadata resolves, rather than after
- that the DHT reads **Ready** first; a magnet added while **Connecting** sits
  waiting, which the User Guide already warns about
- a magnet nobody is seeding — whether that surfaces as an error or hangs

**The fourth box is met.** Verified on a real Debian torrent: a mid-download
kill resumed correctly, and a clean quit relaunches straight into seeding with
no `Checking` state.

The mechanism, since "it looked fast" is not evidence on its own:

- `fastresume: true` on `SessionOptions`. librqbit defaults it to **false**, and
  with it false the JSON store is paired with `NonPersistentBitVFactory` and
  every launch re-hashes everything.
- The bitfield is flushed **during** the download, not only at exit.
  `on_piece_completed` accumulates `unflushed_bitv_bytes` and flushes every
  16 MiB, synchronously again when the torrent finishes, and once more on
  `Drop`. A killed process therefore leaves a bitfield at most 16 MiB stale, so
  a kill costs re-downloading up to 16 MiB — not re-hashing the torrent.
- `RunEvent::Exit` calling `session.stop()` is the final tidy-up rather than the
  thing that makes this work. An earlier version of this note had that backwards
  and concluded a kill must re-hash; it does not.

The state is observable rather than inferred: `<info-hash>.bitv` in the app data
directory is the persisted bitfield, and it is what a relaunch loads instead of
hashing. On a finished torrent every bit is set.

A re-hash, when it does happen, shows as the `Checking` state.

## Open questions for Phase 2

Deferred deliberately, not forgotten:

- Should completed torrents keep seeding by default, or stop at ratio 1.0?
- Does the tray icon own "pause all", or does the main window?
- Light theme: a true light palette, or a dimmed variant of the dark one?

[3]: https://github.com/adamgreenwell/flume/issues/3
[4]: https://github.com/adamgreenwell/flume/issues/4
[5]: https://github.com/adamgreenwell/flume/issues/5
[6]: https://github.com/adamgreenwell/flume/issues/6
[7]: https://github.com/adamgreenwell/flume/issues/7
[8]: https://github.com/adamgreenwell/flume/issues/8
[9]: https://github.com/adamgreenwell/flume/issues/9
[11]: https://github.com/adamgreenwell/flume/issues/11
