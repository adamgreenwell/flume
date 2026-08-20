# Signing & Distribution

What it takes to ship Flume so that a user's operating system does not treat it
as suspicious, and what to tell them when it does.

> **Current state: builds are unsigned.** Signing is optional in the release
> pipeline — the secrets are read if present and skipped if not — so anyone can
> build Flume without certificates. Everything below is what changes when
> real credentials exist.

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

## Windows

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

This is worth doing from the first release. It is free, and it is the only
verification available to users of unsigned builds.

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
