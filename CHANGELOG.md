# Changelog

Notable changes to Flume. Format follows [Keep a Changelog][kac]; versioning is
[semantic][semver].

[kac]: https://keepachangelog.com/en/1.1.0/
[semver]: https://semver.org/spec/v2.0.0.html

## [1.1.0] — 2026-09-01

### Added

- **Only transfer through a tunnel.** An opt-in check that works out which
  network interface your traffic would actually leave by, and can hold all
  transfer while that is not a tunnel. It reads your own routing table and
  sends nothing — no "what is my IP" request to a web service, which would mean
  handing your address to a stranger in order to be told it is protected.

  Holding does not pause your torrents: Flume runs no torrent session at all,
  so there are no transfers, no peer connections, no DHT and no listening port.
  Nothing is written to your torrents, so whatever you paused stays paused and
  whatever was running comes back running. The check runs before the engine
  starts, so nothing announces to a tracker from an address you did not intend.

  A drop takes effect immediately; recovery waits about ten seconds of a steady
  tunnel, so a reconnecting VPN does not re-announce your library to every
  tracker. Off by default. See the User Guide for what the check cannot tell
  you — a PPPoE line looks like a tunnel from here, and OpenVPN on Windows does
  not.

## [1.0.0] — 2026-08-30

First public release.

### Added

- **Torrent lifecycle.** Add by `.torrent` or magnet with a file picker before
  anything downloads, then pause, resume, and remove — with a confirmation
  before any data is deleted.
- **Swarm health.** Every torrent reports whether it will actually finish, not
  just whether it is moving. Computed from the connected peers' bitfields:
  `Healthy` when every piece is well covered, `Thin swarm` when it is covered
  but barely, `No seeds` when some piece is held by nobody at all.
- **"What is limiting this download".** Ranks the constraints Flume can measure
  — your download cap, piece availability, peer upload — and marks at most one
  as binding, with a sentence saying whether changing a setting would help.
- **Piece map.** Two stacked strips sharing one bucketing: what you have above,
  how many peers hold each region below. A tail thinning to the right is a
  download heading for a stall.
- **Import from other clients.** Detects Transmission, qBittorrent and Deluge,
  and takes their torrents over in place — nothing is downloaded twice.
- **First run.** Three questions rather than thirty settings.
- Settings apply immediately, with no OK or Apply button, and survive a restart.
- File selection can be changed after a torrent is added.
- Desktop notifications on completion; magnet links can be opened from a
  browser.

### Fixed

- **Windows: seeding a file another application holds open.** librqbit opened
  every file read _and_ write, including completed torrents that are only ever
  served, so a media player or antivirus holding a download blocked the torrent
  from being added at all. Sent upstream as [`ikatson/rqbit#645`][r645].

### Known limitations

- **Windows builds are unsigned.** SmartScreen warns on an unsigned executable
  until a download reputation builds. The wiki's Signing & Distribution page
  covers the least alarming way through. macOS is signed with a Developer ID
  certificate and notarized by Apple, so Gatekeeper does not warn.
- **Flume runs a patched librqbit.** Two changes are carried on a fork and are
  with upstream as [#644][r644] and [#645][r645]; the patch is removed when they
  land in a release. See the README.
- Per-tracker announce status is not shown — librqbit does not expose it.
- Availability is not shown for a completed torrent: it measures whether the
  _swarm_ can finish your download, a question already answered once you have
  every piece.

[r644]: https://github.com/ikatson/rqbit/pull/644
[r645]: https://github.com/ikatson/rqbit/pull/645
[1.0.0]: https://github.com/adamgreenwell/flume/releases/tag/v1.0.0
