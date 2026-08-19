<div align="center">

# Flume

**A beautiful, cross-platform BitTorrent client.**

[![CI](https://github.com/adamgreenwell/flume/actions/workflows/ci.yml/badge.svg)](https://github.com/adamgreenwell/flume/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![librqbit](https://img.shields.io/badge/librqbit-9.0.0-5319E7)](https://crates.io/crates/librqbit)

</div>

---

Flume is a BitTorrent client built for people who want a torrent client that is
genuinely pleasant to use. It pairs a modern, themeable interface with
[librqbit][librqbit] — a production-grade Rust torrent engine — inside a
[Tauri][tauri] shell, so the whole app ships as a small native binary rather
than a bundled browser.

It is built around a specific use case: **downloading Linux distribution ISOs**
(Ubuntu, Debian, Fedora, Rocky, AlmaLinux) for development and server testing.

> [!NOTE]
> Flume is in early development. Phase 0 (scaffold and engine integration) is
> complete; torrent management lands in Phase 1. See the [Roadmap](#roadmap).

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

> Builds are not yet published. Until Phase 3 ships the release pipeline, build
> from source with the [development setup](#development) below.

| Platform                              | Format                |
| ------------------------------------- | --------------------- |
| macOS 12+                             | `.dmg`                |
| Windows 10/11                         | `.msi`, `.exe` (NSIS) |
| Debian 12+ / Ubuntu 22.04+            | `.deb`                |
| Fedora 38+ / RHEL 9.4+ / Rocky / Alma | `.rpm`                |

## Development

**Prerequisites:** [Rust][rust] (stable, 1.88+), Node.js 22+, and your
platform's [Tauri system dependencies][tauri-prereqs].

```bash
git clone https://github.com/adamgreenwell/flume.git
cd flume
npm install
npm run tauri:dev
```

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

Full detail, including the IPC contract, lives in the [wiki][wiki].

## Roadmap

| Phase | Focus                                                          | Status      |
| ----- | -------------------------------------------------------------- | ----------- |
| 0     | Scaffold, engine integration, CI, docs                         | ✅ Complete |
| 1     | Core torrent lifecycle (add, control, persist, file selection) | 🚧 Next     |
| 2     | Polish: themes, detail views, deep links, tray, notifications  | Planned     |
| 3     | Hardening: streaming, torrent v2, release pipeline, signing    | Planned     |

Tracked on the [project board][board] and in [issues][issues].

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) for the
development workflow, coding standards, and commit conventions.

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
[board]: https://github.com/users/adamgreenwell/projects/8
[issues]: https://github.com/adamgreenwell/flume/issues
