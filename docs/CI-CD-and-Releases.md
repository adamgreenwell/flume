# CI/CD and Releases

## Continuous integration

`.github/workflows/ci.yml` runs on every push to `main`, every pull request,
and on manual dispatch. Concurrency is grouped per ref, so a newer push cancels
an in-flight run.

### `lint-test`

Runs on `ubuntu-22.04` — deliberately the oldest supported Linux base, since
it sets the glibc/WebKitGTK floor for `.deb` builds.

**Frontend**

| Step   | Command                |
| ------ | ---------------------- |
| Types  | `npm run typecheck`    |
| Lint   | `npm run lint`         |
| Format | `npm run format:check` |
| Tests  | `npm run test`         |
| Export | `npm run build`        |

The export step is followed by an assertion that `out/index.html` exists and
that no server runtime code leaked into the bundle. This guards architecture
rule 4 (static export only) mechanically rather than by review.

**Backend**

| Step   | Command                                     |
| ------ | ------------------------------------------- |
| Format | `cargo fmt --check`                         |
| Lint   | `cargo clippy --all-targets -- -D warnings` |
| Tests  | `cargo test`                                |

Warnings are errors. Network-dependent tests are `#[ignore]`d so runs stay
deterministic; run them locally before a librqbit upgrade.

### `audit`

`cargo audit` and `npm audit --omit=dev`, in a separate job so a new advisory
does not mask a real test failure.

## Dependency updates

Dependabot opens weekly PRs for cargo, npm, and GitHub Actions. Related
packages are grouped — Tauri crates and Next packages version together, and a
split upgrade usually fails to compile.

## Releases _(Phase 3 — [#15](https://github.com/adamgreenwell/flume/issues/15))_

Planned: pushing a `v*` tag triggers a matrix build and attaches artifacts to a
GitHub Release.

| Job               | Runner                    | Output         |
| ----------------- | ------------------------- | -------------- |
| `build-linux-deb` | `ubuntu-22.04`            | `.deb`         |
| `build-linux-rpm` | Fedora / RHEL 9 container | `.rpm`         |
| `build-windows`   | `windows-latest`          | `.msi`, `.exe` |
| `build-macos`     | `macos-latest`            | `.dmg`         |

Signing stays optional via repository secrets, empty when unavailable, so forks
and contributors can build without credentials.

### Release checklist

1. Every issue in the milestone is closed or explicitly deferred.
2. CI green on `main`.
3. `cargo test -- --ignored` passes locally (live DHT path).
4. Per-platform smoke checklist in [[Platform-Notes]] completed.
5. Version bumped in `package.json`, `src-tauri/Cargo.toml`, and
   `src-tauri/tauri.conf.json` — all three must match.
6. Changelog updated.
7. Tag and push: `git tag v0.1.0 && git push origin v0.1.0`.
8. Verify artifacts install cleanly on each platform before announcing.

## Secrets

| Secret                                                             | Purpose                        |
| ------------------------------------------------------------------ | ------------------------------ |
| `TAURI_SIGNING_PRIVATE_KEY`                                        | Updater signature              |
| `APPLE_CERTIFICATE`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | macOS signing and notarization |
| `WINDOWS_CERTIFICATE`                                              | Windows Authenticode           |

None are required for CI to pass; absence disables signing, not the build.
