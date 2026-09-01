# User Guide

## Adding torrents

**Magnet links.** Paste into the add box, or copy one and let Flume notice:
the add sheet detects a magnet on the clipboard and offers it. Verified on
macOS, Windows and Linux.

A magnet carries an info hash, not a file list — that has to come from a peer
who already has the torrent, over the DHT. So the status indicator must read
**Ready** first; a magnet added while **Connecting** will sit waiting.

The add sheet counts the seconds while it waits, and gives up after a minute
rather than waiting forever. A well-seeded torrent answers in a few seconds. If
it times out, that usually means the torrent has no active seeders — not that
the link is wrong.

A `.torrent` file needs none of this, because the file list is already inside
it.

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
| Tunnel check        | Off, warn, or hold transfer — see below  |

## Only transfer through a tunnel

**Settings → Network.** Off unless you turn it on.

Flume works out which network interface your traffic would actually leave by,
and can hold everything while that is not a tunnel. It reads this from your own
routing table and sends nothing to do it — there is no "what is my IP" request
to a web service, which would mean handing your address to a stranger in order
to be told it is protected.

Three modes:

| Mode     | What it does                                                      |
| -------- | ----------------------------------------------------------------- |
| **Off**  | Flume does not look. Nothing is checked and nothing is stopped    |
| **Warn** | Tells you when traffic is leaving outside a tunnel; stops nothing |
| **Hold** | Runs no torrent session at all while traffic is outside a tunnel  |

**Hold does not pause your torrents — it stops the engine.** While held there
are no transfers, no peer connections, no DHT and no listening port. Nothing is
written to your torrents, which is why the ones you paused yourself stay paused
and the ones that were running come back running. A drop takes effect
immediately; recovery waits about ten seconds of a steady tunnel, so a VPN
reconnecting does not make your library flap and re-announce to every tracker.

**Accept only one interface.** Leave the picker on _Any tunnel interface_ unless
you have a reason not to. Pinning is stricter and more brittle: on macOS a new
`utun` appears on every VPN connect and the old ones stay, so the number you pin
today is not the one carrying traffic tomorrow. Windows and Linux name the
adapter after the VPN config and stay put.

If you pin an interface Flume cannot identify as a tunnel, it is accepted
anyway — that is your word for it, and Flume says so rather than claiming it
found a tunnel.

### What this check cannot tell you

It sees which interface traffic leaves by and whether it looks like a tunnel. It
cannot see where the tunnel goes or who runs it:

- A **PPPoE connection or USB cellular modem** looks exactly like a VPN tunnel
  from here. If your machine dials the connection itself rather than going
  through a router, the check may call your ordinary internet connection a
  tunnel.
- On **Windows with OpenVPN**, the adapter is named `Local Area Connection` and
  cannot be told apart from an Ethernet card. Transfer will be held even though
  you are protected — pin that interface to get through.

See [[Privacy]] for the full contract.

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

It counts the peers you are connected to and does not count you, because the
question it answers is whether the swarm can finish your download. Once a
torrent is complete the strip disappears — the question is answered, and a
finished torrent cannot stall.

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

**Add dialog stuck fetching a file list.** Only magnets do this, and only
while looking for a peer who has the torrent. It gives up after a minute. A
torrent with no active seeders will never answer, however long you wait.

**Progress after a crash.** Very little is lost. Flume writes its record of
which pieces it has as the download runs, not only at exit, so even killing the
process leaves that record at most a few megabytes behind. Restarting picks up
from there and re-fetches only that much — it does not re-check the whole file.

You may briefly see **Checking** on launch. That is Flume verifying data
already on disk, which is fast and does not re-download anything.
