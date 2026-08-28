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
BitTorrent client."_ Primary use case: downloading Linux distribution ISOs.

Not to be confused with the `flume` crate (an MPMC channel library) or
flume.dev (a React node editor). Both verified unrelated.

## Confirmed decisions

| Decision           | Value                                                  | Rationale                                                                                                                         |
| ------------------ | ------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------- |
| License            | Apache-2.0                                             | Matches librqbit; express patent grant matters for P2P                                                                            |
| Repo visibility    | Private for now                                        | Built open-source-ready; flip when ready                                                                                          |
| Git workflow       | Feature branch → PR → Claude self-merges when green    | PR CI path exercised; readable diff per change                                                                                    |
| Commit style       | Conventional Commits, small and frequent               | History is a deliverable, not bookkeeping                                                                                         |
| Tracking           | GitHub Project #8 + issues per roadmap item            | Transparent audit trail before going public                                                                                       |
| Bundle ID          | `io.github.adamgreenwell.flume`                        | `dev.flume.*` would claim an unrelated project's domain                                                                           |
| TLS                | `librqbit` with `default-features = false`, `rust-tls` | No OpenSSL in the lockfile → no `libssl` runtime dep on Linux                                                                     |
| Fonts              | Instrument Sans + IBM Plex Mono, vendored as woff2     | Still no build-time or runtime fetch; the type ramp is measured against these metrics, and the app must render offline            |
| MSRV               | Rust 1.88                                              | A lower MSRV silently blocks security updates — cargo won't select deps needing more                                              |
| Node (CI + dev)    | 26, matching `@types/node`                             | Build tooling only, never shipped; matching the dev env beats tracking LTS and avoids CI drift                                    |
| TypeScript         | Stay on 5.x                                            | TS 7 (native Go compiler) breaks `typescript-eslint`, so `npm run lint` cannot run — Dependabot ignores the major; revisit in #28 |
| Add flow (Phase 1) | File picker first, then start                          | ISO torrents bundle several images; avoid downloading the wrong 4 GB                                                              |
| Layout (Phase 1)   | Single list + detail panel                             | Focused; scales fine for a personal workload                                                                                      |
| Settings store     | `tauri-plugin-store` (JSON)                            | Sufficient for a flat settings object; no SQLite migration burden                                                                 |

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

## Piece availability, and the librqbit fork

**Flume runs a patched librqbit.** `src-tauri/Cargo.toml` carries a
`[patch.crates-io]` entry pointing at `adamgreenwell/rqbit`, pinned to a rev.
Delete it the moment the change lands upstream in a crates.io release.

The patch adds three things, all in `PeerStats`:

- `have_pieces` — how many pieces a peer holds, clamped to `total_pieces`.
- `have_bitfield` — the bitfield itself, opt-in via
  `PeerStatsFilter::include_bitfield`, off by default.
- public re-exports of `PeerStats` and `PeerStatsFilter`, which were a return
  type and a parameter of a public method while living in a private module.

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
src/                  Next.js app (static export → out/)
  app/                routes; all client components
  components/         presentational React components
  hooks/              stateful logic (useCoreStatus)
  lib/ipc/            typed invoke wrappers + TS mirrors of Rust types
  lib/format.ts       pure display helpers
src-tauri/
  src/engine/         librqbit wrapper — no Tauri types
  src/commands/       #[tauri::command] handlers
  src/state/          shared app state
  tests/engine.rs     integration tests against a real librqbit Session
  tests/commands.rs   IPC-layer tests via Tauri's mock runtime
docs/                 wiki source, mirrored to the GitHub Wiki
```

## Quality gate

```bash
npm run check
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Network-dependent tests are `#[ignore]`d: `cargo test -- --ignored`.

## Status

**Phase 0 complete.** Engine integration verified end to end: the running app
bootstraps the DHT to 100+ nodes and persists state correctly.

**Phase 1 next** — see `docs/Phase-1-Plan.md` for the build order and confirmed
product decisions, plus `docs/Roadmap.md` and the project board.

## Known gaps

- Windows and Linux are unverified locally (developed on macOS 27). CI covers
  build, but first-run behaviour on those platforms needs a manual pass.
- The Phase 0 UI polls at 1 Hz; Phase 1 should replace this with backend-pushed
  events before the torrent count grows.
