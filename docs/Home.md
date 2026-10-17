# Flume

**A beautiful, cross-platform BitTorrent client.**

Flume pairs a modern, themeable interface with [librqbit](https://github.com/ikatson/rqbit) —
a production-grade Rust BitTorrent engine — inside a [Tauri v2](https://tauri.app)
shell. The result is a small native binary rather than a bundled browser.

It is a general-purpose BitTorrent client, and deliberately only that: no
cryptocurrency, no chat, no RSS automation, no bundled search, and no paid
tier.

## Why Flume exists

The BitTorrent protocol is a solved problem. librqbit already handles DHT, the
peer wire protocol, and piece management well. What is _not_ solved is the
experience: most clients are either powerful and unpleasant, or attractive and
crippled.

Flume's bet is that by embedding a mature engine rather than reimplementing one,
all the effort can go into the interface.

## Status

Phases 0 and 1 are complete, and 1.4.0 has shipped. Torrents can be added by
magnet or file with a select-files-first flow, paused, resumed and removed;
state survives a restart without re-hashing. On top of that sits the design
work Phase 1 did not ask for — swarm health, the limiting-factor panel, the
availability histogram — plus seed and queue limits, an opt-in tunnel guard,
and per-torrent peer limits and IP filtering.

What is not established is a line-by-line pass of the smoke-test checklist on
every platform. The 1.2.0 release was installed and exercised on Windows and
Linux and worked, which is a weaker claim than every item being checked — and
it is 1.2.0 rather than the current release, so treat it as evidence that the
app runs on those platforms rather than as validation of what shipped last.

See [[Roadmap]] for what follows, and [[Deliberate-Omissions]] for what will
not.

## Where to go next

| Page                     | What it covers                                          |
| ------------------------ | ------------------------------------------------------- |
| [[Getting-Started]]      | Installing and first run                                |
| [[User-Guide]]           | Adding torrents, file selection, settings               |
| [[Architecture]]         | System design, IPC contract, data flow                  |
| [[Design-System]]        | Tokens, type, controls, accessibility floors            |
| [[Privacy]]              | What is collected, what never is, and how to say no     |
| [[Torrent-Engine-Notes]] | librqbit v9 integration surface and upgrade guidance    |
| [[Platform-Notes]]       | Per-OS packaging, signing, WebView quirks               |
| [[Development-Setup]]    | Toolchain, dev loop, debugging, testing                 |
| [[CI-CD-and-Releases]]   | Pipeline and release process                            |
| [[Roadmap]]              | Phases, backlog, known limitations                      |
| [[Deliberate-Omissions]] | What Flume will not build, and which kind of no each is |

## Name

Unrelated to the [`flume` crate](https://crates.io/crates/flume) (an MPMC
channel library) or [flume.dev](https://flume.dev) (a React node editor). The
name refers to a water channel — a flume carries things downstream.
