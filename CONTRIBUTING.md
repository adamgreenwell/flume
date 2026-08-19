# Contributing to Flume

Thanks for your interest in Flume. This document covers how to get set up, the
standards the codebase holds to, and how changes get merged.

## Getting set up

**Prerequisites:** Rust stable (1.88+), Node.js 22+, and your platform's
[Tauri system dependencies](https://tauri.app/start/prerequisites/).

```bash
npm install
npm run tauri:dev
```

On Debian/Ubuntu you will also need:

```bash
sudo apt-get install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libxdo-dev
```

## Architecture rules

These are not style preferences. Violating one is a design defect, and a review
will ask you to change it.

1. **No torrent binary data over IPC.** librqbit writes pieces to disk. The
   webview receives JSON only — progress, speeds, counts, file lists.
2. **The engine layer imports no Tauri types.** `src-tauri/src/engine/` must
   compile and be testable under plain `cargo test`.
3. **Command handlers are thin.** Logic worth testing belongs in the engine.
4. **No Next.js server features.** Static export only: no `app/api/*`, no
   middleware, no server-only functions. All backend calls go through `invoke`.
5. **Telemetry is throttled to roughly 1 Hz** and batched. No per-piece events.
6. **Shared types stay in sync.** A change to a `serde` struct in Rust changes
   its TypeScript mirror in `src/lib/ipc/types.ts` _in the same commit_.
7. **Minimal Tauri permissions.** Grant only what a feature needs. The shell
   plugin is not to be added without a discussion.

## Standards

**Rust**

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` must be clean.
- Rustdoc `///` on every public item, with `# Errors` / `# Panics` where they apply.
- Prefer returning typed errors over `unwrap`/`expect` in production paths;
  both are lint-warned. In tests they are fine and explicitly allowed.

**TypeScript**

- Strict mode; `tsc --noEmit`, ESLint, and Prettier must all be clean.
- JSDoc on exported functions; typed props on every component.

**Tests**

- Backend: unit tests beside the code, integration tests in `src-tauri/tests/`.
  Network-dependent tests are `#[ignore]`d so CI stays deterministic.
- Frontend: Vitest, using `mockIPC` from `@tauri-apps/api/mocks` rather than a
  live backend.

Run everything CI runs:

```bash
npm run check
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## Commits and pull requests

Flume uses [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(engine): add per-torrent rate limiting
fix(ui): stop the progress bar flickering at 100%
docs: explain the DHT bootstrap timeout
```

Types in use: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, `perf`, `ci`,
`build`.

**Keep commits small and focused.** One logical change each. The history is
meant to be readable — a reviewer should be able to follow the reasoning
commit by commit rather than decoding one large drop.

Work happens on feature branches and lands via pull request. Reference the
issue a change closes (`Closes #12`).

## Reporting bugs

Open an issue using the bug template. Please include your OS and version, the
Flume version, and what you expected versus what happened. If it involves a
specific torrent, the tracker or a magnet link for a **legal, publicly
distributed** file (a Linux ISO is ideal) helps enormously.

## Scope

Flume deliberately does not do: cryptocurrency, chat, RSS automation, or paid
features. Proposals that expand scope should start as an issue for discussion
before any code.
