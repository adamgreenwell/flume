## What this changes

<!-- A short description of the change and why it is needed. -->

Closes #

## How it was verified

<!-- Tests added, manual steps taken, platforms checked. -->

- [ ] `npm run check` passes
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` pass
- [ ] Verified manually on: <!-- macOS / Windows / Linux -->

## Architecture checklist

<!-- Delete any line that does not apply to this change. -->

- [ ] No torrent binary data crosses the IPC boundary
- [ ] Engine layer still imports no Tauri types
- [ ] Rust `serde` types and their TypeScript mirrors were changed together
- [ ] No new Next.js server features (static export still builds)
- [ ] Any new Tauri permission is the minimum the feature needs
- [ ] Public items have rustdoc / JSDoc

## Notes for the reviewer

<!-- Anything non-obvious, or a decision you would like a second opinion on. -->
