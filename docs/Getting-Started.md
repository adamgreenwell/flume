# Getting Started

## Installing

> Packaged builds are not published yet. The release pipeline lands in Phase 3
> ([#15](https://github.com/adamgreenwell/flume/issues/15)). Until then, build
> from source — see [[Development Setup]].

When releases are available:

| Platform                   | File                        | Notes                                    |
| -------------------------- | --------------------------- | ---------------------------------------- |
| macOS 12+                  | `Flume_x.y.z.dmg`           | Unsigned builds need a Gatekeeper bypass |
| Windows 10/11              | `Flume_x.y.z.msi` or `.exe` | SmartScreen may warn on unsigned builds  |
| Debian 12+ / Ubuntu 22.04+ | `flume_x.y.z_amd64.deb`     | `sudo apt install ./flume_*.deb`         |
| Fedora 38+ / RHEL 9.4+     | `flume-x.y.z.rpm`           | `sudo dnf install ./flume-*.rpm`         |

See [[Platform Notes]] for the unsigned-build workarounds.

## First run

On first launch Flume:

1. Opens its window immediately.
2. Starts a torrent session in the background — this is why the status
   indicator briefly reads **Starting**, then **Connecting**.
3. Bootstraps the DHT. Once the routing table has enough nodes, the indicator
   reads **Ready** and magnet links will resolve. This normally takes a few
   seconds.

The status indicator means:

| State          | Meaning                                                 |
| -------------- | ------------------------------------------------------- |
| **Starting**   | The engine has not bound a listening port yet           |
| **Connecting** | Listening; DHT still bootstrapping                      |
| **Ready**      | Peer discovery is working                               |
| **Degraded**   | Running, but DHT is off — magnet links will not resolve |

## Defaults

| Setting              | Default                  |
| -------------------- | ------------------------ |
| Download folder      | Your OS Downloads folder |
| Listen port          | 42221 (TCP)              |
| DHT                  | Enabled                  |
| UPnP port forwarding | Enabled                  |

Session state lives in your OS application-data directory:

- macOS: `~/Library/Application Support/io.github.adamgreenwell.Flume`
- Windows: `%APPDATA%\io.github.adamgreenwell.Flume`
- Linux: `~/.local/share/io.github.adamgreenwell.Flume` (or `$XDG_DATA_HOME`)

## Firewall

For good download speeds and any seeding, allow inbound TCP on port 42221.
UPnP attempts this automatically when your router permits it. If your router
has UPnP disabled — a reasonable security posture — forward the port manually.

You can still download without an open port; you will just connect to fewer
peers.

## Troubleshooting

**Stuck on "Starting".** Usually another process holds port 42221. Check the
terminal output if running from source.

**Stuck on "Connecting".** The DHT cannot reach its bootstrap nodes. Check that
outbound UDP is not blocked.

**"Degraded".** DHT is disabled. Magnet links need it; `.torrent` files with
working trackers will still function.
