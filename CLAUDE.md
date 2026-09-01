# CLAUDE.md — Flume

Living project memory. Update this in the same commit as the decision it
records, not afterwards.

> **Next.js version caveat:** this project uses Next.js 16, which differs from
> older training data. Read `node_modules/next/dist/docs/` before writing
> Next-specific code. (Next's own agent-file generator is disabled via
> `agentRules: false` so it does not overwrite this file.)

## What Flume is

A cross-platform BitTorrent client — Tauri v2 shell, Next.js static-export
frontend, `librqbit` v9 engine. Tagline: _"A beautiful, cross-platform
BitTorrent client."_

**General-purpose — do not give it a use case.** This used to read "primary use
case: downloading Linux distribution ISOs". That is the standard wink and it is
not true; a general-purpose client is general-purpose. Do not reintroduce it,
and do not correct it in the other direction either — acknowledging what people
actually torrent is inducement, which is the one framing that carries real
liability for a tool like this. Describe what the client does and leave what
people download with it to them.

Concrete examples are fine and are not the same thing: "a multi-gigabyte ISO"
illustrating memory bounds, or a distro torrent explaining why file selection
matters, are illustrations rather than claims about purpose.

Not to be confused with the `flume` crate (an MPMC channel library) or
flume.dev (a React node editor). Both verified unrelated.

## Confirmed decisions

| Decision           | Value                                                  | Rationale                                                                                                                                                                                                                            |
| ------------------ | ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| License            | Apache-2.0                                             | Matches librqbit; express patent grant matters for P2P                                                                                                                                                                               |
| Repo visibility    | Private for now                                        | Built open-source-ready; flip when ready                                                                                                                                                                                             |
| Git workflow       | Feature branch → PR → Claude self-merges when green    | PR CI path exercised; readable diff per change                                                                                                                                                                                       |
| Commit style       | Conventional Commits, small and frequent               | History is a deliverable, not bookkeeping                                                                                                                                                                                            |
| Tracking           | GitHub Project #8 + issues per roadmap item            | Transparent audit trail before going public                                                                                                                                                                                          |
| Bundle ID          | `io.github.adamgreenwell.flume`                        | `dev.flume.*` would claim an unrelated project's domain                                                                                                                                                                              |
| TLS                | `librqbit` with `default-features = false`, `rust-tls` | No OpenSSL in the lockfile → no `libssl` runtime dep on Linux                                                                                                                                                                        |
| Fonts              | Instrument Sans + IBM Plex Mono, vendored as woff2     | Still no build-time or runtime fetch; the type ramp is measured against these metrics, and the app must render offline                                                                                                               |
| MSRV               | Rust 1.88                                              | A lower MSRV silently blocks security updates — cargo won't select deps needing more                                                                                                                                                 |
| Rust toolchain     | Pinned in `rust-toolchain.toml` (1.98.0)               | CI ran `@stable` in seven places while dev machines ran whatever `rustup update` last fetched; #134 passed clippy locally on 1.94 and failed CI on 1.98. Not the MSRV — that is `rust-version` in Cargo.toml and nothing verifies it |
| Node (CI + dev)    | 26, pinned in `.nvmrc`                                 | Build tooling only, never shipped; matches `@types/node`. `.nvmrc` is the single source — `engines`, CI and `nvm use` all read it                                                                                                    |
| TypeScript         | Stay on 5.x                                            | TS 7 is outside `typescript-eslint`'s peer range (`>=4.8.4 <6.1.0`), so `npm run lint` cannot run. Ignored in `dependabot.yml`; revisit in #28                                                                                       |
| ESLint             | Stay on 9.x                                            | ESLint 10 removed `context.getFilename()`; the `eslint-plugin-react` inside `eslint-config-next` still calls it. Ignored in `dependabot.yml`                                                                                         |
| Add flow (Phase 1) | File picker first, then start                          | ISO torrents bundle several images; avoid downloading the wrong 4 GB                                                                                                                                                                 |
| Layout (Phase 1)   | Single list + detail panel                             | Focused; scales fine for a personal workload                                                                                                                                                                                         |
| Windows signing    | Not signed, deliberately                               | SmartScreen reputation accrues per certificate from downloads, so an OV cert changes nothing at this volume; EV is hundreds a year for a free app. Keys must also be on hardware since 2023, so the macOS approach does not transfer |
| Settings store     | `tauri-plugin-store` (JSON)                            | Sufficient for a flat settings object; no SQLite migration burden                                                                                                                                                                    |
| Usage reporting    | Opt-in, asked once at first run                        | Opt-out telemetry in a BitTorrent client becomes the top comment in every thread about it, permanently. qBittorrent ships none; that is the baseline. Opt-in costs sample rate, opt-out costs the audience                           |
| Usage collector    | Cloudflare Worker + D1, in `collector/`                | The Worker validates against the wire format, so it must move in the same commit as the Rust enum — a schema split across two repos drifts                                                                                           |
| Tunnel guard       | Stops the engine; never pauses torrents                | `Session::pause` writes `is_paused` to `session.json` synchronously and librqbit stores one paused bit with no reason, so a guard pause is indistinguishable from the user's and quitting while held would strand the library        |
| Guard hysteresis   | Hold on the first failing probe, release after 10 s    | Asymmetric on purpose: protection is never delayed, only recovery. A laptop waking or a VPN reconnecting resolves in seconds, and releasing into one costs a re-announce to every tracker plus a DHT announce, per torrent           |

## Architecture rules (violating one is a design defect)

1. No torrent binary data over IPC. librqbit writes pieces to disk; the UI gets JSON only.
2. `src-tauri/src/engine/` imports **no Tauri types** and is testable under plain `cargo test`.
3. Command handlers are thin. Logic worth testing lives in the engine.
4. Static export only — no `app/api/*`, no middleware, no server-only functions.
5. Telemetry throttled to ~1 Hz and batched. No per-piece events.
6. Rust `serde` structs and their TypeScript mirrors (`src/lib/ipc/types.ts`) change together.
7. Minimal Tauri permissions. No shell plugin.
8. The UI vocabulary is the token set in `src/app/globals.css`. See below.
9. Verdicts the data cannot support are not invented. `SwarmHealth` reports
   `unknown` only when there are no peer bitfields to judge from, never as a
   guess between `thin` and `healthy` — see below.
10. **All network egress originates in Rust.** The webview's CSP is
    `connect-src 'self' ipc: http://ipc.localhost` and is not widened, ever.
    Adding an analytics or crash-reporting SDK to the frontend would mean
    relaxing it, which hands every piece of UI code — including anything that
    renders an attacker-controlled torrent name into the DOM — an egress path
    it does not currently have. One place talks to the outside world:
    `src-tauri/src/usage/sender.rs`.
11. **Nothing that identifies a download leaves the machine.** No info hashes,
    torrent or file names, tracker URLs, peer IPs, download paths or proxy
    URLs — in a usage event or a diagnostics bundle. This rules out free-text
    error strings, because librqbit's errors embed tracker URLs and paths, so
    reported failures are a closed enum (`usage::FailureKind`) rather than a
    message.
12. **The egress guard fails closed, everywhere.** `Verdict::Unknown` does not
    permit transfer; a settings file that exists and cannot be parsed forces
    `Hold`; the gate starts held rather than open; and the status published
    before the first probe says held. Each of those is a place where the
    obvious default is the unsafe one, and a guard that fails open in any of
    them is decoration. The one deliberate exception is a settings file that
    parses but fails validation, where the user's actual choice is known and is
    honoured.

## The egress guard holds by stopping the engine

Worth stating separately because the obvious implementation is wrong and looks
right.

Pausing every torrent when the tunnel drops cannot work: `Session::pause` writes
`is_paused` into `session.json` synchronously, and librqbit stores exactly one
paused bit with no reason attached. A guard pause is therefore indistinguishable
from a user pause, on disk and in memory, so quitting while held brings the
library back paused with the tunnel up — the exact stranding the feature exists
to prevent. Recovering from that needs Flume's own ledger, keyed by info hash
because ids are reused, persisted against a crash, and reconciled every launch.

Stopping the engine deletes that problem rather than solving it. The guard never
touches the torrents, so nothing it does reaches `session.json`, so whatever the
user paused stays paused and whatever was running comes back running. It is also
strictly stronger: pausing leaves the DHT, the TCP listener and the UPnP mapping
running, so a "paused" Flume still announces itself from the address being
protected.

It is what closes the launch window too. librqbit restores _and starts_ every
persisted torrent inside `Session::new_with_opts`, with no hook between
construction and the first tracker announce — so a session built before the
check has already transferred. The guard's first tick runs before any engine
exists.

## Telemetry is not usage reporting

Two different things, and the vocabulary is load-bearing:

- **`src-tauri/src/telemetry.rs`** is the 1 Hz push of torrent status to the
  webview. It never leaves the process. Architecture rule 5 is about this.
- **`src-tauri/src/usage/`** is opt-in anonymous counts sent to a collector.
  It is the only thing in Flume that sends anything anywhere, and only with
  consent.

Do not rename either into the other's territory. `docs/Architecture.md`,
`tests/performance.rs` and CONTRIBUTING all use "telemetry" in the first
sense.

`Settings.usage_reporting` is `Option<bool>` and the three states are the
point: `None` is _not yet asked_, `Some(false)` is a decline that must never
be re-asked. Collapsing it to a `bool` either nags someone who already said no
or treats silence as consent. `FirstRun` records an untouched toggle as an
explicit `Some(false)` on exit, so the value is never `None` after first run.

The install id is written lazily, on the first batch actually sent — consent
followed by a session that records nothing leaves no trace on disk.
`docs/Privacy.md` is the user-facing contract and is the thing this feature is
judged on; change it in the same commit as the schema.

## Piece availability, and the librqbit fork

**Flume runs a patched librqbit.** `src-tauri/Cargo.toml` carries a
`[patch.crates-io]` entry pointing at `adamgreenwell/rqbit`, pinned to a rev.
Delete it the moment the change lands upstream in a crates.io release.

**Do not delete the fork's `peer-availability` branch.** `Cargo.lock` pins the
full commit SHA, so a force-push cannot silently change what is built — but the
commit still has to remain reachable. Deleting the branch, or the fork, leaves
it eligible for garbage collection and every build and CI run breaks with a
fetch error. The fork must also stay public: CI clones it anonymously.

**A second patch fixes Windows file locking (#9).** `FilesystemStorage` opened
every file read _and_ write when `allow_overwrite` is set, including a complete
torrent that will only ever be served. On Windows a write open needs the holder
to have granted `FILE_SHARE_WRITE`, so another application holding a download
with `FILE_SHARE_READ` made the add fail outright. The patch retries read-only
on a sharing violation and logs the degraded mode; seeding only reads, so a
completed torrent is served normally. Unix is unaffected — an open handle does
not restrict other opens there.

The first patch adds three things, all in `PeerStats`:

- `have_bitfield` — the peer's bitfield, opt-in via
  `PeerStatsFilter::include_bitfield`, off by default.
- `have_pieces` — the count, behind the same flag. Gated too, so the default
  path computes nothing and existing librqbit callers pay nothing; that was the
  maintainer's stated condition.
- public re-exports of `PeerStats` and `PeerStatsFilter`, which were a return
  type and a parameter of a public method while living in a private module.

Sent upstream as `ikatson/rqbit#644`. Flume reads only `have_bitfield`.

**A per-peer count is not enough,** which is worth remembering before anyone
tries to shrink the patch back to it. A count gives the _mean_ copies per
piece; the verdict needs the _minimum_. Two peers holding 500 pieces each may
overlap completely or not at all — identical averages, and only one of those
torrents can finish. `availability::compute` therefore works from bitfields,
and `overlapping_peers_are_not_the_same_as_complementary_ones` pins the case.

**Bitfields are byte-padded and the padding is not trustworthy.** librqbit's
`on_bitfield` validates the byte length and stores the peer's bytes verbatim,
so a peer that sets the spare trailing bits would inflate any count taken over
the whole bitfield. Both the patch and `availability::compute` slice to
`total_pieces` first. librqbit does the same thing itself two lines below the
length check.

**The Thin/Healthy threshold scales with the peer count.** Healthy is
`rarest >= min(3, live_peers)`. The design says "every piece on ≥3 peers", but
taken literally that is unreachable below three peers — two peers that are both
seeds hold every piece twice over and would read Thin forever. Below three
peers the swarm is judged on coverage rather than punished for its size.

Rule 9 still holds where the data runs out: `rarest == 0` resolves to `None`
and no bitfields at all resolves to `Unknown`, which the UI renders as
"Connected" rather than as a verdict. Issue #79 tracks the history;
`ikatson/rqbit#643` is the upstream ask.

## Design tokens

`src/app/globals.css` holds the whole visual vocabulary. Rules that hold across
the frontend:

- **Never introduce a colour that is not a token.** Tailwind utilities are named
  after the tokens — `bg-bg-1`, `text-fg-2`, `border-line`, `bg-acc-deep`. There
  is no second set of friendlier names, deliberately: two vocabularies is how
  `--flume-line` and "gray-800" end up meaning the same thing to different
  people. A step that is genuinely missing is derived in OKLCH at fixed chroma
  and hue, never eyeballed between two existing values.
- **Sizes and rates are decimal** (GB, MB/s) because that is what disks and ISPs
  quote — `formatBytes` is 1000-based, three significant figures; `formatSpeed`
  is one decimal and holds MB/s down to 0.1 so a rate column reads as one unit.
  Piece length is the only binary figure, rendered MiB, because that is what
  the wire format uses.
- **`formatDuration` exists twice** — `src/lib/format.ts` and
  `src-tauri/src/engine/torrent.rs`. The engine decides what a torrent's
  `detail` line says, so it has to format the duration inside that sentence.
  Both render `1 h 07 min` / `2 min 30 s` / `45 s` and both sides assert the
  same four examples. Change one, change the other.
- **Every number is `flume-num`** (mono, tabular figures). Without it, columns
  jitter on every 1 Hz tick.
- `fg-3` is the floor for text and `line-2` the floor for control borders.
  `fg-dis` is for disabled controls only and must never carry text the user
  needs to read. `src/app/tokens.test.ts` enforces this and records the
  pairings that currently fall short.
- **Status is never colour alone** — a dot, a word, and a sentence. Download and
  upload are always labelled, never distinguished by colour alone.
- Icons are stroked SVG on a 16×16 grid at a constant 1.5px optical weight. No
  emoji, no icon font, nothing filled.
- Control heights are 28 / 30 / 34px; radii 4 / 6 / 9px. Do not round these to a
  framework scale — the spacing was chosen against the type ramp.

`npm run storybook` renders every primitive in both themes with axe running
beside it.

## librqbit v9 API notes

**v9.0.0 released 2026-08-15.** Verified against the actual crate source, not
v8 examples. The v8 → v9 reorganisation is real and v8-era snippets will not
compile.

- `SessionOptions.dht: Option<DhtSessionConfig>` — `None` disables DHT. There
  is no `disable_dht` boolean.
- `SessionOptions.listen: Option<ListenerOptions>` **defaults to `None`**,
  meaning no incoming peers and no seeding. Flume always sets it explicitly.
- UPnP is `ListenerOptions.enable_upnp_port_forwarding`, a runtime flag.
- **There are no cargo features for DHT, UPnP, or torrent v2** — all are
  always compiled in. Only TLS backend, `http-api*`, `postgres`, `prometheus`,
  and a few others are feature-gated.
- `DhtPersistenceConfig.config_filename: None` means a **global OS path**, not
  your session directory. Always set it explicitly — see issue #19.
- Useful accessors: `get_dht()`, `listen_addr()`, `announce_port()`,
  `stats_snapshot()`, `client_name_and_version()`, `stop()`.
- `Speed` is `{ mbps: f64 }` with `.as_bytes() -> u64`. Flume converts to raw
  bytes/sec at the boundary rather than exposing librqbit's type over IPC.

Source for reference: `~/.cargo/registry/src/index.crates.io-*/librqbit-9.0.0/`

## Layout

```
collector/            Cloudflare Worker + D1 for usage counts
src/                  Next.js app (static export → out/)
  app/                routes; all client components
  components/         presentational React components
  hooks/              stateful logic (useCoreStatus)
  lib/ipc/            typed invoke wrappers + TS mirrors of Rust types
  lib/format.ts       pure display helpers
src-tauri/
  src/engine/         librqbit wrapper — no Tauri types
  src/diagnostics/    redacted bundle builder — no Tauri types
  src/usage/          opt-in counts + sender — no Tauri types
  src/commands/       #[tauri::command] handlers
  src/state/          shared app state
  tests/engine.rs     integration tests against a real librqbit Session
  tests/commands.rs   IPC-layer tests via Tauri's mock runtime
  tests/usage_contract.rs  pins the wire format against collector/schema.json
docs/                 wiki source, mirrored to the GitHub Wiki
```

## Quality gate

```bash
npm run check
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Tests needing more than a checkout are `#[ignore]`d: `cargo test -- --ignored`.
They are not uniform, and a failure there is not automatically a regression:

- **DHT and magnet tests** need working internet.
- **`peer_connections_go_through_a_configured_proxy`** additionally needs a
  SOCKS5 proxy on `127.0.0.1:1080`. Without one it fails on a 120-second
  timeout, which looks alarming and means nothing. See the wiki's development
  setup page for the one-line command that starts one.
- **Timing tests** (`tests/performance.rs`, and the availability walk in
  `src/engine/availability.rs`) only mean anything under `--release`.

Each test's `#[ignore]` reason names its own prerequisite.

**`npm run check` refuses to run on the wrong Node**, because below 22 it would
not fail honestly: jsdom's copy of undici calls
`require('node:worker_threads').markAsUncloneable`, which does not exist yet,
and vitest reports that as "no tests" with a worker error rather than as a
version problem — and it passes on retry if a different `node` happens to come
first, so it reads as flaky. `scripts/check-node.mjs` turns that into one
sentence. `engines` plus `engine-strict` only gate `npm install`, which is not
where this bites.

## Status

**Phase 0 complete.** Engine integration verified end to end: the running app
bootstraps the DHT to 100+ nodes and persists state correctly.

**Phase 1 complete.** All nine build-order items are implemented and all seven
definition-of-done boxes are met, the last of them by manual use rather than by
code: both add routes verified, controls and settings exercised, a mid-download
kill resuming without a re-hash.

The design retrofit landed on top of it — swarm health, the limiting-factor
panel, the availability histogram — none of which Phase 1 asked for.

See `docs/Roadmap.md` and the project board for what follows.

## Known gaps

- Per-platform coverage is uneven. Development is on macOS 27; the add flow —
  both routes, including clipboard magnet detection — has been exercised on
  macOS, Windows and Linux, and Windows additionally through the file-locking
  work in #9. What has not happened is a deliberate walk of the smoke checklist
  in `docs/Platform-Notes.md` on each, so the gaps that remain are the ones
  nobody has gone looking for.
- Telemetry is pushed, not polled (`src-tauri/src/telemetry.rs` emits, the
  frontend `listen()`s), but the snapshot is still _computed_ every second
  whether or not anyone is looking — including while the window is hidden.
  Availability now skips torrents that are not downloading, so a mostly-seeding
  library is cheap; a library of many concurrent downloads is not. See
  `analyse_stays_within_its_share_of_a_telemetry_tick` for the per-torrent cost.
