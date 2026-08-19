# User Guide

> Most of what this page will describe arrives in Phase 1. Today Flume shows
> live engine status and does not yet manage torrents. This page is the
> intended shape, and is updated as features land.

## Adding torrents _(Phase 1)_

**Magnet links.** Paste into the add box. Flume detects a magnet link on the
clipboard when the window regains focus and offers to add it.

Magnet links resolve their metadata over the DHT, so the status indicator must
read **Ready** first. A magnet added while **Connecting** will sit waiting.

**`.torrent` files.** Use the file picker, or drag the file onto the window.

## Selecting files _(Phase 1)_

Each torrent has a file tree with checkboxes. Deselected files are not
downloaded.

This matters for distro torrents, which often bundle several ISO variants plus
checksum files when you want one image.

## Controlling torrents _(Phase 1)_

| Action      | Effect                                                  |
| ----------- | ------------------------------------------------------- |
| Pause       | Stops transfer, keeps the torrent                       |
| Resume      | Restarts transfer from existing progress                |
| Remove      | Removes from the list, **asks** whether to delete files |
| Open folder | Reveals the download in your file manager               |

Removal always asks before deleting data. Deleting a partially downloaded ISO
by accident is a bad afternoon.

## Settings _(Phase 1)_

| Setting             | Notes                                    |
| ------------------- | ---------------------------------------- |
| Download folder     | Where completed and in-progress files go |
| Rate limits         | Global, plus per-torrent overrides       |
| Max active torrents | Limits concurrent transfers              |
| Listen port         | Default 42221                            |
| UPnP                | Automatic router port forwarding         |
| DHT                 | Required for magnet links                |
| Theme               | Light, dark, or follow system            |

## Seeding

Flume seeds completed torrents while running. Seeding requires an open
listening port — see the firewall notes in [[Getting-Started]].

Seeding Linux ISOs back is genuinely useful; distro mirrors carry real
bandwidth costs.

## Troubleshooting

**Slow or no download.** Check the status indicator reads **Ready**. Check peer
count — zero peers on a well-seeded torrent usually means a blocked port.

**Torrent stuck at 99%.** Usually a single rare piece. librqbit will keep
trying; leave it running.

**Downloads restart after a crash.** Fast-resume state flushes on clean
shutdown. If the app is killed hard, some re-hashing on next launch is
expected — it verifies rather than re-downloads.
