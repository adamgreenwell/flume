# Changelog

Notable changes to Flume. Format follows [Keep a Changelog][kac]; versioning is
[semantic][semver].

[kac]: https://keepachangelog.com/en/1.1.0/
[semver]: https://semver.org/spec/v2.0.0.html

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

- **Builds are unsigned.** Windows SmartScreen and macOS Gatekeeper will warn
  until a signing certificate is in place. The wiki's Signing & Distribution
  page covers the least alarming way through.
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
