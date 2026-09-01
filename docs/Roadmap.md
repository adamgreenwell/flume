# Roadmap

Tracked on the [project board](https://github.com/users/adamgreenwell/projects/8)
and in [issues](https://github.com/adamgreenwell/flume/issues).

## Phase 0 — Scaffold & repo hygiene ✅

**Complete** — merged in [#20](https://github.com/adamgreenwell/flume/pull/20), milestone closed, CI green on `main`.

- Next.js 16 static export inside a Tauri v2 shell
- librqbit v9 embedded, DHT bootstrapping, UPnP forwarding, persistence
- `get_core_status` proving the full IPC path end to end
- Dark landing page showing live engine telemetry
- Apache-2.0, README, CONTRIBUTING, issue/PR templates, Dependabot
- CI: fmt, clippy, tsc, ESLint, Prettier, Vitest, cargo test, audits
- 5 Rust unit tests, 6 integration tests (engine + IPC), 20 frontend tests
- Three security advisories found and fixed by CI before the first merge

## Phase 1 — Core torrent lifecycle (MVP) 🚧

The phase that makes Flume usable. **The core loop works**: telemetry is
event-based, and torrents can be added by magnet or file with a
select-files-first flow, then paused, resumed, and removed.

- ~~Add via magnet link~~ ✅ ([#3](https://github.com/adamgreenwell/flume/issues/3))
- ~~Add via `.torrent` file picker~~ ✅ ([#4](https://github.com/adamgreenwell/flume/issues/4))
- ~~Torrent list: progress, speeds, ETA, peers, ratio; pause/resume/remove~~ ✅ ([#5](https://github.com/adamgreenwell/flume/issues/5))
- ~~Per-torrent file tree with selective download~~ ✅ ([#6](https://github.com/adamgreenwell/flume/issues/6))
- ~~Settings with persistence~~ ✅ ([#7](https://github.com/adamgreenwell/flume/issues/7))
- ~~Resume correctly across restarts~~ ✅ ([#8](https://github.com/adamgreenwell/flume/issues/8))
- Investigate Windows file-locking and seeding ([#9](https://github.com/adamgreenwell/flume/issues/9))

Also in this phase: replace the Phase 0 polling hook with backend-pushed,
batched events before the torrent count grows.

## Phase 2 — Polish & platform integration ✅

**Complete.** Milestone closed 5/5.

- ~~Theming and the visual design pass~~ ✅ ([#10](https://github.com/adamgreenwell/flume/issues/10))
- ~~Per-torrent detail view with piece heatmap~~ ✅ ([#11](https://github.com/adamgreenwell/flume/issues/11))
- ~~Magnet protocol association and single instance~~ ✅ ([#12](https://github.com/adamgreenwell/flume/issues/12))
- ~~Notifications and system tray~~ ✅ ([#13](https://github.com/adamgreenwell/flume/issues/13))
- ~~Keyboard shortcuts and accessibility baseline~~ ✅ ([#14](https://github.com/adamgreenwell/flume/issues/14))

## Phase 3 — Hardening & distribution 🚧

- ~~Release pipeline for all four package formats~~ ✅ ([#15](https://github.com/adamgreenwell/flume/issues/15))
- ~~Performance validation with 10+ torrents~~ ✅ ([#17](https://github.com/adamgreenwell/flume/issues/17))
- ~~Signing, notarization, and troubleshooting docs~~ ✅ ([#18](https://github.com/adamgreenwell/flume/issues/18))
- ~~Release build blocked by a proc-macro failure~~ ✅ ([#22](https://github.com/adamgreenwell/flume/issues/22))

Still open, all blocked on something outside the code:

- Sequential download ([#16](https://github.com/adamgreenwell/flume/issues/16)) — **not
  implementable** against librqbit v9; `FilePriorities` is `pub(crate)` and no priority
  setter exists. Awaiting a product decision.
- GTK3 advisories ([#21](https://github.com/adamgreenwell/flume/issues/21)) — upstream in
  Tauri's Linux backend; resolves when it moves off GTK3.
- TypeScript 7 ([#28](https://github.com/adamgreenwell/flume/issues/28)) — blocked until
  `typescript-eslint` supports it.

### Measured performance

With 15 torrents on macOS:

| Metric                 | Value       | Budget                   |
| ---------------------- | ----------- | ------------------------ |
| `telemetry()` per call | 171 µs      | 1,000,000 µs (1 Hz tick) |
| Serialized payload     | 5,345 bytes | —                        |
| Detail + files query   | 2.2 µs      | 500,000 µs (2 Hz panel)  |

The payload figure is the one to watch: it should scale with torrent _count_,
never with piece count or file size.

## Diagnostics and usage reporting

Added on top of Phase 3, and not something an earlier phase asked for.

- **Diagnostics report** — Settings → Privacy builds a redacted bundle the user
  copies into an issue. Nothing is sent. This is the piece that closes the
  "Windows and Linux unverified locally" gap below: it turns "it doesn't work"
  into a paste naming the bound port, the DHT node count and the log tail.
- **Usage counts** — opt-in, asked once at first run, off unless granted. Sent
  from Rust; the webview's CSP forbids it reaching the network at all.
- **Collector** — a Cloudflare Worker and D1 database in `collector/`, which
  validates every batch against the wire format and rejects anything it does
  not recognise rather than storing it.

Deliberately **not** included: crash reporting. A Rust panic hook is easy, but
it catches only Rust panics — not a segfault in a dependency, an OOM kill, or a
webview crash, and the webview is a separate process running a different engine
on each platform. Real native capture means minidumps, a crash-handler child
process, symbol upload for three targets per release, and a macOS
hardened-runtime interaction. That is its own project.

See [[Privacy]].

## The tunnel check

Added on top of Phase 3, and not something any earlier phase asked for
([#143](https://github.com/adamgreenwell/flume/issues/143)).

Opt-in, off by default. Flume works out which interface traffic would actually
leave by — a route lookup against the local table, sending nothing — and can
hold all transfer while that is not a tunnel.

Two decisions are worth knowing without reading the code:

- **Holding stops the engine rather than pausing torrents.** `Session::pause`
  writes `is_paused` to `session.json` synchronously and librqbit stores one
  paused bit with no reason, so a guard pause would be indistinguishable from
  the user's — and quitting while held would strand the library paused with the
  tunnel up. Stopping the session touches no torrent at all, and is stronger:
  pausing leaves the DHT, the listener and UPnP running.
- **The first check runs before any engine exists.** librqbit restores _and
  starts_ persisted torrents inside `Session::new_with_opts`, so a session built
  before the check has already announced.

Not covered, deliberately: OpenVPN on Windows, which is indistinguishable from
an Ethernet adapter through what `network-interface` exposes. The interface pin
is the workaround. Closing it properly needs `IP_ADAPTER_ADDRESSES.IfType`,
which means a new dependency for one protocol on one platform.

See [[Privacy]] and [[User-Guide]].

## Phase 4 — Client feature parity 🚧

Seventeen issues on the
[Phase 4 milestone](https://github.com/adamgreenwell/flume/milestone/5),
sequenced in four waves. The waves are dependency order, not priority:

- **Wave 1** — the policy engine
  ([#54](https://github.com/adamgreenwell/flume/issues/54)) and what sits
  directly on it: seed ratio and time limits
  ([#55](https://github.com/adamgreenwell/flume/issues/55)) and queue management
  ([#56](https://github.com/adamgreenwell/flume/issues/56)); plus sort and
  filter ([#57](https://github.com/adamgreenwell/flume/issues/57)), which
  depends on nothing.
- **Wave 2** — labels ([#58](https://github.com/adamgreenwell/flume/issues/58)),
  scheduled alternative limits
  ([#59](https://github.com/adamgreenwell/flume/issues/59)), incomplete
  directory ([#60](https://github.com/adamgreenwell/flume/issues/60)).
- **Wave 3** — independent plumbing: per-torrent download folder
  ([#61](https://github.com/adamgreenwell/flume/issues/61)), blocklists
  ([#62](https://github.com/adamgreenwell/flume/issues/62)), peer limits
  ([#63](https://github.com/adamgreenwell/flume/issues/63)), watch folder
  ([#64](https://github.com/adamgreenwell/flume/issues/64)), all-time statistics
  ([#65](https://github.com/adamgreenwell/flume/issues/65)).
- **Wave 4** — the re-add primitive
  ([#66](https://github.com/adamgreenwell/flume/issues/66)) and the three
  features it unlocks: force recheck
  ([#67](https://github.com/adamgreenwell/flume/issues/67)), relocate
  ([#68](https://github.com/adamgreenwell/flume/issues/68)), edit trackers
  ([#69](https://github.com/adamgreenwell/flume/issues/69)).

### Field notes from 1.0

Four items that came out of using the shipped client rather than out of a
feature comparison. They are placed inside the waves above rather than beside
them.

The decisions are taken. They are recorded here so none of them is re-opened
from scratch:

| Question            | Decision                                    | Why                                                                                                                         |
| ------------------- | ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Collapsed rail      | An icon rail, never zero width              | The network footer carries the guard's held state; a rail that can hide it turns a deliberate hold into unexplained silence |
| Columns             | User-configurable — choose, reorder, resize | A fixed set makes the overflow work anyway; letting the user pick is the reason to build it once rather than twice          |
| Magnet deadline     | Ten seconds, then offer to add it anyway    | Sixty seconds was right when failing was the only outcome. With a fallback, the deadline's job is to reach the choice fast  |
| A magnet that lands | Holds for file selection, never auto-starts | The add flow exists so nobody fetches gigabytes of the wrong thing, and an unattended resolve is exactly that case          |
| Per-torrent limits  | Deferred until librqbit answers             | The add-time-only version is worse than nothing, and the fix belongs upstream rather than in a third local patch            |

#### A per-torrent record that Flume owns

**Build this first. It is the only new persistent concept here, and two of the
four items below need it.**

librqbit persists exactly five things per torrent — info hash, trackers, output
folder, file selection, paused bit. Everything else Flume wants to remember
about a torrent has nowhere to live:

- **When it was added.** There is no timestamp in the engine at any layer. The
  current "recently added" sort uses the librqbit session id, which is closer
  to right than it looks — ids are persisted and restored through
  `preferred_id`, so they survive a restart — but `next_id` is `max(key) + 1`
  over what is still there, so removing the newest torrent hands its number to
  the next add. Arrival order holds until something is removed, and a column
  showing a _date_ is not possible at any effort.
- **A magnet that has not resolved.** It is not a torrent yet, so librqbit has
  nowhere to put it (see below).
- **A per-torrent rate limit**, when that becomes buildable.

So: `src-tauri/src/library/`, Tauri-free like `engine` and `settings`, owning a
`library.json` beside `settings.json` — one record per info hash. Later it is
where a label ([#58](https://github.com/adamgreenwell/flume/issues/58)), a
per-torrent folder ([#61](https://github.com/adamgreenwell/flume/issues/61)) and
the policy engine's counters
([#54](https://github.com/adamgreenwell/flume/issues/54)) go, and
[#66](https://github.com/adamgreenwell/flume/issues/66) has already concluded
that Flume state should key on info hash throughout, because session ids do not
survive a re-add.

It is deliberately **not** part of `Settings`. Settings is a flat object behind
a whole-object write that runs validation, a restart check, live limit
application and a usage diff on every call. A per-torrent map with a different
write cadence does not belong behind that.

#### Collapsible sidebar

**Wave 1. No dependencies, no engine work, and the smallest of the four.**

The rail is a hard-coded `grid-cols-[248px_1fr]` in `src/app/page.tsx`.
Collapsing it means a CSS variable and one more frontend-only field in
`Settings`, beside `theme` and `density` — both of which are already stored
there for the same reason: a preference the user re-sets on every launch is not
a preference.

`TitleBar` is `col-span-full`, so the 88px macOS traffic-light inset lives in
the title-bar row and is unaffected by the rail's width. There is no
per-platform work in this at all.

Two things live in the rail that must not collapse with it. The search field
owns the `/` shortcut, so a collapsed rail has to expand and focus rather than
swallow the key. And the network footer — DHT nodes, listening port, and the
egress guard's line — is the rail's most valuable content.

**It collapses to an icon rail, not to zero.** Roughly 56px keeps the view icons
and a status dot, which is enough to carry a held guard with a tooltip. Zero
width is only honest once the guard line lives somewhere else — the Dock is the
plausible home, since it already carries DHT nodes — and that is a larger change
than this feature earns.

Accessibility is the rest of the cost: `aria-expanded` on the toggle, and view
names surviving as accessible labels once the text is gone. A `title` tooltip is
not an accessible name.

#### User-configurable columns

**Wave 1, immediately after sort and filter
([#57](https://github.com/adamgreenwell/flume/issues/57)).**

Sorting exists today as three chips in the toolbar — activity, added, size —
and is not persisted at all. #57 owns the sort itself: every column, both
directions, stable against a list that updates every second, remembered across
restarts. Land it first. The hard part there is stability under a 1 Hz tick,
not the click target, and building resize handles against a sort model that is
still moving means building them twice. When the column model lands, sorting
moves from the toolbar chips into the headers; two ways to sort at once is worse
than either one.

The column model is the rest: which columns exist, which are shown, in what
order, at what width, and what happens when they no longer fit. The user
chooses all four, which is also what makes horizontal overflow necessary rather
than defensive — a user who can add columns will exceed the pane.

**A configurable layout has to survive its own upgrade.** This is the part that
is easy to leave out and expensive to add back. A layout saved by 1.1 will be
read by 1.2, which has a column 1.1 never heard of. So the layout is reconciled
against the column registry on load: unknown ids dropped, missing ids inserted
at their default position, at least one column always visible, and a minimum
width per column so a drag cannot produce a three-pixel column. Without that
step, adding a column in a later release silently hides it from everyone who
ever touched the picker.

The registry lives on both sides, like every other structure that crosses the
boundary: a `ColumnId` enum in Rust with its TypeScript mirror. An opaque blob
in `Settings` would be smaller and would give up the single source of truth the
settings module was built to keep.

Three more things follow from the structure of the list:

- **Widths are currently duplicated on purpose.** `ColumnHeader` and
  `TorrentRow` each hard-code the same figures, and `ColumnHeader`'s doc comment
  defends the duplication as two layout facts that happen to agree. Configurable
  widths end that: one source, read by both. The comment and the note in
  `CLAUDE.md` change in the same commit as the code.
- **Overflow restructures the list.** Header and rows scroll as one container —
  a header that does not track its rows is worse than no overflow — and
  `ExpandedRow`, a full-width sibling of the rows, has to span the scrolled
  width rather than the viewport. No pinned columns in the first version:
  pinning fights the selection stripe and is its own feature.
- **Widths belong in CSS custom properties on the container**, not in inline
  styles on every row. A drag then repaints without re-rendering every row
  against a 1 Hz tick.

Accessibility is a real share of the cost and not optional: `aria-sort` on the
sorted header, `aria-colcount` and `aria-colindex` once the set is dynamic, a
resize handle that answers to arrow keys, and a reordering path that is not
drag-only. The picker is the natural place for the keyboard path — move up,
move down — and right-clicking the header is the conventional way to reach it,
reusing the `ContextMenu` primitive that already exists.

The state dot and the name are not hideable. Everything else is.

#### Add a magnet that will not resolve

**Wave 1. The cost is in the list, not in the retry.**

**The deadline drops from sixty seconds to ten**, because the deadline's job
changes. Sixty seconds was right when expiry was the end of the road; with an
offer to add it anyway, the deadline exists to reach that choice quickly. Ten
seconds is roughly what a well-seeded magnet takes on a bootstrapped DHT, which
is the same measurement that justified sixty for a cold one — so **the clock
starts when the DHT is ready, not when the user presses add.** Otherwise the
first magnet of every session expires on the bootstrap rather than on the swarm,
and the fallback stops being a fallback. The engine already knows readiness:
`DHT_READY_NODE_THRESHOLD` and `EngineHealth`. The live-DHT test in
`tests/engine.rs` previews immediately after `Engine::start`, so it has to wait
for readiness too, or it becomes a coin toss.

**librqbit v9 cannot hold a torrent whose metadata has not arrived.**
`Session::add_torrent` resolves the magnet _before_ the managed torrent exists —
the `metadata: None` arm awaits `resolve_magnet` — and it does that whether or
not `paused` is set. So "add it to the list and let it resolve there" is not a
state that can be asked for. This is worth knowing before anyone reaches for
`AddTorrentOptions`.

What is available instead is a pending list in the record store above: the
magnet held by info hash, its display name taken from the link's `dn` and marked
provisional, retried in the background with backoff, and rendered as a row that
has no session id behind it. **It rides the telemetry snapshot as its own
array**, not merged into `torrents` — merging would make every field on
`TorrentSummary` either a lie or an `Option` for a row with no session.

**When it lands, it holds for file selection** — the row becomes "file list
ready, choose files" and a notification fires, using the path
[#13](https://github.com/adamgreenwell/flume/issues/13) already built.
`Session::update_only_files` is public, so the confirmed add flow does not have
to bend to accommodate any of this. A magnet nobody is seeding must read as
"still looking, no peers yet" — never as an error, never as progress.

**Retrying a magnet is network egress, and the guard governs it.** Resolution
runs through the librqbit session, which does not exist while the guard holds,
so it cannot leak — but the retry loop has to observe the hold and stand down
rather than spin on failures, and pending rows read as held like everything
else. That is architecture rule 12 applying to a new place that can get it
wrong.

The real cost is that every list operation has to handle a row with no id
behind it: pause, remove, open detail, the context menu, keyboard selection.
The retry loop is the easy half.

#### Per-torrent speed limits

**Deferred until librqbit answers. Not scheduled.**

Investigated and recorded in
[#49](https://github.com/adamgreenwell/flume/issues/49); re-verified against
librqbit 9.0.0 for this roadmap, with one fact #49 does not carry.

**The enforcement path already exists and is already per-torrent.** A peer's
download requests await the torrent's own limiter and then the session's, and
uploads do the same, so the effective rate is the lower of the two — which also
means a per-torrent limit can never raise a torrent above the global cap, and
the control will have to say so. Nothing has to be built to make a per-torrent
cap work. What is missing is a handle: `TorrentStateLive.ratelimits` is a
private field with no accessor. `AddTorrentOptions.ratelimits` is public, so a
limit can be set when a torrent is added — but `SerializedTorrent` persists only
the info hash, trackers, output folder, file selection and paused bit, so the
value does not survive a restart. A control that cannot be changed and silently
resets is worse than an absent one, which is why the add-time-only version is
not worth shipping while waiting.

Flume already runs a patched librqbit for exactly this shape of problem. Both
existing patches are small and gated — one behind an opt-in flag, one behind a
fallback — and the first went upstream as
[ikatson/rqbit#644](https://github.com/ikatson/rqbit/pull/644). A third,
exposing the live torrent's `Limits` the way `Session.ratelimits` is already
exposed, would be the smallest of the three and adds no new type.

**The decision is to ask upstream rather than to carry it.** Two local patches
is a maintenance position; three starts to be a fork. So the ask goes upstream
and this feature waits for the answer — which means someone has to file it, and
until they do, "waiting on upstream" is not true.

When it does land, the work is a limit per info hash in the record store,
applied on change and re-applied at launch. One caveat to record now rather than
discover later: librqbit restores _and starts_ persisted torrents inside
`Session::new_with_opts`, so a per-torrent cap is not in force until Flume
re-applies it immediately afterwards. The window is short but real — it is the
same window the egress guard exists to close, and the reason the guard's first
tick runs before any engine exists.

### What has to change before any of it

Two things in the current code would make all of this harder than it needs to
be, and both are cheap now:

**Settings are written by whole-object read-modify-write from the frontend.**
`src/app/page.tsx` reads the settings, spreads them, and writes them back to
change one field. That is safe with one writer. The rail state and a column
layout are two more writers, one of which fires from a drag, so two writes in
flight will clobber each other. A merge-in-Rust patch command replaces it, and
column drags persist on drag end rather than per pointer move.

**New UI preferences reach the wire format.** `SettingKey` already carries
`UiTheme` and `UiDensity`, so the rail state and the column layout each need a
deliberate yes or no — and a yes moves `collector/schema.json` and
`tests/usage_contract.rs` in the same commit, as architecture rule 11 requires.

### Order of work

1. The settings write path — half a day, and everything below writes settings.
2. Collapsible sidebar — ships on its own, depends on nothing.
3. The upstream ask for a per-torrent `Limits` accessor — filed early because
   the clock is not Flume's.
4. The per-torrent record store, with `added_at` — the keystone.
5. Sort and filter ([#57](https://github.com/adamgreenwell/flume/issues/57)),
   now with a real "added" date to sort on.
6. The column model on top of it.
7. Pending magnets.

Per-torrent limits join the order if and when upstream answers.

## Known limitations

- **Windows and Linux unverified locally.** Developed on macOS 27. CI builds
  for all platforms, but first-run behaviour needs a manual pass.
- **Polling, not events.** The Phase 0 status hook polls at 1 Hz. Fine for one
  status card, wrong for a torrent list.

- **Magnet association is untested on macOS and Windows.** The OS registration
  lives in the installed bundle, so it cannot be exercised by `tauri dev` —
  see [[Platform-Notes]]. Blocked behind the release pipeline
  ([#15](https://github.com/adamgreenwell/flume/issues/15)), which is itself
  blocked by [#22](https://github.com/adamgreenwell/flume/issues/22).

## Out of scope

Flume deliberately does not do cryptocurrency, chat, RSS automation, or paid
features. It also will not ship a second CLI — `rqbit` already exists.
