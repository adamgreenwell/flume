<div align="center">

# Flume

**A beautiful, cross-platform BitTorrent client.**

[![CI](https://github.com/adamgreenwell/flume/actions/workflows/ci.yml/badge.svg)](https://github.com/adamgreenwell/flume/actions/workflows/ci.yml)
[![Website](https://img.shields.io/badge/website-flume.adamgreenwell.com-5ab8ea)](https://flume.adamgreenwell.com)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![librqbit](<https://img.shields.io/badge/librqbit-9.0.1%20(patched)-5319E7>)](https://github.com/adamgreenwell/rqbit/tree/peer-availability)

</div>

---

Flume is a BitTorrent client built for people who want a torrent client that is
genuinely pleasant to use. It pairs a modern, themeable interface with
[librqbit][librqbit] — a production-grade Rust torrent engine — inside a
[Tauri][tauri] shell, so the whole app ships as a small native binary rather
than a bundled browser.

It is a **general-purpose** BitTorrent client, and deliberately only that: no
cryptocurrency, no chat, no RSS automation, no bundled search, and no paid
tier.

> [!NOTE]
> Flume is approaching 1.0. The core torrent lifecycle, the polish pass, and
> the release pipeline are all built; what is left is signing Phase 1 off
> against real torrents on each platform. See the [Roadmap](#roadmap).

## Why another torrent client?

Most torrent clients are either powerful and unpleasant, or pretty and
crippled. Flume's bet is that the engine is a solved problem — librqbit already
handles DHT, peer protocol, and piece management well — so all the effort can go
into the part that is not solved: **the experience**.

- **Actually cross-platform.** macOS, Windows, and both Debian- and RHEL-family
  Linux are first-class targets, not afterthoughts.
- **Small and fast.** A native WebView instead of a bundled Chromium.
- **Safe by construction.** Torrent data is written to disk by the engine and
  never crosses the UI boundary. See [Architecture](#architecture).

## Installation

> No release has been tagged yet, so there is nothing to download. The pipeline
> that builds these is in place and runs on a `v*` tag; until then, build from
> source with the [development setup](#development) below.

| Platform                              | Format                |
| ------------------------------------- | --------------------- |
| macOS 12+                             | `.dmg`                |
| Windows 10/11                         | `.msi`, `.exe` (NSIS) |
| Debian 12+ / Ubuntu 22.04+            | `.deb`                |
| Fedora 38+ / RHEL 9.4+ / Rocky / Alma | `.rpm`                |

## Development

**Prerequisites:** [Rust][rust] (stable, 1.88+), Node.js 26 (see `.nvmrc`), and your
platform's [Tauri system dependencies][tauri-prereqs].

```bash
git clone https://github.com/adamgreenwell/flume.git
cd flume
npm install
npm run tauri:dev
```

### A note on the librqbit dependency

**Your first build fetches librqbit from a fork, not crates.io.** That is
deliberate, and `src-tauri/Cargo.toml` says so at the point it happens:

```toml
[patch.crates-io]
librqbit = { git = "https://github.com/adamgreenwell/rqbit.git", rev = "..." }
```

Upstream librqbit tracks each peer's bitfield for piece picking but does not
expose it, so an embedder cannot compute what the _swarm_ holds — which is what
answers "will this download finish?" rather than merely "is it moving?". The
fork adds two opt-in fields and three re-exports; nothing is computed unless
asked for, so existing librqbit callers are unaffected.

It is pinned by **full commit SHA** in `Cargo.lock`, so a force-push to the fork
cannot change what you build. The same change is with upstream as
[`ikatson/rqbit#644`][rqbit-pr]; when it lands in a crates.io release the
`[patch.crates-io]` section is deleted and this note goes with it.

The wiki's [Torrent Engine Notes][engine-notes] has the full reasoning, including
why a per-peer _count_ is not sufficient.

[rqbit-pr]: https://github.com/ikatson/rqbit/pull/644
[engine-notes]: https://github.com/adamgreenwell/flume/wiki/Torrent-Engine-Notes

Useful scripts:

| Command                   | What it does                                         |
| ------------------------- | ---------------------------------------------------- |
| `npm run tauri:dev`       | Run the desktop app with hot reload                  |
| `npm run check`           | Typecheck, lint, format-check, and test the frontend |
| `npm run test`            | Frontend tests (Vitest)                              |
| `cargo test`              | Backend tests — run from `src-tauri/`                |
| `cargo test -- --ignored` | Also run network-dependent DHT tests                 |

Before committing, the same gate CI runs:

```bash
npm run check && cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## Architecture

The webview never touches torrent binary data. librqbit writes pieces straight
to disk; the UI only ever receives small JSON payloads.

```text
┌──────────── Tauri v2 WebView (Next.js static export) ─────────────┐
│  Themeable React/Tailwind SPA  ⇄  @tauri-apps/api: invoke/listen  │
└───────────────────┬──────────────────────────▲────────────────────┘
                    │ commands (JSON only)     │ events (JSON, ~1 Hz)
                    ▼                          │
┌───────────────────┴──────────────────────────┴────────────────────┐
│  Rust core (tokio)                                                │
│    engine/    thin librqbit wrapper — no Tauri types, unit tested  │
│    commands/  #[tauri::command] handlers — thin, no business logic │
│    state/     app state and persistence                           │
└───────────────────────────────────────────────────────────────────┘
```

Full detail, including the IPC contract, lives in the [documentation][docs].

## Roadmap

| Phase | Focus                                                          | Status                      |
| ----- | -------------------------------------------------------------- | --------------------------- |
| 0     | Scaffold, engine integration, CI, docs                         | ✅ Complete                 |
| 1     | Core torrent lifecycle (add, control, persist, file selection) | ✅ Built, awaiting sign-off |
| 2     | Polish: themes, detail views, deep links, tray, notifications  | ✅ Complete                 |
| 3     | Hardening: release pipeline, signing, performance              | 🚧 Mostly complete          |

Phase 1 is implemented but not signed off: adding a real ISO by magnet and by
`.torrent`, resuming mid-download across a relaunch, and Windows seeding
([#48](https://github.com/adamgreenwell/flume/issues/48)) all want a run on a
real torrent rather than a green CI badge.

Tracked on the [project board][board] and in [issues][issues].

## Documentation

The full documentation is at **[flume.adamgreenwell.com/docs][docs]**, and the
same pages are mirrored into the [GitHub Wiki][wiki]. Both are generated from
[`docs/`](docs) in this repository — edit there, not in the wiki, so changes are
reviewed alongside the code.

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) for the
development workflow, coding standards, and commit conventions, and
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for the standard everyone is held to.

Found a security issue? Please do not open a public issue — [SECURITY.md](SECURITY.md)
explains how to report it privately.

## License

[Apache-2.0](LICENSE) © 2026 Adam Greenwell.

Flume embeds [librqbit][librqbit], also Apache-2.0. See [NOTICE](NOTICE).

> **Name note:** unrelated to the [`flume`](https://crates.io/crates/flume)
> crate (an MPMC channel library) or [flume.dev](https://flume.dev) (a React
> node editor).

[librqbit]: https://github.com/ikatson/rqbit
[tauri]: https://tauri.app
[rust]: https://rustup.rs
[tauri-prereqs]: https://tauri.app/start/prerequisites/
[wiki]: https://github.com/adamgreenwell/flume/wiki
[docs]: https://flume.adamgreenwell.com/docs/
[board]: https://github.com/users/adamgreenwell/projects/8
[issues]: https://github.com/adamgreenwell/flume/issues
