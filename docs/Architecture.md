# Architecture

## System overview

```text
┌──────────── Tauri v2 WebView (Next.js static export) ─────────────┐
│  Themeable React/Tailwind SPA  ⇄  @tauri-apps/api: invoke/listen  │
└───────────────────┬──────────────────────────▲────────────────────┘
                    │ commands (JSON only)     │ events (JSON, ~1 Hz)
                    ▼                          │
┌───────────────────┴──────────────────────────┴────────────────────┐
│  Rust core (tokio runtime)                                        │
│                                                                   │
│   src-tauri/src/engine/    thin librqbit::Session wrapper.        │
│                            No Tauri types. Unit testable.         │
│   src-tauri/src/commands/  #[tauri::command] handlers. Thin.      │
│   src-tauri/src/state/     app state and persistence.             │
└───────────────────┬───────────────────────────────────────────────┘
                    │ pieces written directly
                    ▼
              ┌───────────┐
              │   Disk    │
              └───────────┘
```

## The rules

These are structural guarantees, not conventions.

### 1. The webview never touches torrent binary data

librqbit writes pieces straight to disk. The UI receives only small JSON
payloads: progress percentages, transfer rates, peer counts, file listings.

This is what keeps memory bounded on a multi-gigabyte ISO. Routing piece data
through IPC would mean serialising it, copying it into the WebView heap, and
holding it there — which is how torrent clients end up consuming gigabytes of
RAM on a large download.

### 2. The engine layer imports no Tauri types

`src-tauri/src/engine/` compiles and runs under plain `cargo test`. It is the
layer most exposed to librqbit churn, so it must be testable without spawning a
WebView. See `src-tauri/tests/engine.rs`, which drives a real `Session` with no
Tauri runtime present.

### 3. Command handlers are thin

Handlers unwrap shared state, call the engine, and map errors. Anything worth
testing lives in the engine, where it can be tested.

### 4. Telemetry is throttled and batched

Status updates run at approximately 1 Hz. Per-piece events would flood the IPC
channel on a fast download and jank the UI for no informational gain.

### 5. Flume owns its IPC types

The types crossing the boundary are defined in `src-tauri/src/engine/status.rs`
— they are _not_ re-exports of librqbit's internal stats structs. A librqbit
upgrade therefore cannot silently change the contract the frontend depends on;
the compiler forces a look at the mapping instead.

## The IPC contract

Rust `serde` structs use `#[serde(rename_all = "camelCase")]`. Their TypeScript
mirrors live in `src/lib/ipc/types.ts` and must change in the same commit.

### `get_core_status`

Returns `CoreStatus`.

| Field           | Type             | Meaning                                  |
| --------------- | ---------------- | ---------------------------------------- |
| `clientVersion` | `string`         | Client string, e.g. `"Flume 0.1.0"`      |
| `listenPort`    | `number \| null` | Bound peer port, `null` if not listening |
| `announcePort`  | `number \| null` | Port announced to trackers               |
| `dht`           | `DhtStatus`      | DHT subsystem health                     |
| `downloadDir`   | `string`         | Absolute download path                   |
| `uptimeSeconds` | `number`         | Seconds since session start              |
| `downloadBps`   | `number`         | Aggregate download rate, bytes/sec       |
| `uploadBps`     | `number`         | Aggregate upload rate, bytes/sec         |
| `livePeers`     | `number`         | Connected peers across all torrents      |
| `health`        | `EngineHealth`   | Derived readiness indicator              |

`DhtStatus`: `{ enabled, nodesV4, nodesV6, outstandingRequests }`.

`EngineHealth`: `"starting" | "connecting" | "ready" | "degraded"`.

- `starting` — the peer listener has not bound a port yet
- `connecting` — listening, DHT enabled, routing table below 8 nodes
- `ready` — listening with a usable DHT routing table
- `degraded` — listening, but DHT is disabled, so magnet links cannot resolve

### Errors

Commands reject with `CommandError`:

```ts
{
  kind: string;
  message: string;
}
```

`kind` is a stable machine-readable identifier so the frontend can branch
without matching on message text. Currently: `engineNotReady`.

## Startup sequence

1. Tauri builds the app and registers `AppState` and the command handlers.
2. The window opens **immediately** — first paint is never blocked.
3. A background task derives `EngineConfig` from OS conventions and starts the
   session (DHT bootstrap, listener bind, persistence restore).
4. Until the engine is ready, `get_core_status` returns `engineNotReady` and the
   UI shows a `starting` state.
5. On `RunEvent::Exit`, session shutdown is awaited so fast-resume state is
   flushed and a restart resumes rather than re-hashing.

## Where state lives

| What                       | Where                                                                               |
| -------------------------- | ----------------------------------------------------------------------------------- |
| Downloads                  | User's Downloads folder by default (configurable)                                   |
| Session state, fast-resume | OS app-data dir, e.g. `~/Library/Application Support/io.github.adamgreenwell.Flume` |
| DHT routing table          | `dht.json` inside the session directory                                             |

The DHT path is set explicitly. librqbit's default is a **global** OS path
shared across instances, which both leaks state and causes port collisions
between two running copies — see issue #19.
