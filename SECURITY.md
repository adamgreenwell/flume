# Security Policy

## Reporting a vulnerability

**Do not open a public issue.**

Use GitHub's private reporting: the **Security** tab on this repository →
**Report a vulnerability**. That opens a private thread visible only to the
maintainer, and it is the preferred route because it keeps the disclosure and
the eventual fix in one place.

If that is unavailable to you, email **adamgreenwell@gmail.com** with `flume
security` in the subject.

Please include what you were running (version, OS), what you did, what
happened, and — if you have one — a proof of concept. A minimal reproduction is
worth more than a scanner report.

## What to expect

Flume is maintained by one person. That sets honest expectations rather than an
SLA:

- **Acknowledgement within 7 days.** If you have not heard back, assume the
  mail was lost and send it again.
- **An assessment within 30 days**, saying whether it is in scope, what the
  severity looks like, and what the plan is.
- **Credit in the release notes** when a report leads to a fix, unless you
  would rather not be named.

There is no bug bounty.

## Supported versions

Only the latest release. Flume is pre-1.0 and there are no maintenance
branches; a fix ships in the next release rather than being backported.

## Scope

**In scope** — anything in this repository:

- The Rust core in `src-tauri/`, including the engine wrapper and command
  handlers.
- The IPC boundary. Flume's central architectural rule is that torrent binary
  data never crosses it — librqbit writes pieces to disk and the webview only
  receives JSON. A way to make binary data cross that boundary, or to reach a
  command that should not be reachable, is a finding.
- Deep-link and magnet-URI handling. Input arriving from the OS is parsed
  deliberately narrowly; a way past that is a finding.
- The Tauri capability set. Flume grants minimal permissions and ships no shell
  plugin. A privilege it should not have is a finding.
- Handling of settings that carry credentials — notably the SOCKS5 proxy URL,
  which may contain a username and password.
- The frontend, where a payload from a torrent's own metadata (a file name, a
  tracker URL, a peer's client string) can reach the DOM.

**Out of scope**, and better reported upstream:

- Vulnerabilities in [librqbit](https://github.com/ikatson/rqbit), which Flume
  embeds. Report those to that project; tell us too, so the pin can be moved.
- Vulnerabilities in Tauri, WebKitGTK, WebView2 or WKWebView.
- The known GTK3 advisories on Linux, tracked in
  [#21](https://github.com/adamgreenwell/flume/issues/21). They live in Tauri's
  Linux backend and resolve when it moves off GTK3.

**Not vulnerabilities.** These are properties of BitTorrent or of the current
release, and are documented rather than hidden:

- **Peers can see your IP address.** That is how the protocol works. Flume
  offers a SOCKS5 proxy setting for outgoing peer connections; note that it
  routes peer traffic, not necessarily every lookup. If anonymity is your
  threat model, a torrent client is not the control you want.
- **DHT announces that you are in a swarm.** Disable DHT in Settings if that
  matters to you; magnet links stop working when you do.
- **Releases may be unsigned.** Builds are signed only when signing credentials
  were configured for that release, and the download page and
  [Signing & Distribution](https://github.com/adamgreenwell/flume/wiki/Signing-and-Distribution)
  both say so. An unsigned build is a distribution gap, not a vulnerability —
  but a build whose contents do not match this repository very much is one.

## Dependencies

`cargo audit` and `npm audit` run in CI, and Dependabot is enabled. If you have
found an advisory that CI is not catching, that gap is itself worth reporting.
