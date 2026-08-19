# Development Setup

## Prerequisites

- **Rust** stable, 1.88 or newer
- **Node.js** 22 or newer
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

Backend, from `src-tauri/`:

| Command                                     | What it does                              |
| ------------------------------------------- | ----------------------------------------- |
| `cargo test`                                | Unit and integration tests (offline only) |
| `cargo test -- --ignored`                   | Also run network-dependent DHT tests      |
| `cargo clippy --all-targets -- -D warnings` | Lint, warnings are errors                 |
| `cargo fmt`                                 | Format                                    |

## Testing strategy

**Backend.** Unit tests live beside the code. Integration tests in
`src-tauri/tests/engine.rs` drive a real `librqbit::Session` with no Tauri
runtime — this is why the engine layer must not import Tauri types.

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
