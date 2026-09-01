# Smoke Test Checklist

What to check on a platform Flume has never actually been run on. Ordered so
the things most likely to be broken come first — if the app does not start,
nothing below matters.

Record results in the release's issue or a comment, including the ones that
pass. "Everything worked" is not reproducible; "Ubuntu 24.04 arm64, all of
§1–§4 passed, §5 tray icon missing" is.

## 0. Before you start

- [ ] Verify the download against `SHA256SUMS.txt`
- [ ] Note the exact OS name, version, and architecture
- [ ] If a previous Flume ran on this machine, delete its data directory first,
      so you are testing first-run behaviour and not a restored session

| OS      | Data directory                                                |
| ------- | ------------------------------------------------------------- |
| Windows | `%APPDATA%\io.github.adamgreenwell.Flume`                     |
| Linux   | `~/.local/share/io.github.adamgreenwell.Flume`                |
| macOS   | `~/Library/Application Support/io.github.adamgreenwell.Flume` |

## 1. Install and launch

- [ ] The installer runs and completes
- [ ] Note what the OS says about the build being unsigned, **verbatim** —
      Windows SmartScreen wording differs by version, and the docs should match
      what users actually see
- [ ] The window opens
- [ ] The window is not blank — a blank window means the WebView failed to load
      the frontend, which is the single most likely cross-platform failure
- [ ] The status pill reaches **Ready** within about 30 seconds

If the pill stays on _Connecting_, DHT bootstrap is being blocked — usually a
firewall. Note it and continue; it does not block the rest.

## 2. Add a torrent

Use a real, legal torrent. Ubuntu's is ideal:
<https://releases.ubuntu.com/24.04.3/>

- [ ] **Magnet:** paste into the add dialog, press Resolve
- [ ] The file list appears within ~30s (this needs a working DHT)
- [ ] Deselect a file; the "Download _n_" button updates its size
- [ ] Confirm; the torrent appears in the list
- [ ] Progress advances and the download speed is non-zero
- [ ] **`.torrent` file:** the file picker opens and resolves a downloaded file
- [ ] **Drag and drop:** dragging a `.torrent` onto the window highlights it and
      opens the add dialog

## 3. Control

- [ ] Pause: state changes to _Paused_ and speeds drop to zero
- [ ] Resume: transfer restarts
- [ ] Right-click a row: the context menu appears **at the pointer**, and near
      the window edge it flips to stay on-screen
- [ ] "Open containing folder" opens the correct directory in the file manager
- [ ] Remove **without** deleting files: the row disappears, the files remain
- [ ] Remove **with** deleting files: the files are gone

## 4. Persistence

The most valuable check here, and the easiest to get wrong.

- [ ] Add a torrent, deselect at least one file, let it download briefly
- [ ] Quit Flume **properly** (window close or tray → Quit, not a kill)
- [ ] Reopen
- [ ] The torrent is still listed
- [ ] Progress resumed rather than restarting from zero
- [ ] **Files → the same files are still deselected**

That last one matters most: a silent reset to "all files" would download the
gigabytes you deliberately excluded, and nothing on screen would look wrong.

## 5. Platform integration

- [ ] **Tray icon** appears. On Linux this genuinely may not — some desktops
      have no system tray, which Flume treats as non-fatal. Note your desktop
      environment (GNOME, KDE, …) either way
- [ ] Tray → Pause all, then Resume all
- [ ] Tray → Show brings the window forward
- [ ] **Magnet association:** click a magnet link in a browser. Flume should
      come forward with the add dialog prefilled
- [ ] With Flume already running, click another magnet — it should reuse the
      same window, **not** start a second copy
- [ ] Completion notification appears when a torrent finishes

If another torrent client is installed, it may already own the `magnet:`
default. Both are registered; the OS picks. Note which one wins.

## 6. Settings

- [ ] Change the theme; it applies immediately
- [ ] Set theme to System, then change the OS appearance — Flume follows
      without a restart
- [ ] Set an upload limit; the rate drops accordingly within a few seconds
- [ ] Change the listen port and save; the warning about restarting appears,
      and transfers resume by themselves afterwards
- [ ] Reopen settings: the values persisted

## 7. The tunnel check

Needs a VPN on the machine. Off by default, so skip if there is none.

- [ ] Settings → Network, set the check to **Warn** with the VPN disconnected:
      a sentence names the interface traffic is leaving by, and nothing stops
- [ ] Set it to **Hold**: the engine stops before the dialog closes, the library
      empties, and the dock figures read as dashes rather than zeros
- [ ] The message names the interface and says your torrents were not paused
- [ ] It does **not** say "The torrent engine is still starting"
- [ ] Connect the VPN: a countdown appears, and after about ten seconds the
      library comes back
- [ ] Whatever you had paused before the hold is still paused
- [ ] Disconnect: transfer stops again within a second or two
- [ ] Quit while held, relaunch: the library is still intact and still held —
      **not** paused, and not the first-run "No torrents yet" screen
- [ ] Pin an interface that is not the live one: transfer holds, and the message
      offers to re-pin

The launch case is the one worth being careful about. Quit while held, relaunch,
and check the log for `holding at launch, no torrent engine started` — there
should be no tracker or DHT activity after that line at all.

## 8. Windows only — the file-locking question ([#9](https://github.com/adamgreenwell/flume/issues/9))

Unanswered, and only answerable on Windows. The claim predates librqbit v9 and
has never been checked against it.

- [ ] Let a torrent finish so Flume is seeding it
- [ ] Open the downloaded file in another application that holds it open — a
      media player, or `certutil -hashfile <path> SHA256` in another terminal
- [ ] Watch Flume's upload speed and the torrent's state
- [ ] Does it keep seeding, or does the torrent error?

Either answer closes the issue. "It kept seeding" means no patch is needed
against v9.

## 9. Anything odd

Worth recording even if it seems minor:

- Fonts that look wrong or fall back
- Layout that clips or overflows at your window size or scaling factor
- Slow or janky window resizing
- Anything in the terminal if you launched from one
