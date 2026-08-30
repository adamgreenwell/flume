# Signing & Distribution

What it takes to ship Flume so that a user's operating system does not treat it
as suspicious, and what to tell them when it does.

> **Current state: builds are unsigned.** The release pipeline selects between
> a signed and an unsigned build depending on whether `APPLE_CERTIFICATE` is
> set, so anyone can build Flume without certificates. See
> [Setting up macOS signing](#setting-up-macos-signing) to enable it.

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

**Decision: Flume does not sign Windows builds.** Taken deliberately; the
reasoning is below so it does not get re-argued from scratch.

### Why not

**An OV certificate would change nothing at Flume's download volume.**
SmartScreen reputation accrues _per certificate_, from downloads. A new OV
certificate starts at zero, so users keep seeing the warning while the
certificate is paid for. EV clears SmartScreen immediately — that is most of
what the extra cost buys — but it is several hundred a year for a free app.

This is the trap worth naming, because it inverts the Apple intuition:
**Apple sells trust, Microsoft makes you earn it.** A Developer ID certificate
silences Gatekeeper from the first download. An OV certificate is a ticket to
_start accumulating_ reputation, not the reputation itself.

**The private key cannot live in a GitHub secret.** Since the CA/Browser Forum
tightened requirements in June 2023, code signing keys must be held on FIPS
140-2 Level 2 hardware — a USB token or an HSM. Certificate authorities no
longer issue an exportable `.pfx`. So the approach that works for macOS, where
the `.p12` is base64'd into a secret, is not available here at any price.

### If this is revisited

[Azure Trusted Signing][ats] is the option that would fit: roughly $10/month,
key in Microsoft's HSM, signing driven by an API, so CI keeps working and there
is no token to plug in. Two things to confirm _before_ paying, because they
decide whether it is worth anything:

1. **Does it confer SmartScreen reputation immediately, or accrue it like OV?**
   If the latter, it buys a valid signature and the same warning.
2. **Identity validation.** Historically the friction was proving
   organisational history.

Certificates from it are short-lived and rotate every few days. That is by
design and safe: signatures are timestamped, and a timestamped signature stays
valid after its certificate expires.

An EV certificate plus a self-hosted Windows runner also works, and costs real
money and real operational burden.

[ats]: https://learn.microsoft.com/azure/trusted-signing/

### Costs, for reference

| Requirement                 | Cost                                         |
| --------------------------- | -------------------------------------------- |
| OV code signing certificate | ~$200-400/year                               |
| EV code signing certificate | ~$300-600/year, often needs a hardware token |

The difference matters more than the price. **SmartScreen reputation** is built
per-certificate from download volume: an OV certificate starts with none, so
early users still see warnings until enough downloads accumulate. An EV
certificate gets reputation immediately.

For a project with modest download numbers, an OV certificate can warn users
for a long time. That is worth knowing before spending the money.

### What a user sees without it

A blue **"Windows protected your PC"** dialog. The **Run anyway** button is
hidden behind **More info**, which is deliberate and catches people out.

Some browsers also flag the download itself as untrusted.

## Linux

No signing gate. Users install a `.deb` or `.rpm` and it works.

If Flume is ever published to a repository, packages are signed with a GPG key
and users import the public key — a different model, and a much cheaper one.

The AppImage format supports embedded signatures, but almost nothing verifies
them, so it buys little.

## Checksums, which cost nothing

Whatever happens with certificates, publishing SHA-256 checksums alongside
releases lets a careful user verify their download:

```bash
shasum -a 256 -c flume_0.1.0_aarch64.dmg.sha256
```

This is free, and it is the only verification available to users of unsigned
builds. The release workflow publishes `SHA256SUMS.txt` covering every asset:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

It is generated by downloading the assets back off the release rather than from
the build artifacts, so the checksums describe what a user actually gets.

## Recommendation

1. **Ship unsigned first**, with checksums and clear instructions. Wait for
   real users before spending money.
2. **macOS signing is the highest-value purchase** if demand appears: the
   gatekeeping is the most aggressive and the fix is the most obscure.
3. **Windows EV over OV**, or neither. An OV certificate that still shows
   SmartScreen warnings for months is the worst value of the three options.
4. **Never work around gatekeeping in the installer.** Any instruction that
   disables a security feature globally, rather than for this one app, is worse
   than the warning.
