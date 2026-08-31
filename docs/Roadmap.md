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
