# CLAUDE.md — Flume

Living project memory. Update this in the same commit as the decision it
records, not afterwards.

> **Next.js version caveat:** this project uses Next.js 16, which differs from
> older training data. Read `node_modules/next/dist/docs/` before writing
> Next-specific code. (Next's own agent-file generator is disabled via
> `agentRules: false` so it does not overwrite this file.)

## What Flume is

A cross-platform BitTorrent client — Tauri v2 shell, Next.js static-export
frontend, `librqbit` v9 engine. Tagline: *"A beautiful, cross-platform
BitTorrent client."* Primary use case: downloading Linux distribution ISOs.

Not to be confused with the `flume` crate (an MPMC channel library) or
flume.dev (a React node editor). Both verified unrelated.

## Confirmed decisions

| Decision | Value | Rationale |
| --- | --- | --- |
| License | Apache-2.0 | Matches librqbit; express patent grant matters for P2P |
| Repo visibility | Private for now | Built open-source-ready; flip when ready |
| Git workflow | Feature branch → PR → Claude self-merges when green | PR CI path exercised; readable diff per change |
| Commit style | Conventional Commits, small and frequent | History is a deliverable, not bookkeeping |
| Tracking | GitHub Project #8 + issues per roadmap item | Transparent audit trail before going public |
| Bundle ID | `io.github.adamgreenwell.flume` | `dev.flume.*` would claim an unrelated project's domain |
| TLS | `librqbit` with `default-features = false`, `rust-tls` | No OpenSSL in the lockfile → no `libssl` runtime dep on Linux |
| Fonts | System font stack, not `next/font/google` | Avoids build-time network fetch; native feel per OS |

## Architecture rules (violating one is a design defect)

1. No torrent binary data over IPC. librqbit writes pieces to disk; the UI gets JSON only.
2. `src-tauri/src/engine/` imports **no Tauri types** and is testable under plain `cargo test`.
3. Command handlers are thin. Logic worth testing lives in the engine.
4. Static export only — no `app/api/*`, no middleware, no server-only functions.
5. Telemetry throttled to ~1 Hz and batched. No per-piece events.
6. Rust `serde` structs and their TypeScript mirrors (`src/lib/ipc/types.ts`) change together.
7. Minimal Tauri permissions. No shell plugin.

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
  tests/              integration tests against a real Session
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

**Phase 1 next** — see `docs/Roadmap.md` and the project board.

## Known gaps

- Windows and Linux are unverified locally (developed on macOS 27). CI covers
  build, but first-run behaviour on those platforms needs a manual pass.
- The Phase 0 UI polls at 1 Hz; Phase 1 should replace this with backend-pushed
  events before the torrent count grows.
