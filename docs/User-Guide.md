# User Guide

## Adding torrents

**Magnet links.** Paste into the add box. Flume detects a magnet link on the
clipboard when the window regains focus and offers to add it.

Magnet links resolve their metadata over the DHT, so the status indicator must
read **Ready** first. A magnet added while **Connecting** will sit waiting.

**`.torrent` files.** Use the file picker, or drag the file anywhere onto the
window — Flume highlights the drop target and opens the add dialog with it.
Non-torrent files are ignored rather than reported as an error.

**Clipboard.** If you have a magnet link on your clipboard when you open the
add dialog, Flume prefills it. The clipboard is read only at that moment, when
you have deliberately opened the dialog — never in the background.

## Selecting files

Each torrent has a file tree with checkboxes. Deselected files are not
downloaded.

This matters for distro torrents, which often bundle several ISO variants plus
checksum files when you want one image.

## Controlling torrents

| Action      | Effect                                                  |
| ----------- | ------------------------------------------------------- |
| Pause       | Stops transfer, keeps the torrent                       |
| Resume      | Restarts transfer from existing progress                |
| Remove      | Removes from the list, **asks** whether to delete files |
| Open folder | Reveals the download in your file manager               |

Removal always asks before deleting data. Deleting a partially downloaded ISO
by accident is a bad afternoon.

## Settings

| Setting             | Notes                                    |
| ------------------- | ---------------------------------------- |
| Download folder     | Where completed and in-progress files go |
| Rate limits         | Global, plus per-torrent overrides       |
| Max active torrents | Limits concurrent transfers              |
| Listen port         | Default 42221                            |
| UPnP                | Automatic router port forwarding         |
| DHT                 | Required for magnet links                |
| Theme               | Light, dark, or follow system            |

## Looking at one torrent

Select a torrent and choose **Open details** for the inspector. The overview
tab answers a question the list cannot: not "is this downloading" but "will
this finish".

### Swarm health

The health column carries a word, never a colour alone.

| Reads          | Means                                                                                   |
| -------------- | --------------------------------------------------------------------------------------- |
| **Healthy**    | Every piece is held by several connected peers                                          |
| **Thin swarm** | Every piece is held, but only just — losing a peer could strand it                      |
| **No seeds**   | No connected peer holds some piece, so this cannot finish as it stands                  |
| **Connected**  | Peers are connected but none has sent a bitfield yet, so there is nothing to judge from |
| **Seeding**    | Complete, and serving peers                                                             |
| **Idle**       | Paused, checking, or stopped on an error                                                |

**Connected is not a synonym for healthy.** It means Flume does not know yet.
Guessing between thin and healthy from the peer count alone would be a
confident wrong answer: six peers who between them hold every piece will
finish, and forty peers who all stopped at the same 6% will not.

### The piece map

Two strips, stacked and sharing the same columns.

The **upper** strip is what you have: verified, in flight, not yet requested.
Useful because overall progress hides shape — 60% with a solid head and an
empty tail is downloading in order, and 60% scattered evenly is pulling
rarest-first.

The **lower** strip is how many peers hold each region. It is the one that
warns you: a tail that thins towards the right is a torrent heading for a
stall. A region **no** peer holds is drawn full height in red, taller than
anything else on the strip, and the caption says so.

### What is limiting this download

Ranks the constraints Flume can actually measure and marks at most one as
binding, with a sentence saying whether changing a setting would help.

- **Your download cap** — binding when you are hitting it. Raising it in
  Settings will help.
- **Peer upload** — binding when nothing on your machine is holding the
  transfer back, so the peers are. No setting will make this faster.
- **Piece availability** — binding when a piece is missing entirely. Nothing in
  Settings fixes that; it needs a peer holding the missing pieces to appear.

If nothing is binding, nothing is marked. A factor Flume cannot measure a
ceiling for shows no bar and reads **Not measured** rather than an invented
one.

Connection slots, disk write queue and hash-checking load are **absent** rather
than estimated — the torrent engine does not report them, and a plausible
number would be worse than none.

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
