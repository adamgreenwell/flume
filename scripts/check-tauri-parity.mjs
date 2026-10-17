/**
 * Fails when a Tauri Rust crate and its npm counterpart drift apart.
 *
 * `tauri build` refuses to run when a `tauri*` crate and its `@tauri-apps/*`
 * package are not on the same major/minor, and nothing before it looks. So
 * the drift is invisible to `npm run check`, to `cargo test`, and to CI —
 * every one of them passes — and surfaces only in a release build, where it
 * fails on all platforms at once after the tag is already pushed. That is the
 * most expensive place to find it and the only place it was findable.
 *
 * It drifts because the two ecosystems update separately: `dependabot.yml`
 * has a `tauri` group for the crates under /src-tauri and another for
 * `@tauri-apps/*` under /, and neither knows about the other. Either group
 * merging alone is enough to break the next release.
 *
 * Compared from the lockfiles rather than the manifests, because resolved
 * versions are what `tauri build` reads — a manifest range of "2.11.5" can
 * resolve to 2.12.4, and the manifest would look fine while the build fails.
 *
 * `tauri` pairs with `@tauri-apps/api`; every `tauri-plugin-X` pairs with
 * `@tauri-apps/plugin-X`. A crate with no npm counterpart is not a pair and
 * is skipped: several plugins are Rust-only here.
 */
import { readFileSync } from "node:fs";

const root = new URL("../", import.meta.url);

/** Resolved crate versions from `Cargo.lock`, keyed by crate name. */
function crateVersions() {
  const lock = readFileSync(new URL("src-tauri/Cargo.lock", root), "utf8");
  const found = new Map();
  // One `[[package]]` block per crate, `name` then `version`. Parsed rather
  // than depending on a TOML library: this runs before anything installs.
  for (const block of lock.split("[[package]]")) {
    const name = block.match(/^\s*name\s*=\s*"([^"]+)"/m)?.[1];
    const version = block.match(/^\s*version\s*=\s*"([^"]+)"/m)?.[1];
    if (name && version && /^tauri(-plugin-[a-z0-9-]+)?$/.test(name)) {
      found.set(name, version);
    }
  }
  return found;
}

/** Resolved `@tauri-apps/*` versions from `package-lock.json`. */
function packageVersions() {
  const lock = JSON.parse(
    readFileSync(new URL("package-lock.json", root), "utf8"),
  );
  const found = new Map();
  for (const [path, entry] of Object.entries(lock.packages ?? {})) {
    // The hoisted copy only. A nested one belongs to a dependency and is not
    // what the bundle resolves for our own imports.
    const match = path.match(/^node_modules\/(@tauri-apps\/[a-z0-9-]+)$/);
    if (match && entry.version) found.set(match[1], entry.version);
  }
  return found;
}

/** The npm package a crate pairs with, or null when it has none. */
function counterpart(crate) {
  if (crate === "tauri") return "@tauri-apps/api";
  const plugin = crate.match(/^tauri-plugin-([a-z0-9-]+)$/);
  return plugin ? `@tauri-apps/plugin-${plugin[1]}` : null;
}

const minor = (version) => version.split(".").slice(0, 2).join(".");

const crates = crateVersions();
const packages = packageVersions();
const mismatched = [];
let compared = 0;

for (const [crate, crateVersion] of [...crates].sort()) {
  const npm = counterpart(crate);
  const npmVersion = npm && packages.get(npm);
  if (!npmVersion) continue;
  compared += 1;
  if (minor(crateVersion) !== minor(npmVersion)) {
    mismatched.push(`  ${crate} ${crateVersion}  vs  ${npm} ${npmVersion}`);
  }
}

if (compared === 0) {
  console.error(
    "check-tauri-parity: found no crate/package pairs to compare, which means\n" +
      "this check is not checking anything. Either the lockfiles moved or the\n" +
      "naming convention changed; fix the script rather than deleting it.",
  );
  process.exit(1);
}

if (mismatched.length > 0) {
  console.error(
    `Tauri versions have drifted apart in ${mismatched.length} place(s):\n` +
      `${mismatched.join("\n")}\n\n` +
      "`tauri build` refuses to run on this, so a release would fail on every\n" +
      "platform at once. Bring the two sides onto the same major/minor before\n" +
      "tagging:\n\n" +
      "  cd src-tauri && cargo update tauri tauri-plugin-<name> ...\n" +
      "  npm install @tauri-apps/api@<matching minor> ...\n\n" +
      "If cargo will not move the crate, check `rust-version` in\n" +
      "src-tauri/Cargo.toml: the MSRV-aware resolver holds back any version\n" +
      "that needs a newer Rust than the MSRV claims, silently.",
  );
  process.exit(1);
}

console.log(`Tauri versions agree across ${compared} crate/package pairs.`);
