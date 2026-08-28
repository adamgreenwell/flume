# Development Setup

## Prerequisites

- **Rust** stable, 1.88 or newer
- **Node.js** 26, as pinned in `.nvmrc` — `nvm use` or `fnm use` picks it up.
  `npm run check` refuses to run on anything older and says why, because below
  Node 22 the test suite fails as an unreadable vitest worker crash rather than
  as a version error
- Platform system dependencies (below)

Install Rust via [rustup](https://rustup.rs).

### macOS

Xcode Command Line Tools:

```bash
xcode-select --install
```

### Windows

- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
  with the "Desktop development with C++" workload
- WebView2 runtime (preinstalled on Windows 11 and current Windows 10)

### Debian / Ubuntu

```bash
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev \
  librsvg2-dev patchelf build-essential curl wget file libxdo-dev libssl-dev
```

### Fedora / RHEL / Rocky / Alma

```bash
sudo dnf install -y webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel gcc gcc-c++ make
```

## Running

```bash
npm install
npm run tauri:dev
```

This starts `next dev` on port 3000 and then builds and launches the Rust
binary, which loads the dev server in the WebView. The first Rust build
compiles librqbit and takes several minutes; subsequent builds are seconds.

## Scripts

| Command               | What it does                                                 |
| --------------------- | ------------------------------------------------------------ |
| `npm run tauri:dev`   | Run the desktop app with hot reload                          |
| `npm run tauri:build` | Produce a production bundle for the current OS               |
| `npm run dev`         | Frontend only, in a browser (IPC calls will fail — expected) |
| `npm run check`       | Typecheck + lint + format-check + test                       |
| `npm run test:watch`  | Vitest in watch mode                                         |
| `npm run storybook`   | Component harness on :6006, both themes, axe                 |

Backend, from `src-tauri/`:

| Command                                     | What it does                              |
| ------------------------------------------- | ----------------------------------------- |
| `cargo test`                                | Unit and integration tests (offline only) |
| `cargo test -- --ignored`                   | Also run tests needing network or a proxy |
| `cargo clippy --all-targets -- -D warnings` | Lint, warnings are errors                 |
| `cargo fmt`                                 | Format                                    |

## Testing strategy

**Backend.** Two layers of integration test, each proving something the other
cannot:

- `src-tauri/tests/engine.rs` drives a real `librqbit::Session` with no Tauri
  runtime at all — this is why the engine layer must not import Tauri types.
- `src-tauri/tests/commands.rs` drives commands through Tauri's **mock runtime**,
  so `#[tauri::command]` registration, state injection, and the `serde` round
  trip are exercised without a WebView. It asserts the presence of every
  camelCase key in the payload, because a `serde` rename would otherwise break
  the frontend silently — the compiler cannot check across the IPC boundary.

Unit tests live beside the code.

Tests that need the internet are `#[ignore]`d so CI stays deterministic. Run
them before any librqbit upgrade.

**Frontend.** Vitest with `mockIPC` from `@tauri-apps/api/mocks`, so tests
never need a running backend:

```ts
import { mockIPC } from "@tauri-apps/api/mocks";

mockIPC((cmd) => {
  if (cmd === "get_core_status") return sampleStatus;
  throw new Error(`unexpected command: ${cmd}`);
});
```

## Debugging

**Frontend.** Right-click → Inspect Element in the dev build opens devtools.

**Backend.** `tracing` output goes to the terminal running `tauri:dev`. Raise
verbosity with `RUST_LOG`:

```bash
RUST_LOG=librqbit=debug,flume_lib=debug npm run tauri:dev
```

Note librqbit is verbose at `debug`; scope it to the module you care about.

**Running the frontend alone.** `npm run dev` and open `localhost:3000`. The UI
renders and the error path is exercised, because `invoke` is unavailable
outside the WebView. Useful for pure layout work.

## Gotchas

- **Next.js rewrites `CLAUDE.md`.** Next 16 regenerates agent files on every
  `next dev`. Disabled via `agentRules: false` in `next.config.ts`.
- **ESLint and Rust build output.** `src-tauri/target/` contains generated JS
  shims; it is in the ESLint ignore list.
- **Port 42221.** The default listen port. If something else holds it, the
  session start fails; the engine logs the error and the UI stays in
  `starting`.
- **Two instances collide.** By design each instance wants the same listen
  port and session directory. Use a separate session directory to run two.

## Testing the SOCKS5 proxy setting

Verifying that a proxy setting _works_ needs a proxy, and most people do not
have one lying around. `scripts/socks5-test-proxy.py` is a disposable one —
standard library only, no dependencies, no install.

```bash
python3 scripts/socks5-test-proxy.py     # listens on 127.0.0.1:1080
```

It logs every connection, which is the point: seeing real peer addresses appear
proves traffic is going through the proxy rather than merely being accepted by
validation and then ignored.

Then either set the proxy in Settings and add a torrent, or run the test that
does it automatically:

```bash
cargo test --manifest-path src-tauri/Cargo.toml \
  --test engine peer_connections_go_through -- --ignored --nocapture
```

A successful run looks like this in the proxy's output:

```
  #146  CONNECT 158.173.21.94:32419
  #147  CONNECT 14.155.232.52:1111
  #148  CONNECT 73.14.216.61:15675
```

Those are BitTorrent peers on their own ports, reached through the proxy.

The server implements only the no-auth handshake and `CONNECT` from RFC 1928.
That is all librqbit needs, and deliberately not more — it is a test fixture,
not something to leave running.
