#!/usr/bin/env bash
#
# Runs everything CI runs, locally, before anything is pushed.
#
#   npm run preflight            # the standard gate
#   npm run preflight -- --full  # also builds the macOS bundle
#
# Why this exists: this repository is private, so Actions minutes are billed,
# and a full release run costs several dollars. Every failure caught here is a
# failure that did not cost anything.
#
# Exit codes are checked directly rather than grepped from output -- a filtered
# gate once reported success while the chain had actually failed earlier.

set -uo pipefail

FULL=0
[[ "${1:-}" == "--full" ]] && FULL=1

# Checked before anything else, because the failure it prevents is actively
# misleading. Below Node 22, jsdom's copy of undici calls
# `markAsUncloneable`, which does not exist yet, and vitest reports that as
# "no tests" with a worker error rather than as a version problem. It also
# passes on retry if a different node happens to come first, so it reads as
# flaky rather than as a wrong toolchain.
if ! node scripts/check-node.mjs; then
  echo
  echo "Wrong Node version. Nothing else was run."
  exit 1
fi

failed=0
run() {
  local label="$1"; shift
  printf '  %-34s' "$label"
  if "$@" >/tmp/preflight.log 2>&1; then
    echo "ok"
  else
    echo "FAILED"
    echo "    --- last 15 lines ---"
    tail -15 /tmp/preflight.log | sed 's/^/    /'
    failed=1
  fi
}

echo "Frontend"
run "typecheck"        npm run typecheck
run "lint"             npm run lint
run "format"           npm run format:check
run "tests"            npm run test
run "static export"    npm run build

echo "Backend"
run "cargo fmt"        cargo fmt --manifest-path src-tauri/Cargo.toml --check
run "clippy"           cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
run "cargo test"       cargo test --manifest-path src-tauri/Cargo.toml

echo "Workflows"
for f in .github/workflows/*.yml; do
  run "$(basename "$f")" python3 -c "import yaml,sys;yaml.safe_load(open(sys.argv[1]))" "$f"
done

if [[ $FULL -eq 1 ]]; then
  echo "Bundle"
  # The step most worth doing locally: bundling is where the release pipeline
  # has failed most often, and on macOS it costs nothing to check here.
  run "tauri build"    npm run tauri:build

  DMG=$(find src-tauri/target/release/bundle/dmg -name '*.dmg' 2>/dev/null | head -1)
  if [[ -n "$DMG" ]]; then
    printf '  %-34s' "dmg has no licence gate"
    if hdiutil imageinfo "$DMG" 2>/dev/null | grep -q "Software License Agreement: false"; then
      echo "ok"
    else
      echo "FAILED — the dmg would demand a click-through EULA"
      failed=1
    fi
  fi
fi

echo
if [[ $failed -eq 0 ]]; then
  echo "All checks passed. Safe to push."
else
  echo "Something failed. Fixing it here costs nothing; finding it in CI does."
fi
exit $failed
