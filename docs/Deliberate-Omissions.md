# Deliberate Omissions

Features other clients have that Flume is **not** planning to build, and why.

The point of writing them down is that an absence looks the same whether it was
decided or forgotten. Someone comparing Flume against Transmission or Deluge
should be able to tell which of these is a judgement they might disagree with,
and which is a wall nobody has found a way through yet.

Three different kinds of "no" are mixed together below, and the difference
matters more than the list does:

| Kind                 | What it means                                                |
| -------------------- | ------------------------------------------------------------ |
| **Blocked upstream** | Flume would build it; librqbit does not expose what it needs |
| **Declined**         | Reachable, and the cost is judged not worth paying           |
| **Out of scope**     | A different product, not a missing feature                   |

A blocked item can change without anyone changing their mind. A declined one
cannot.

## Blocked upstream

Tracked together in
[#196](https://github.com/adamgreenwell/flume/issues/196), which is the live
record of what Flume is asking librqbit for and how likely each ask is.

### Granular file priority

Transmission offers high / normal / low per file. librqbit exposes only
include and exclude, which Flume already has as the file-selection step.

The type that would carry it, `FilePriorities`, is `pub(crate)`, and there is
no setter. `FileInfo::iter_piece_priorities` is public, but it takes no
argument and decides nothing — it walks a file's pieces in a fixed order, so
it is not a lever even though it is reachable.

Flume cannot approximate this from outside. Priority affects piece selection
inside the engine's request pipeline, and that pipeline is private.

### Protocol encryption preferences

Most clients offer require / prefer / disable for peer protocol encryption.
librqbit surfaces no control over it — nothing in its session or peer
connection options mentions it — so there is no setting for Flume to pass
through and nothing to report about what is in force.

Worth being precise about what that does and does not mean: this is about
_obfuscation between peers_, which is a different thing from the transport
encryption a VPN gives you. Flume's answer to "can my ISP see what this is" is
the [[Privacy]] page and the tunnel guard, not this setting.

## Declined

### Remote access and a web UI

Transmission's daemon plus web UI is central to how a lot of people use it, and
librqbit ships an `http-api` cargo feature that would provide most of it.

Declined anyway, and this is the one most likely to be worth arguing about.
Flume's architecture commits to no HTTP or control API, minimal Tauri
permissions, and a locked-down webview CSP. Worth being exact about that
first one, since this is a paragraph about network exposure: Flume _does_
listen, on the BitTorrent port it binds deliberately so it can accept
incoming peers and seed, and that port is a setting. What it has no listener
for is anything that can be told what to do.

Adding an HTTP server — even bound to localhost — is a different product with
a different threat model: it is a way in that takes instructions rather than
pieces, it needs its own authentication story, and it turns "what can reach
the engine" from a settled question into an ongoing one.

The honest version of this decision is that it is about scope and attention
rather than about security being unachievable. A client with a remote API is a
reasonable thing to want; it is not the thing being built here. If headless use
ever becomes a goal it should be decided explicitly, not arrive as a flag
somebody turned on.

## Out of scope

Cryptocurrency, chat, RSS and a paid tier are named as anti-goals on [[Home]]
and in the README; this page is where the reasoning lives. The plugin system
is not listed in either, and is written down here for the first time — the
absence of a line about it was exactly the gap this page exists to close.

### RSS feeds

Deluge and qBittorrent both have it. It is automation on top of a client rather
than a client feature, and it pulls in a scheduler, a dedupe story, and a
filter language. Flume has no ambition to be the thing that decides what to
download.

### A plugin system

Deluge's defining feature, and genuinely good. It implies a stable public API,
a sandboxing model, a distribution channel and a trust story about third-party
code running inside a process that talks to the network. Each of those is a
larger commitment than any feature a plugin would add.

### Cryptocurrency, chat, bundled search, a paid tier

Named for completeness because people ask. None of them is a BitTorrent client.

## Changing any of this

A blocked item changes when librqbit exposes what it needs — watch
[#196](https://github.com/adamgreenwell/flume/issues/196) rather than this
page.

A declined or out-of-scope item changes by someone making the argument in an
issue and the decision being revisited on purpose. The failure mode this page
exists to prevent is one of these quietly becoming a feature because it was
easy in the moment, which is how a focused client stops being one.
