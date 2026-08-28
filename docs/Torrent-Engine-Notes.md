# Torrent Engine Notes (librqbit)

Flume pins **librqbit 9.0.0** (released 2026-08-15, Apache-2.0).

## Why librqbit

It is designed as an embeddable library rather than a CLI with a library
bolted on. Its author maintains an `Api` facade returning serde-serializable
stats, which is exactly the shape a GUI needs. The `rqbit` repository also
contains a `desktop/` Tauri app — useful as reference, but Flume's design
diverges (Next.js frontend, different UX), so it is not copied.

## Cargo configuration

```toml
librqbit = { version = "9.0.0", default-features = false, features = ["rust-tls"] }
```

`default-features = false` drops two defaults:

- **`default-tls`** → `reqwest/native-tls` → OpenSSL. Linking system OpenSSL
  makes `.deb`/`.rpm` packages depend on a matching `libssl` at runtime, which
  is a classic "works on Ubuntu, fails on Fedora" trap.
- **`http-api-client`** → a client for talking to a _remote_ rqbit instance.
  Flume embeds the engine in-process, so this is dead weight.

`rust-tls` substitutes rustls plus ring-backed SHA-1. Verified result: the
lockfile contains no `openssl` or `openssl-sys`. The only match for "openssl"
is `openssl-probe`, a pure-Rust crate that merely locates CA bundle paths.

## Flume runs a patched librqbit

`src-tauri/Cargo.toml` also carries a `[patch.crates-io]` entry pointing at
[`adamgreenwell/rqbit`](https://github.com/adamgreenwell/rqbit), pinned to a
revision. A fresh `cargo build` therefore fetches librqbit from GitHub rather
than crates.io, which is expected rather than a misconfiguration.

The patch adds three things to `PeerStats`, all needed to answer "will this
torrent finish?":

| Addition                                 | Why                                                                                  |
| ---------------------------------------- | ------------------------------------------------------------------------------------ |
| `have_pieces`                            | How many pieces a peer holds, clamped to `total_pieces`.                             |
| `have_bitfield`                          | The bitfield itself, opt-in and off by default.                                      |
| `PeerStats`/`PeerStatsFilter` re-exports | They were a return type and a parameter of a public method, inside a private module. |

**A count alone is not enough**, which matters if anyone is tempted to shrink
the patch. A per-peer count gives the _mean_ copies per piece; the verdict needs
the _minimum_. Two peers holding 500 pieces each may overlap completely or not
at all — identical counts, and only one of those torrents can finish.

**Two constraints keep builds working:**

- **Do not delete the fork's `peer-availability` branch, or the fork.**
  `Cargo.lock` pins the full commit SHA, so a force-push cannot change what is
  built — but the commit still has to remain reachable. Delete the branch and it
  becomes eligible for garbage collection, and every build fails on fetch.
- **The fork must stay public.** CI clones it anonymously.

Upstream ask is [`ikatson/rqbit#643`](https://github.com/ikatson/rqbit/issues/643), sent as [`#644`](https://github.com/ikatson/rqbit/pull/644).
Delete the `[patch.crates-io]` section the moment it lands in a crates.io
release.

## v8 → v9 API changes

**Do not copy v8-era examples or gists.** The reorganisation is real.

| Concern       | v8 (approx.)                             | v9                                                   |
| ------------- | ---------------------------------------- | ---------------------------------------------------- |
| Disabling DHT | `disable_dht: bool`                      | `dht: Option<DhtSessionConfig>` — `None` disables    |
| Listen port   | `listen_port_range`                      | `listen: Option<ListenerOptions>` with `listen_addr` |
| UPnP          | `enable_upnp_port_forwarding` on session | Moved onto `ListenerOptions`                         |

### Gotcha: `listen` defaults to `None`

`SessionOptions::default()` sets `listen: None`, which means **no incoming
connections and therefore no seeding**. This is easy to miss because downloads
still work. Flume always sets it explicitly.

### Gotcha: DHT persistence defaults to a global path

`DhtPersistenceConfig.config_filename: None` means "OS-specific default" — a
single global file shared by every instance on the machine, _not_ the session
directory you passed to `SessionOptions`.

Two consequences: DHT state escapes your session directory, and because
`dht_listen_addr` resolves its port as `explicit -> stored -> random`, a second
instance tries to bind the same persisted UDP port and fails with
`Address already in use`.

Flume sets it explicitly to `<session_dir>/dht.json`. See issue #19.

### There are no feature flags for DHT, UPnP, or torrent v2

The original project brief assumed these were opt-in cargo features. They are
not — all are unconditionally compiled in. `librqbit`'s actual optional
features are: `default-tls`, `rust-tls`, `http-api`, `http-api-client`,
`postgres`, `prometheus`, `watch`, `webui`, `storage_middleware`,
`upnp-serve-adapter`, `tokio-console`, `tracing-subscriber-utils`,
`disable-upload`, `async-bt`, and a few internal test flags.

Note that `upnp-serve-adapter` is a **DLNA/UPnP media server**, not port
forwarding — a genuinely confusing name collision.

## Integration surface used by Flume

| Call                                              | Purpose                                          |
| ------------------------------------------------- | ------------------------------------------------ |
| `Session::new_with_opts(PathBuf, SessionOptions)` | Start the session                                |
| `session.get_dht() -> Option<&Dht>`               | DHT handle; `dht.stats()` for routing table size |
| `session.listen_addr() -> Option<SocketAddr>`     | Actual bound peer port                           |
| `session.announce_port() -> Option<u16>`          | Port announced to trackers                       |
| `session.stats_snapshot()`                        | Speeds, peer counts, uptime, byte counters       |
| `session.client_name_and_version()`               | Client identification string                     |
| `session.stop()`                                  | Graceful shutdown, flushes persistence           |

Types to know:

- `Speed` is `{ mbps: f64 }` with `.as_bytes() -> u64`. Flume converts to raw
  bytes/sec at the boundary rather than exposing it over IPC.
- `DhtStats` is `{ id, outstanding_requests, routing_table_size, routing_table_size_v6 }`.
- `AggregatePeerStats` has a `live` field directly, alongside per-transport
  `live_tcp` / `live_utp` / `live_socks`.

## Upgrading

1. Read the crate's changelog and diff `SessionOptions` first — it is where
   breaking changes concentrate.
2. Read the actual source, not documentation snippets:
   `~/.cargo/registry/src/index.crates.io-*/librqbit-<version>/`
3. Run `cargo test -- --ignored` to exercise the live DHT path, which catches
   bootstrap and binding regressions that offline tests cannot.
4. Verify the lockfile still has no OpenSSL: `grep openssl src-tauri/Cargo.lock`
   should only match `openssl-probe`.
5. Check whether the patched `PeerStats` fields have landed upstream. If they
   have, drop the `[patch.crates-io]` section and rebase off the fork; if they
   have not, rebase the fork onto the new tag before bumping, since a patch
   whose version no longer matches the requirement is silently ignored — cargo
   warns `patch ... was not used in the crate graph` and the build then fails on
   the missing fields.

## Known platform issue: Windows file locking

Reported behaviour: on Windows, another process holding a handle to a file can
block seeding. A community client ("Drift") patched librqbit's storage layer
for this.

**Status: unverified on v9.** Tracked in issue #9. Confirm it reproduces before
patching or vendoring anything — it may already be fixed.
