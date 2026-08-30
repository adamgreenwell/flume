# Getting Started

## Installing

Download the file for your platform from the
[latest release](https://github.com/adamgreenwell/flume/releases/latest).

| Platform                   | File                             | Notes                                      |
| -------------------------- | -------------------------------- | ------------------------------------------ |
| macOS 12+                  | `Flume_<version>_universal.dmg`  | One build for both Intel and Apple Silicon |
| Windows 10/11              | `Flume_<version>_x64-setup.exe`  | The `.msi` is for managed deployment       |
| Debian 12+ / Ubuntu 22.04+ | `Flume_<version>_amd64.deb`      | `sudo apt install ./Flume_*.deb`           |
| Fedora 38+ / RHEL 9.4+     | `Flume-<version>-1.x86_64.rpm`   | `sudo dnf install ./Flume-*.rpm`           |
| Any Linux                  | `Flume_<version>_amd64.AppImage` | `chmod +x` and run — no install            |

Windows and Linux also ship arm64 builds, named `arm64` or `aarch64` in place
of `x64`/`amd64`/`x86_64`. macOS needs no such choice: the `.dmg` is a universal
binary.

Every release carries a `SHA256SUMS.txt` if you want to check what you
downloaded.

The macOS build is signed and notarized, so it opens without argument. The
Windows builds are **not** signed, so SmartScreen will interrupt the installer
with an "unrecognised app" panel — that is what an unsigned installer looks
like, not a sign the download is damaged. See [[Signing-and-Distribution]] for
what each system says and the least alarming way past it.

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
