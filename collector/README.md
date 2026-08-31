# Flume usage collector

Validates and stores the opt-in usage counts described in
[`docs/Privacy.md`](../docs/Privacy.md). A Cloudflare Worker in front of a D1
database; about 250 lines in total.

## Why it lives in this repository

`schema.json` is the wire format. It is checked by
`src-tauri/tests/usage_contract.rs`, which serialises one of every Rust event
variant and asserts this file describes exactly that set. Splitting the two
across repositories would let them drift silently — a client sending an event
the collector drops looks like working code and loses data. Here, adding an
event to the Rust enum fails the test until this file is updated in the same
commit.

## Why validation matters more here than in the client

The client is best-effort code on someone else's machine. This is the part
that cannot be talked into storing a torrent name.

Every batch is checked against `schema.json` and **anything unrecognised is
rejected rather than ignored** — an unknown event type, an unknown field, a
value outside the allowlist, an extra key on the envelope. Silently dropping
an unexpected field is the failure mode that matters: a future client bug
would start sending one and nobody would find out until it was in the
database.

The Worker also never reads `CF-Connecting-IP` or `User-Agent`, and
`schema.sql` has no column for either.

## Deploying

```bash
npm install
npx wrangler d1 create flume-usage    # copy the id into wrangler.jsonc
npm run migrate
npm run deploy
```

Then build the app with the endpoint baked in:

```bash
FLUME_USAGE_ENDPOINT=https://<your-worker>/v1/usage npm run tauri build
```

Without that variable the client compiles with reporting inert and sends
nothing — which is the default for CI, for `cargo test`, and for anyone who
builds Flume from source without deliberately opting into it.

**That combination used to fail silently and no longer does.** A build with no
endpoint but with usage reporting switched on will queue events to disk and
send none of them, with nothing visibly wrong. It now logs a warning at
startup, and again if consent is granted later, and the diagnostics report says
`ON, BUT THIS BUILD HAS NO COLLECTOR ENDPOINT COMPILED IN`. Both paths are
covered by `should_warn` in `src-tauri/src/usage/sender.rs`.

`build.rs` declares `cargo::rerun-if-env-changed=FLUME_USAGE_ENDPOINT`, so
changing the variable forces a rebuild. Without that, setting it on an
already-built tree changes nothing: cargo sees no reason to recompile and the
previous value — including no value at all — stays baked in.

## Developing

```bash
npm run dev        # wrangler dev, on http://localhost:8787
npm run typecheck
```

A batch the collector should refuse, for checking that it does:

```bash
curl -i -X POST http://localhost:8787/v1/usage \
  -H 'content-type: application/json' \
  -d '{"schema":1,"installId":"9f1d6f2e-9a4b-4c2d-8f3a-1b2c3d4e5f60",
       "appVersion":"1.0.0","os":"macos","arch":"aarch64",
       "events":[{"at":0,"event":"launched","torrentName":"something"}]}'
```

Expected: `400`, `event launched: expected keys [at, event], got [at, event,
torrentName]`.

## Querying

```sql
-- Daily active installs.
SELECT date(at, 'unixepoch') AS day, COUNT(DISTINCT install_id) AS installs
FROM events WHERE event = 'launched' GROUP BY day ORDER BY day DESC;

-- Which errors actually fire in the wild.
SELECT value AS kind, COUNT(*) AS n
FROM events WHERE event = 'operationFailed' GROUP BY kind ORDER BY n DESC;

-- Which settings people reach for.
SELECT value AS setting, COUNT(*) AS n
FROM events WHERE event = 'settingChanged' GROUP BY setting ORDER BY n DESC;
```

Counts are approximate: delivery is at-least-once and there is no dedupe
table, so a response lost after the rows were written means a few events are
counted twice. See `docs/Privacy.md`.
