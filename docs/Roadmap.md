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

- Add via magnet link, with clipboard detection ([#3](https://github.com/adamgreenwell/flume/issues/3))
- Add via `.torrent` file picker and drag-and-drop ([#4](https://github.com/adamgreenwell/flume/issues/4))
- Torrent list: progress, speeds, ETA, peers, ratio; pause/resume/remove ([#5](https://github.com/adamgreenwell/flume/issues/5))
- Per-torrent file tree with selective download ([#6](https://github.com/adamgreenwell/flume/issues/6))
- Settings with persistence ([#7](https://github.com/adamgreenwell/flume/issues/7))
- Resume correctly across restarts ([#8](https://github.com/adamgreenwell/flume/issues/8))
- Investigate Windows file-locking and seeding ([#9](https://github.com/adamgreenwell/flume/issues/9))

Also in this phase: replace the Phase 0 polling hook with backend-pushed,
batched events before the torrent count grows.

## Phase 2 — Polish & platform integration

- Theming and the visual design pass ([#10](https://github.com/adamgreenwell/flume/issues/10))
- Per-torrent detail view with piece heatmap ([#11](https://github.com/adamgreenwell/flume/issues/11))
- Magnet/`.torrent` protocol association and single instance ([#12](https://github.com/adamgreenwell/flume/issues/12))
- Notifications and system tray ([#13](https://github.com/adamgreenwell/flume/issues/13))
- Keyboard shortcuts, context menus, accessibility ([#14](https://github.com/adamgreenwell/flume/issues/14))

## Phase 3 — Hardening & distribution

- Release pipeline for all four package formats ([#15](https://github.com/adamgreenwell/flume/issues/15))
- Sequential download for streaming ([#16](https://github.com/adamgreenwell/flume/issues/16))
- Performance validation with 10+ torrents ([#17](https://github.com/adamgreenwell/flume/issues/17))
- Signing, notarization, and troubleshooting docs ([#18](https://github.com/adamgreenwell/flume/issues/18))

## Known limitations

- **Windows and Linux unverified locally.** Developed on macOS 27. CI builds
  for all platforms, but first-run behaviour needs a manual pass.
- **Polling, not events.** The Phase 0 status hook polls at 1 Hz. Fine for one
  status card, wrong for a torrent list.
- **No published builds.** Until Phase 3, build from source.

## Out of scope

Flume deliberately does not do cryptocurrency, chat, RSS automation, or paid
features. It also will not ship a second CLI — `rqbit` already exists.
