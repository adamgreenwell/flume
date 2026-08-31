# Privacy

Flume collects nothing by default, and asks once whether it may collect
anything at all.

This page is the whole answer. If something here is vague, that is a bug —
open an issue.

## What Flume never collects

Not with your consent, not without it, not in a diagnostics report, not in a
log file that leaves your machine:

- What you download. No torrent names, file names, info hashes, or magnet
  links.
- Who you download it from. No tracker addresses, peer IP addresses, or DHT
  node addresses.
- Where you put it. No file paths, folder names, or drive names.
- Who you are. No account, no email address, no name, no machine identifier,
  no hardware serial, no MAC address, no IP address stored by the collector.
- What you type. No search terms, no settings values.

There is no free-text field anywhere in the wire format. Every value Flume can
send is one of a fixed list, enumerated below and enforced twice — once by the
Rust type in `src-tauri/src/usage/mod.rs`, and again by the collector, which
rejects any batch containing a field or value it does not recognise.

## The two features

### Diagnostics report — nothing is sent

**Settings → Privacy → Diagnostics report.**

Builds a report about this install for you to paste into a bug report. It is
shown on screen first, and you copy it yourself. Flume does not send it
anywhere and has no way to.

It contains Flume's version, your OS and CPU type, whether the listen port
bound, how many DHT nodes were found, how many torrents are in your library
(the count, never the list), your settings described rather than quoted — the
download folder appears as "exists, inside the home directory", a proxy as
"configured (socks5)" — and the last 200 lines of the current session's log
with paths, addresses, URLs, info hashes and torrent names removed.

**One honest limitation.** Redaction removes torrent names by matching them
against the torrents currently in your library. A log line naming a torrent
you have already removed has nothing to match against and can survive. That is
why the report is shown before it is copied: you are the only person who can
recognise it. Read it before you paste it.

### Usage counts — opt-in, off unless you say yes

**Settings → Privacy → Send anonymous usage counts.**

Asked once during first run and off unless you turn it on. Declining is
permanent; Flume does not ask again.

If you turn it on, Flume sends, at most once an hour and once when you quit:

| Field        | Value                                                      |
| ------------ | ---------------------------------------------------------- |
| `installId`  | A random UUID generated on your machine when you consented |
| `appVersion` | Flume's version, e.g. `1.0.0`                              |
| `os`         | `macos`, `windows` or `linux`                              |
| `arch`       | `x86_64` or `aarch64`                                      |
| `events`     | The list below                                             |

`installId` is random. It is **not** derived from your hardware, your network,
your username or anything else — it is a UUID with no relationship to you or
your machine, so it cannot be linked to you or correlated with any other
application. Turning the setting off deletes it, along with anything queued
and not yet sent. Turning the setting back on generates a new one.

Every event is timed to the hour, not the second.

| Event              | Carries                                                        |
| ------------------ | -------------------------------------------------------------- |
| `launched`         | nothing                                                        |
| `sessionEnded`     | how long Flume ran, as one of five ranges                      |
| `libraryCount`     | how many torrents, as one of five ranges                       |
| `torrentPreviewed` | `magnet` or `file`                                             |
| `torrentAdded`     | nothing                                                        |
| `torrentCompleted` | nothing                                                        |
| `torrentRemoved`   | whether the files were deleted too                             |
| `libraryImported`  | how many torrents came from another client, as a range         |
| `settingChanged`   | which setting, e.g. `net.proxy` — **never what you set it to** |
| `operationFailed`  | which class of error, e.g. `metadataTimeout`                   |

Counts are ranges rather than exact numbers because a range is what anyone
would graph, and "the install with 1,483 torrents" is one identifiable person.

### What the collector stores

The collector runs on Cloudflare Workers and writes to a D1 database. Its
source is in `collector/` in this repository — the whole thing is about 250
lines and you can read it.

It stores exactly the fields listed above. It does **not** store your IP
address or your `User-Agent`; the code never reads either, and the database
schema has no column for them. Requests reach Cloudflare, which sees your IP
as any web server would, but nothing in Flume's control records it.

Counts are approximate. Delivery is at-least-once, so a response lost after a
row was written means a handful of events are counted twice. For aggregate
counters that is a better trade than the complexity of exact-once delivery,
and overstating the precision would be worse than the imprecision.

## Turning it off

Settings → Privacy → **Send anonymous usage counts** → off.

Immediately, with no restart: the install ID file and the queue of unsent
events are both deleted from your machine. Data already received cannot be
tied back to you — the ID is gone from your side and was never linked to you
in the first place — so there is nothing to request the deletion of.

## Network connections Flume makes

Whether or not you consent to usage counts, Flume connects to:

- **Trackers** listed in the torrents you add.
- **Peers**, directly or through your configured SOCKS5 proxy.
- **The DHT**, if enabled, which is how magnet links work at all.
- **Your router**, if UPnP is enabled, to request a port mapping.

That is the complete list. Flume has no update checker, no crash reporter, no
analytics SDK, no font or asset fetching — the fonts are vendored, so the
interface renders with no network at all — and no bundled search.

The interface itself cannot make network requests. Its Content Security Policy
allows it to talk to the Rust backend and nothing else, so every connection
above originates in Rust where it can be audited in one place.

## Changes to this page

The wire format is versioned (`schema: 1`). A change to what is collected
means a new schema version, a change to this page, and a change to the
consent text — in the same commit, because
`src-tauri/tests/usage_contract.rs` fails until the client and the collector
agree.
