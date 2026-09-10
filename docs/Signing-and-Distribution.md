# Signing & Distribution

What it takes to ship Flume so that a user's operating system does not treat it
as suspicious, and what to tell them when it does.

> **Current state, per platform:**
>
> - **macOS — signed and notarized.** The Apple secrets are set, so the release
>   workflow takes the signed path.
> - **Windows — unsigned, with an application to SignPath Foundation pending as
>   of 2026-09-10.** This reverses an earlier decision; see [Windows](#windows).
> - **Linux — unsigned.** Normal for direct download; the distributions that
>   would require a signature are the ones Flume is not in.
>
> The pipeline selects between a signed and an unsigned build depending on
> whether `APPLE_CERTIFICATE` is set, so anyone can still build Flume without
> certificates. See [Setting up macOS signing](#setting-up-macos-signing).

## What signing actually buys

Not security in the sense people assume. A signature does not make the code
safe; it makes the code _attributable_ and _tamper-evident_, which is what the
OS gatekeeping is really checking.

The practical benefit is that users are not confronted with a scary dialog that
trains them to click through warnings.

## macOS

| Requirement                          | Cost                                 |
| ------------------------------------ | ------------------------------------ |
| Apple Developer Program              | $99/year                             |
| Developer ID Application certificate | Included                             |
| Notarization                         | Included, but adds minutes per build |

Signing alone is not enough. macOS also requires **notarization** — uploading
the build to Apple, which scans it and issues a ticket that gets stapled to the
bundle. An app that is signed but not notarized is still blocked.

Set these repository secrets and the release workflow picks them up:

| Secret                       | What it is                                               |
| ---------------------------- | -------------------------------------------------------- |
| `APPLE_CERTIFICATE`          | Developer ID cert as base64 `.p12`                       |
| `APPLE_CERTIFICATE_PASSWORD` | Password for that `.p12`                                 |
| `APPLE_SIGNING_IDENTITY`     | e.g. `Developer ID Application: Name (TEAMID)`           |
| `APPLE_ID`                   | Apple ID used for notarization                           |
| `APPLE_PASSWORD`             | An **app-specific** password, never the account password |
| `APPLE_TEAM_ID`              | 10-character team identifier                             |

### What a user sees without it

macOS refuses to open the app: _"Flume is damaged and can't be opened"_ or
_"cannot be opened because the developer cannot be verified"_. The first
message is misleading — nothing is damaged; the quarantine attribute is set and
there is no notarization ticket.

Their options:

1. Right-click the app → **Open** → **Open** in the dialog. Works on most
   versions, and is the least alarming route.
2. **System Settings → Privacy & Security**, then "Open Anyway" next to the
   blocked app.
3. Remove the quarantine attribute directly:

   ```bash
   xattr -dr com.apple.quarantine /Applications/Flume.app
   ```

Option 3 is what most guides lead with, and it is the one to put last: telling
users to strip security attributes from downloaded binaries is a bad habit to
teach, even when it is correct here.

## Setting up macOS signing

You need an Apple Developer Program membership. Everything below is done by
**you**, on your own machine — the certificate and passwords must never be
pasted into a chat, a file in this repository, or a CI log.

### 1. Create a Developer ID Application certificate

It must be this exact type. "Apple Development" and "Mac App Distribution"
certificates will not work for distributing outside the App Store, and the
failure mode is confusing — the build signs successfully and then notarization
rejects it.

1. In Xcode: **Settings → Accounts → Manage Certificates → + → Developer ID
   Application**. (Or create it at
   [developer.apple.com/account/resources/certificates](https://developer.apple.com/account/resources/certificates).)
2. In **Keychain Access**, find it under _My Certificates_ — it must show a
   disclosure triangle with a private key inside. Without the private key it
   cannot sign.
3. Right-click → **Export** → `.p12` format, and set a strong password. You
   will need that password in step 3.

### 2. Find your signing identity and team ID

```bash
security find-identity -p codesigning -v
```

Copy the full quoted string, which looks like:

```
Developer ID Application: Your Name (A1B2C3D4E5)
```

The 10-character code in parentheses is your team ID.

### 3. Create an app-specific password for notarization

**Not your Apple ID password.** Generate one at
[appleid.apple.com](https://appleid.apple.com) → Sign-In and Security →
App-Specific Passwords.

An app-specific password can be revoked individually and cannot be used to sign
in to your account, which is exactly what you want sitting in CI.

### 4. Set the repository secrets

```bash
./scripts/setup-macos-signing.sh ~/Desktop/flume-signing.p12
```

The script prompts for each value rather than taking it as an argument, so
nothing lands in shell history or in `ps` output.

`APPLE_SIGNING_IDENTITY` and `APPLE_TEAM_ID` are set separately, because
neither is actually a secret — both appear in plain text inside every signed
binary.

> **Why the export is manual.** `security export` cannot select a single
> identity; it exports every identity of the requested type from the keychain.
> On a machine that also has an _Apple Development_ certificate, automating it
> would ship a second private key to CI that CI has no use for. Keychain
> Access can export exactly one, so fewer keys leave the machine.

| Secret                       | Value                                                |
| ---------------------------- | ---------------------------------------------------- |
| `APPLE_CERTIFICATE`          | base64 of the `.p12` (the command above pipes it in) |
| `APPLE_CERTIFICATE_PASSWORD` | The password you set when exporting                  |
| `APPLE_SIGNING_IDENTITY`     | The full `Developer ID Application: ...` string      |
| `APPLE_ID`                   | Your Apple ID email                                  |
| `APPLE_PASSWORD`             | The **app-specific** password from step 3            |
| `APPLE_TEAM_ID`              | The 10-character code                                |

Then delete the `.p12` from disk, or move it somewhere encrypted. It is a
signing key.

### 5. That is all the wiring

The release workflow already branches on whether `APPLE_CERTIFICATE` is
non-empty. Once the secrets exist, the next tagged build takes the signed path
automatically — no workflow change needed.

`hardenedRuntime` is already enabled (it is Tauri's default) and is required
for notarization.

### If notarization fails

Notarization runs after signing and adds several minutes. Common causes:

| Symptom                                             | Cause                                                |
| --------------------------------------------------- | ---------------------------------------------------- |
| `The signature does not include a secure timestamp` | Certificate is not a Developer ID Application cert   |
| `Team is not yet configured for notarization`       | Developer Program enrolment is still processing      |
| Invalid credentials                                 | Account password used instead of an app-specific one |
| Something about JIT or unsigned executable memory   | See below                                            |

**On entitlements:** Flume deliberately ships none. The hardened runtime blocks
JIT, and a common reflex is to add `com.apple.security.cs.allow-jit` and
`allow-unsigned-executable-memory` pre-emptively. Do not. WKWebView runs
JavaScript in a separate system process with its own entitlements, so the app
usually does not need them — and both entitlements meaningfully weaken the
hardened runtime. If notarization genuinely fails for that reason, add only
`allow-jit`, and only then.

### Verifying a signed build

```bash
codesign -dv --verbose=4 /Applications/Flume.app
spctl -a -vvv -t install /Applications/Flume.app
xcrun stapler validate /Applications/Flume.app
```

The second should say `accepted` with `source=Notarized Developer ID`. The
third confirms the notarization ticket is stapled, which is what lets the app
open on a machine with no internet connection.

## Windows

**Decision: sign, via SignPath Foundation.** Applied 2026-09-10; approval is
theirs to give, so this section describes a decision made and an outcome
pending.

This reverses the earlier "deliberately unsigned" decision. The reasoning is
kept below rather than deleted, because the thing that changed is not a
preference — it is that two of the load-bearing facts stopped being true.

### Why the old reasoning is void

It rested on this: _"EV clears SmartScreen immediately — that is most of what
the extra cost buys."_ That was correct when written and is not correct now.
Microsoft removed the behaviour in 2024 and
[says so directly][smartscreen]:

> EV certificates no longer bypass SmartScreen. Years ago, signing files with
> an Extended Validation (EV) code signing certificate would result in positive
> SmartScreen reputation by default, but this behavior no longer exists.

So the old conclusion — don't buy EV — survives, but inverted. It is not that
EV costs too much for what it gives; it is that it now gives **nothing over
OV**. No certificate at any price buys instant trust outside the Microsoft
Store. "Sign so it is not flagged" is no longer a purchasable outcome.

### What made signing worth doing anyway

The old decision measured signing against a single warning on an occasional
manual download. Two things make that the wrong measure.

**Unsigned reputation restarts from zero on every release.**
[Microsoft][smartscreen]:

> When a file is not signed, SmartScreen reputation must build for each new
> version of your files, starting with zero reputation. Reputation cannot
> transfer from previous versions unless both were signed using the same
> publisher identity.

Signed with a stable identity, reputation accumulates _across_ versions.
Unsigned, every release is a new stranger. The in-place updater ([#176]) turns
that from a nuisance into the defining cost, because its whole purpose is to
ship more releases to more people.

**Smart App Control blocks rather than warns.** On Windows 11 it will refuse to
execute unsigned files without positive reputation, and it applies to every
executable rather than only downloaded ones. A dismissible warning and a
refusal to run are not the same problem.

### SignPath Foundation, and its one real catch

Free code signing for open-source projects — OV-level, key on their HSM, driven
from CI, no hardware token, no personal identification. They verify the binary
was built from the public repository. Stellarium, Flameshot, LiteDB and
GitExtensions use it, and Microsoft lists it as the open-source route.

**The certificate is issued to SignPath Foundation, not to Flume.** Their own
description: _"we verify that the binary was built from your open source
repository and vouch for that with our name."_ So the publisher string in the
UAC dialog and the SmartScreen panel reads **SignPath Foundation**. That is the
price, and it is not nothing for a BitTorrent client, where an unfamiliar third
party on the installer is not obviously more reassuring than the name that
matches the repository and the website.

The upside of the same fact is that the certificate has been signing known
software for years, and certificate reputation can carry a new file past the
warning. **Unverified** — do not plan around it until a signed build is
observed in the wild.

It also means shared fate in both directions. Negative reputation earned on
that certificate by anyone is Flume's problem too, and vice versa. P2P software
is the category most likely to attract it through no fault of the code, which
is also the likeliest reason the application is declined.

Note that the foundation is operated by SignPath GmbH, the commercial vendor.
Independence is stated as an aspiration, not a current fact.

### What was rejected

| Option                 | Why not                                                                                                                                                                                         |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Azure Artifact Signing | ~$120/year. Would work — individual tier covers the USA, and it puts Flume's own identity on the binary. Declined on cost. This is the fallback if SignPath says no                             |
| OV certificate         | $150–300/year plus a token or cloud HSM, for behaviour identical to the free option                                                                                                             |
| EV certificate         | $400+/year for behaviour identical to OV since 2024                                                                                                                                             |
| Microsoft Store (MSIX) | The only zero-warning path, and Microsoft re-signs — but Tauri emits no MSIX. Its documented Store route lists a Win32 installer, which the Store does **not** re-sign, so it buys nothing here |
| Self-signed            | Treated as worse than unsigned                                                                                                                                                                  |

### Do not start on one identity and move to another

[Microsoft][smartscreen] again: _"Use a consistent signing identity — changing
your signing certificate affects the publisher trust signal."_ Reputation
belongs to the identity, so switching discards whatever was accumulated. That
is why this was decided once rather than by trying the free option and falling
back — if SignPath declines, the fallback starts from zero either way, but
switching _after_ building reputation would throw real value away.

### Two signing systems, and they are not related

Easy to conflate and expensive to conflate:

- **Authenticode** is this section — who Windows says published the installer.
- **`TAURI_SIGNING_PRIVATE_KEY`** is the updater's minisign key ([#176]), which
  proves an update came from Flume. Different algorithm, different key,
  different purpose, no hardware requirement. Windows never looks at it and
  SmartScreen has never heard of it.

Signing Windows builds does not give Flume an updater, and shipping an updater
does not sign anything.

### The key-on-hardware problem, and why it stopped mattering

Since the CA/Browser Forum tightened requirements in June 2023, code signing
private keys must be held on FIPS 140-2 Level 2 hardware — a token or an HSM —
and CAs no longer issue an exportable `.pfx`. The macOS approach, where the
`.p12` is base64'd into a GitHub secret, is unavailable here at any price.

Both surviving options solve this by never handing over the key: SignPath and
Azure each hold it in their own HSM and sign on request from CI. An OV or EV
certificate would have meant a physical token and therefore a self-hosted
Windows runner.

[smartscreen]: https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation
[options]: https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options
[signpath]: https://signpath.org/
[#176]: https://github.com/adamgreenwell/flume/issues/176
