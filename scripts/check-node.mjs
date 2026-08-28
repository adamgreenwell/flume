/**
 * Fails fast, and legibly, on the wrong Node.
 *
 * `engines` plus `engine-strict` only gate `npm install`. The failure that
 * actually bites is running the suite on a Node installed *after* a good
 * install, or with a different `node` first on PATH — and that failure is
 * unreadable: jsdom pulls in undici, whose webidl shim does
 * `require('node:worker_threads').markAsUncloneable`, absent before Node 22.
 * Vitest reports it as "Failed to start forks worker" for every test file and
 * exits with "no tests", which reads like a vitest bug rather than a version
 * mismatch. It also passes on retry if the right node happens to come first,
 * so it looks intermittent.
 *
 * The required major is read from `engines` so there is one source of truth.
 */
import { readFileSync } from "node:fs";

const pkg = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);

const required = Number.parseInt(pkg.engines.node.replace(/^\D+/, ""), 10);
const actual = Number.parseInt(process.versions.node.split(".")[0], 10);

if (Number.isNaN(required)) {
  console.error(
    `check-node: could not read a major version out of engines.node ` +
      `("${pkg.engines.node}"). Fix package.json rather than removing this check.`,
  );
  process.exit(1);
}

if (actual < required) {
  console.error(
    [
      "",
      `  This project needs Node ${required}. You are running ${process.versions.node}.`,
      `  (${process.execPath})`,
      "",
      "  Below Node 22 the test suite does not fail honestly — jsdom's copy of",
      "  undici calls an API that does not exist yet, and vitest reports it as",
      '  "no tests" with a worker error rather than as a version problem.',
      "",
      "  With nvm or fnm installed, `nvm use` / `fnm use` reads .nvmrc.",
      "",
    ].join("\n"),
  );
  process.exit(1);
}
