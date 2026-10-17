# Changelog

Notable changes to Flume. Format follows [Keep a Changelog][kac]; versioning is
[semantic][semver].

[kac]: https://keepachangelog.com/en/1.1.0/
[semver]: https://semver.org/spec/v2.0.0.html

## [Unreleased]

## [1.4.0] — 2026-10-17

### Added

- **A download folder per torrent.** The add sheet shows where the torrent
  will be written and lets you change it, defaulting to wherever the last add
  went. Set at add time, which is the honest limit: moving a torrent's data
  afterwards is a different operation and is still to come.

  The part worth knowing is that **changing the destination re-asks the
  questions the sheet exists to answer.** Free space belongs to a volume and
  "you already have this file" belongs to a folder, so both figures the sheet
  arrived with describe the _default_ destination and neither survives picking
  another one. They are recomputed for the folder you chose — picking a
  different disk is usually _why_ you are changing it, so showing the original
  volume's free space would be wrong exactly when it is being relied on.

  Your file selection is left alone when the destination changes. It started
  as "everything you do not already have", and a file that is absent from the
  new folder now shows unmarked and unselected rather than being silently
  ticked back on.

- **A queue you can reorder, and a queue that says what it is doing.** Right-
  click a waiting torrent for **Move up in queue** and **Move down in queue**.
  Before this the queue admitted strictly oldest-first with no way to change
  it, which was the last open item on the queue work.

  The order is decided in Rust from the engine's own snapshot, not from the
  list on screen — that is sorted by whatever you chose, and has nothing to do
  with what the queue admits next. The expanded row now shows a torrent's
  place in the line, and shows it _only_ when the queue holds one: a dash
  would imply the queue has an opinion about every torrent, and it does not.

  **There can be more than one line, and that is worth knowing if you set
  limits per kind.** A cap on downloads says nothing about which seed runs
  next, so with only that set your seeds have no place at all: the **In
  queue** line is absent from the expanded row rather than showing a dash,
  and there is no move action — there is no line for them to be in. Cap both kinds and you have two
  lines, ranked separately: a download and a seed can both read "1", because
  each is first of what it is actually waiting behind, and a download cannot
  be moved past a seed because doing so would change neither one's turn. A
  total limit puts them in one line together — but only if it can actually
  bite: one download and one seed cannot breach a total of ten, so that total
  decides nothing and the two lines stay separate.

  A torrent you started past the limits now says so, as **Queue: Started by
  you**. It runs on top of the limits rather than inside them — neither
  admitted nor counted — and without saying that, a limit that is being
  overridden reads as a limit that is broken.

- **A peer limit you can see and set.** Settings → Network now carries "Connect
  to at most", a per-torrent cap on connected peers, and the detail panel shows
  the live count against it — `28 / 128` rather than `28`.

  This makes an existing ceiling visible rather than introducing one, which is
  worth being plain about. Flume passed librqbit nothing for this, and nothing
  does not mean unlimited: the engine falls back to 128 per torrent, sizing a
  semaphore nobody could see. So the default is 128 — the number already in
  force — and upgrading changes how no existing install behaves.

  The limit is per torrent because that is the only shape the engine offers, so
  ten torrents at 128 is a ceiling of 1280 sockets. The settings copy says so,
  since that is the figure a modest router's NAT table has to hold. Changing it
  needs a session restart, and the panel reports the limit the engine is
  actually enforcing rather than the one saved on disk, so the two cannot
  silently disagree while a restart is pending.

- **Sort the list by any of ten keys, either way round, and it is remembered.**
  Name, size, progress, both speeds, ratio, time left, date added, state, and
  activity — the download and upload rates together, which stays the default.
  The toolbar's three chips become a picker and a direction toggle; ten chips
  would not fit, and the overflow would have to be a menu anyway.

  The ordering is a tested module rather than a comparator inline in the list,
  because the properties are easy to get subtly wrong. It is **total**: every
  comparison ends on the session id, so no two rows ever tie. Rows update
  every second and the list re-sorts on each tick, so without that, equal rows
  swap places continuously and clicking one is aiming at a moving target.
  Rows with **no value stay last whichever way the arrow points** — "no
  estimate" is not "finishes soonest" — and flipping the direction never
  reorders rows that tie, because the tie-breaks do not reverse.

  Choosing a key starts it in the direction it is useful in: largest first for
  the numbers, A to Z for the name. The key names are column names, so #147
  can read this value when sorting moves into the column headers rather than
  having to migrate it.

- **IP blocklists and allowlists.** Settings → Network takes a URL for each —
  `http://`, `https://` or `file://`. A blocklist refuses peers inside it; an
  allowlist refuses everything outside it.

  The rail's network footer reports **how many ranges actually loaded**, and
  that is the part worth having rather than the setting itself. librqbit reads
  a list that fetched with a clean 200 and parsed to nothing exactly as it
  reads one that worked, so an HTML error page, a login redirect or a format
  it does not understand gives you zero ranges and no complaint. Believing you
  are filtered when you are not is the failure this feature has to avoid, so
  the footer says "Blocklist loaded nothing" rather than staying quiet. For an
  allowlist it says "no peers can connect", because an empty exclusive list
  refuses everyone.

  A list Flume cannot fetch at all stops the engine rather than starting it
  unfiltered — the same reasoning. If that happens while changing the setting,
  the previous settings are restored and the engine comes back, so an
  unreachable host cannot strand the next launch.

  The settings copy is deliberately unflattering about what this buys:
  blocklists are large, stale and of debated effectiveness, and they do not
  make anyone anonymous.

### Fixed

- The limiting-factor panel's notes claimed connection slots had no ceiling to
  measure against, on the grounds that no limit was set. There was always a
  ceiling; Flume just did not know its value. ([#63][i63])
- Text settings committed on every keystroke, so a field whose value is
  validated could not be typed into at all — the first character of
  `https://…` is `h`, which is rejected, and the field snapped back before the
  second arrived. The only way to set one was to paste it whole. They now
  commit on blur or Enter, and Escape abandons the edit. The proxy URL field
  had this from the day it shipped. ([#62][i62])
- The egress guard retried a failed engine start every second, indefinitely.
  Harmless while a start could only fail locally; with a blocklist URL in the
  mix each retry carried an HTTP request to a third party. It now backs off
  from two seconds to a minute, and still never gives up. ([#62][i62])
- Settings changes made while a slow one was still saving could overwrite
  each other, and a control could visibly snap back to its old value until
  its own save finished. The settings screen installs whatever the backend returns, and
  a filter URL makes a save take seconds, so a theme toggle started later
  could finish first and then be replaced by the older reply — and lost on
  disk at the next edit. The screen's writes are serialised now, as the main
  window's already were. ([#62][i62])
- An engine that finished starting after the tunnel guard began holding was
  published anyway, and transferred until the next tick noticed it. The guard
  could not stop it: its held branch looks for an engine to shut down, and
  the restart had already taken the old one out, so there was nothing there.
  A session built under a held guard is now discarded. ([#201][i201])
- The tunnel guard could tear down an engine a settings change had just
  built and rebuild the same thing, because the engine is briefly absent
  during a restart and the guard read that as "nobody has started one". With
  a filter list to fetch, that gap spans several of its one-second checks.
  ([#201][i201])
- Two settings changes close together could start two librqbit sessions at
  once, fighting over the same port and session directory, with the slower
  one publishing itself last — so the newer settings could be saved and not
  in force. Restarts are serialised now. The window was microseconds wide
  until a blocklist URL made starting the engine fetch over the network.
  ([#201][i201])
- Escape in a settings text field abandoned the edit and closed the whole
  dialog, because the dialog listens for it on `window` and `preventDefault`
  does not stop an event getting there. ([#62][i62])
- A setting that failed to restart the session was saved to disk anyway, so it
  was retried on the next launch with the same result and no way to correct it
  from inside the app. The previous settings are now restored. Reachable
  before this with a listen port already in use, and trivially reachable with
  a blocklist URL. ([#62][i62])

[i62]: https://github.com/adamgreenwell/flume/issues/62
[i201]: https://github.com/adamgreenwell/flume/issues/201
[i63]: https://github.com/adamgreenwell/flume/issues/63

## [1.3.0] — 2026-09-10

### Added

- **Queue management.** Flume can be told how many torrents run at once, under
  Settings → Queue. Three limits rather than one, because downloading and
  seeding do not cost the same thing: a download uses the connection in both
  directions and writes to disk, while a finished torrent only uploads. Set
  either or both, and a total as a ceiling over them; leave one unset and it
  does not apply.

  Torrents beyond a limit **wait** rather than stop. Queued is its own state,
  not a paused one — it draws a clock rather than a pause mark, says "queued —
  starts on its own when a slot frees", and reads that way to a screen reader
  too. The distinction matters because the two look identical and behave
  oppositely: a paused torrent stays put, a queued one starts itself the moment
  a slot frees. Admission is oldest first, and a torrent added before Flume
  recorded arrival times sorts last rather than first.

  A torrent stopped by a seed limit is never started by the queue, and does not
  hold a slot it is not using. That precedence is decided once, in one place,
  rather than per feature.

- **Resuming a queued torrent runs it now, and keeps it running.** Resuming
  something the queue parked takes it out of the queue for good — it keeps
  running past the limit until you pause it again, which is how you say
  otherwise. It runs on top of the limits rather than displacing anything, so
  starting one torrent never stops a different one you did not touch. Resuming
  a torrent **you** paused is unchanged and simply un-pauses it: it rejoins the
  queue and waits its turn.

  The choice survives quitting. Without that, the queue would park the torrent
  again on the next launch — the same surprise, one restart later.

## [1.2.0] — 2026-09-09

### Fixed

- **"Recently added" now sorts by when a torrent actually arrived.** It sorted
  by session id, which is not the same thing: librqbit persists no timestamp
  and computes the next id as one past the highest still present, so removing
  the newest torrent hands its number to the next one added. Flume now keeps
  its own record of when each torrent arrived, keyed by info hash. Torrents
  added before this release have no recorded time and sort last rather than
  being given an invented one.
- **A torrent whose `.torrent` file went missing could hang Flume's startup
  indefinitely.** librqbit reads a missing sidecar as empty bytes rather than
  as an error and restores the row as a magnet, and that path has no timeout —
  so for a torrent nobody is seeding, the session never finished starting. In
  1.1.0 that also meant the tunnel check's loop never started, leaving its
  status frozen with no way to recover from inside the app. Starting the
  session is now bounded, and the failure says your library has not been
  deleted.
- **A start that fails now says which torrent is the reason, and says it in the
  app.** The bounded start above stops the hang but not the problem: Flume
  still could not name the row to remove, and the explanation only ever reached
  the log file, because the window kept showing "the torrent engine is still
  starting" for the rest of the session. Flume now checks which persisted
  torrents have no `.torrent` file beside them, names them in the message, and
  the window shows that message instead of a startup sentence that has stopped
  being true. Nothing is deleted or repaired on your behalf — a sidecar can go
  missing because a drive is not mounted yet, which is not a torrent you want
  dropped.

### Added

- **Seed ratio and seed time limits.** Flume can now be told when to stop
  seeding, globally under Settings → Seeding or for one torrent from its
  inspector. Both limits are optional and independent, and whichever is reached
  first wins. Seeding time counts time actually spent seeding rather than time
  since the torrent was added, and it survives quitting — a limit set in hours
  means the same thing across a restart.

  A torrent stopped this way says so, in the list and in its detail panel,
  rather than showing a bare "paused" that looks indistinguishable from a
  failure. **Keep seeding** lifts the limit for that torrent and starts it
  again; it appears in place of Resume, because resuming alone would leave the
  limit in force and stop the torrent a second later.

  A per-torrent limit replaces the global one outright rather than merging with
  it, which is what makes "seed this one forever" expressible. Per-torrent
  limits are removed along with their torrent.

- **Collapsible sidebar.** The rail collapses to a 56px icon rail and back,
  from a toggle beside the wordmark or from Settings → Appearance, and the
  choice survives a relaunch. It never collapses to nothing: the network
  footer is where a held tunnel check is reported, so collapsed it becomes one
  status dot whose name carries all three lines. Pressing `/` on a collapsed
  rail expands it and puts the caret in the search field rather than doing
  nothing.

## [1.1.0] — 2026-09-01

### Added

- **Only transfer through a tunnel.** An opt-in check that works out which
  network interface your traffic would actually leave by, and can hold all
  transfer while that is not a tunnel. It reads your own routing table and
  sends nothing — no "what is my IP" request to a web service, which would mean
  handing your address to a stranger in order to be told it is protected.

  Holding does not pause your torrents: Flume runs no torrent session at all,
  so there are no transfers, no peer connections, no DHT and no listening port.
  Nothing is written to your torrents, so whatever you paused stays paused and
  whatever was running comes back running. The check runs before the engine
  starts, so nothing announces to a tracker from an address you did not intend.

  A drop takes effect immediately; recovery waits about ten seconds of a steady
  tunnel, so a reconnecting VPN does not re-announce your library to every
  tracker. Off by default. See the User Guide for what the check cannot tell
  you — a PPPoE line looks like a tunnel from here, and OpenVPN on Windows does
  not.

## [1.0.0] — 2026-08-30

First public release.

### Added

- **Torrent lifecycle.** Add by `.torrent` or magnet with a file picker before
  anything downloads, then pause, resume, and remove — with a confirmation
  before any data is deleted.
- **Swarm health.** Every torrent reports whether it will actually finish, not
  just whether it is moving. Computed from the connected peers' bitfields:
  `Healthy` when every piece is well covered, `Thin swarm` when it is covered
  but barely, `No seeds` when some piece is held by nobody at all.
- **"What is limiting this download".** Ranks the constraints Flume can measure
  — your download cap, piece availability, peer upload — and marks at most one
  as binding, with a sentence saying whether changing a setting would help.
- **Piece map.** Two stacked strips sharing one bucketing: what you have above,
  how many peers hold each region below. A tail thinning to the right is a
  download heading for a stall.
- **Import from other clients.** Detects Transmission, qBittorrent and Deluge,
  and takes their torrents over in place — nothing is downloaded twice.
- **First run.** Three questions rather than thirty settings.
- Settings apply immediately, with no OK or Apply button, and survive a restart.
- File selection can be changed after a torrent is added.
- Desktop notifications on completion; magnet links can be opened from a
  browser.

### Fixed

- **Windows: seeding a file another application holds open.** librqbit opened
  every file read _and_ write, including completed torrents that are only ever
  served, so a media player or antivirus holding a download blocked the torrent
  from being added at all. Sent upstream as [`ikatson/rqbit#645`][r645].

### Known limitations

- **Windows builds are unsigned.** SmartScreen warns on an unsigned executable
  until a download reputation builds. The wiki's Signing & Distribution page
  covers the least alarming way through. macOS is signed with a Developer ID
  certificate and notarized by Apple, so Gatekeeper does not warn.
- **Flume runs a patched librqbit.** Two changes are carried on a fork and are
  with upstream as [#644][r644] and [#645][r645]; the patch is removed when they
  land in a release. See the README.
- Per-tracker announce status is not shown — librqbit does not expose it.
- Availability is not shown for a completed torrent: it measures whether the
  _swarm_ can finish your download, a question already answered once you have
  every piece.

[1.4.0]: https://github.com/adamgreenwell/flume/releases/tag/v1.4.0
[1.3.0]: https://github.com/adamgreenwell/flume/releases/tag/v1.3.0
[1.2.0]: https://github.com/adamgreenwell/flume/releases/tag/v1.2.0
[1.1.0]: https://github.com/adamgreenwell/flume/releases/tag/v1.1.0
[r644]: https://github.com/ikatson/rqbit/pull/644
[r645]: https://github.com/ikatson/rqbit/pull/645
[1.0.0]: https://github.com/adamgreenwell/flume/releases/tag/v1.0.0
