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

Phase 0 is complete. The application starts, embeds a real librqbit session,
bootstraps the DHT, and reports live status to the UI over Tauri IPC.

Torrent management arrives in Phase 1. See [[Roadmap]].

## Where to go next

| Page                     | What it covers                                       |
| ------------------------ | ---------------------------------------------------- |
| [[Getting-Started]]      | Installing and first run                             |
| [[User-Guide]]           | Adding torrents, file selection, settings            |
| [[Architecture]]         | System design, IPC contract, data flow               |
| [[Design-System]]        | Tokens, type, controls, accessibility floors         |
| [[Torrent-Engine-Notes]] | librqbit v9 integration surface and upgrade guidance |
| [[Platform-Notes]]       | Per-OS packaging, signing, WebView quirks            |
| [[Development-Setup]]    | Toolchain, dev loop, debugging, testing              |
| [[CI-CD-and-Releases]]   | Pipeline and release process                         |
| [[Roadmap]]              | Phases, backlog, known limitations                   |

## Name

Unrelated to the [`flume` crate](https://crates.io/crates/flume) (an MPMC
channel library) or [flume.dev](https://flume.dev) (a React node editor). The
name refers to a water channel — a flume carries things downstream.
