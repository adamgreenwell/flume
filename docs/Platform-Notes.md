# Platform Notes

> Testing a build on a platform for the first time? Work through
> [[Smoke-Test-Checklist]] rather than clicking around — it is ordered so the
> most likely failures surface first.

## Icons

`app-icon.png` in the repository root is the source of every app icon, and is
the default `cargo tauri icon` looks for. Regenerate after changing it:

```bash
cargo tauri icon
```

That rewrites everything in `src-tauri/icons/`. It also emits `android/` and
`ios/` sets, which Flume does not ship — delete them, or the tree grows 35
files and 1.5 MB for platforms with no build.

The source must have a real alpha channel. macOS and Linux draw the icon
against whatever is behind it, so a baked-in background renders as a square
tile rather than a shaped mark.

## Build matrix

| Target                     | Runner                  | Output                |
| -------------------------- | ----------------------- | --------------------- |
| macOS 12+                  | `macos-latest`          | `.dmg`, `.app`        |
| Windows 10/11              | `windows-latest`        | `.msi`, `.exe` (NSIS) |
| Debian 12+ / Ubuntu 22.04+ | `ubuntu-22.04`          | `.deb`                |
| Fedora 38+ / RHEL 9.4+     | Fedora/RHEL 9 container | `.rpm`                |

**The Linux glibc floor is set by the build runner, not by our code.** A binary
built on Ubuntu 24.04 will not run on Debian 12. This is why CI pins
`ubuntu-22.04` for `.deb` and uses a RHEL 9-era container for `.rpm`.

Flume's TLS choice helps here: because we build librqbit with `rust-tls`
instead of native TLS, packages carry **no `libssl` runtime dependency**, which
removes the most common cross-distro breakage.

## macOS

**Minimum:** 12.0 (set in `tauri.conf.json`).

**WebView:** WKWebView, bundled with the OS. No runtime to ship.

**Released builds are signed and notarized.** The release workflow signs with a
Developer ID certificate and submits to Apple's notary service; the rc.2 run
logged `Notarizing Finished with status Accepted`. Gatekeeper does not warn.

Signing is conditional on `secrets.APPLE_CERTIFICATE` being set — the workflow
picks the signed or unsigned path from `HAS_APPLE_CERT`, so a fork with no
certificate still builds rather than failing. Check which ran before claiming
a build is signed: the job's step list says `Build and bundle (signed)` or
`(unsigned)`, and one of them is always skipped.

`scripts/setup-macos-signing.sh` sets the four sensitive secrets. It never
echoes one and never takes one as an argument.

**An unsigned build** — a local `tauri build`, or a fork's — is blocked by
Gatekeeper. Bypass via right-click → Open, or:

```bash
xattr -dr com.apple.quarantine /Applications/Flume.app
```

**Signing** requires an Apple Developer account ($99/yr), a Developer ID
Application certificate, and notarization through Apple's service. Tracked in
[#18](https://github.com/adamgreenwell/flume/issues/18).

### Do not set `licenseFile` in `tauri.conf.json`

It looks like obvious good practice — ship the licence with the app — and what
it actually does on macOS is turn the `.dmg` into a **click-through EULA**.
Every user must accept an agreement before the disk image will open, and
non-interactive tooling cannot mount it at all (`hdiutil: attach canceled`).

That pattern is inherited from commercial software. Apache-2.0 is permissive
and applies whether or not anyone clicks Agree; the licence is already in the
repository, in `NOTICE`, and inside the app bundle. The gate adds friction and
buys nothing.

Confirm with:

```bash
hdiutil imageinfo Flume_x.y.z_aarch64.dmg | grep "Software License Agreement"
```

`false` is correct.

### Do not set `minimumSystemVersion` in `tauri.conf.json`

It looks like the obvious place to declare the minimum macOS, and it breaks the
release build.

Setting it makes the Tauri CLI export `MACOSX_DEPLOYMENT_TARGET` for the entire
cargo invocation. That variable leaks into the **host** build of proc-macro
crates, where it breaks `ctor-proc-macro`; the build then fails with
`can't find crate for ctor_proc_macro`. Any value triggers it.

The minimum is declared in `src-tauri/Info.plist` instead, which
`tauri-bundler` merges into the generated plist. Finder enforces
`LSMinimumSystemVersion` either way, so the user-visible behaviour is the same.

See [#22](https://github.com/adamgreenwell/flume/issues/22) for the full
investigation.

**Universal binaries** (`--target universal-apple-darwin`) roughly double
bundle size. Decision deferred.

## Windows

**Minimum:** Windows 10 1803.

**WebView:** WebView2, preinstalled on Windows 11 and current Windows 10. Tauri
can bundle a bootstrapper for older systems.

**SmartScreen** warns on unsigned executables until a download reputation
builds. Windows builds are **not** signed — unlike macOS, which is. Code
signing certificates cost real money annually; EV certificates clear
SmartScreen immediately, OV ones build reputation over time.

**File locking and seeding — confirmed on v9, and fixed.** Windows refuses an
open whose access conflicts with the sharing mode of an existing handle.
`FilesystemStorage` opened every file read _and_ write when `allow_overwrite`
is set, including a complete torrent that would only ever be served, so another
application holding a download with `FILE_SHARE_READ` blocked the add outright.

The patched librqbit Flume carries retries read-only on a sharing violation.
Seeding only reads, so a completed torrent is served normally. Sent upstream as
[`ikatson/rqbit#645`](https://github.com/ikatson/rqbit/pull/645); see
[#9](https://github.com/adamgreenwell/flume/issues/9).

A holder that permits **no** sharing at all (`share_mode(0)`) still cannot be
read by anyone, which is Windows as documented rather than a defect.
`src-tauri/tests/windows_file_locking.rs` covers both, and is the only place
they run — the file is `#[cfg(windows)]`.

## Linux

**WebView:** WebKitGTK 4.1. This is the single biggest source of rendering
differences from macOS and Windows — it lags on newer CSS features. Test layout
changes on Linux early rather than late.

### Debian / Ubuntu

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev \
  librsvg2-dev patchelf build-essential curl wget file libxdo-dev libssl-dev
```

### Fedora / RHEL / Rocky / Alma

```bash
sudo dnf install -y webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel gcc gcc-c++ make
```

RHEL 9 derivatives may need EPEL for `libappindicator-gtk3-devel`. Verify
`webkit2gtk4.1` availability on the exact base image before relying on it.

### Wayland

Tauri runs under Wayland via GTK. If rendering misbehaves, force X11:

```bash
GDK_BACKEND=x11 flume
```

### AppImage

Optional additional output. Bundles more dependencies, so it works across more
distros at the cost of size.

## Magnet link association

Flume registers itself as a handler for `magnet:` URIs. How that registration
happens differs by platform, which matters when testing:

| Platform | Registered by                                        | Works in `tauri dev`?              |
| -------- | ---------------------------------------------------- | ---------------------------------- |
| macOS    | `CFBundleURLTypes` in the bundled app's `Info.plist` | **No** — needs an installed `.app` |
| Windows  | Registry entries written by the installer            | **No** — needs an installed build  |
| Linux    | `.desktop` MIME entry, or at runtime                 | Yes, via runtime registration      |

On macOS the app logs `runtime deep-link registration unavailable
(unsupported platform)` at debug level on every dev start. That is expected,
not a fault: macOS has no runtime registration API, so the association only
exists for an installed bundle.

**Consequence for testing:** clicking a magnet link in a browser cannot be
verified with `npm run tauri:dev` on macOS or Windows. Build a bundle
(`npm run tauri:build`) and launch that instead.

Verified on macOS against a real bundle: `CFBundleURLTypes` registers the
`magnet` scheme, and launching the app a second time with a magnet argument
hands it to the running instance and exits, rather than starting a second
engine.

> **If another client is installed**, it may already own the `magnet:` default.
> Both apps are registered; macOS picks one. Change the default in the other
> client's settings, or via a LaunchServices utility.

## System tray

The tray is optional by design. Some Linux desktops ship no system tray at all,
so a failure to create the icon is logged and ignored rather than being fatal —
the app is perfectly usable without one.

Left-clicking the icon reveals the window on Windows and Linux; macOS opens the
menu on any click, which is the platform convention and is handled by Tauri.

## Sandboxing and firewalls

Flume needs:

- **Outbound TCP** to peers and trackers
- **Outbound UDP** for DHT
- **Inbound TCP** on the listen port (42221) for incoming peers and seeding
- **Outbound UDP to the gateway** for UPnP port mapping, if enabled

macOS prompts for incoming connections on first launch. Linux firewalls
(`ufw`, `firewalld`) usually need an explicit rule for inbound.

## The tunnel check reads differently on each platform

`src-tauri/src/egress/` decides which interface traffic leaves by. What it can
see varies, and the differences are measured rather than assumed — run
`cargo test --lib egress::tests::show_this_machines_interfaces -- --ignored
--nocapture` on any machine to see exactly what it is handed.

**macOS.** Every tunnel is a `utun`, whatever the protocol — WireGuard,
OpenVPN and OpenConnect all land there, since kexts were blocked on Apple
Silicon and `NEPacketTunnelProvider` is the only route left. A new `utun`
appears on each connect and the old ones persist; one machine went from
`utun0`–`utun8` to `utun0`–`utun12` in an afternoon, which is why pinning is
discouraged here. The MAC address is useless on this platform:
`network-interface` reports a placeholder for every interface, including real
Ethernet adapters.

**Linux.** The name belongs to the VPN config, not the OS — TorGuard's
WireGuard interface is `torguard-wg`, not `wg0`. What identifies it is that a
tunnel has no `AF_PACKET` entry and so reports no MAC, while `eth0` reports a
real one.

**Windows.** The friendly name is the WireGuard config file's name, so the same
vendor produces `wg-torguard` here against `torguard-wg` on Linux. The OpenVPN
adapter is the hard case: a TAP-Windows Adapter V9 called `Local Area
Connection`, with a MAC and an 802.3 media type, indistinguishable from an
Ethernet card through anything the crate exposes. `IP_ADAPTER_ADDRESSES.IfType`
(53 `propVirtual` against 6 `ethernetCsmacd`) would separate them; the crate
reads that field and does not expose it, and Flume forbids `unsafe_code`. So
OpenVPN on Windows is not recognised, and the interface pin is the way through.

On **Windows on ARM**, WireGuard is the only one of TorGuard's three tunnel
types that connects at all — the TAP-family drivers are kernel-mode x64 `.sys`
files and Windows on ARM does not emulate those. That bounds the gap above to
x64.

## Per-platform smoke checklist

Run before tagging a release:

- [ ] Window opens and renders correctly
- [ ] Engine reaches **Ready** within ~10s
- [ ] Listen port binds; verify in status
- [ ] Add a magnet (a Linux ISO) and confirm metadata resolves
- [ ] Download completes and file integrity verifies
- [ ] Seeding works with another client connected
- [ ] Quit and relaunch; torrent resumes without full re-hash
- [ ] Open containing folder works
- [ ] Theme matches system setting
- [ ] With a VPN available: the tunnel check names the right interface, and
      Hold stops and restarts the engine (see the Smoke Test Checklist)
