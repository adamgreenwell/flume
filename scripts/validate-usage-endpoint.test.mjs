import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import {
  exportUsageEndpoint,
  validateUsageEndpoint,
} from "./validate-usage-endpoint.mjs";

const script = fileURLToPath(
  new URL("./validate-usage-endpoint.mjs", import.meta.url),
);
const good = "https://flume-usage.real-account.workers.dev/v1/usage";

test("accepts only a concrete HTTPS collector route", () => {
  for (const value of [good, "https://collector.flume.app:8443/v1/usage"]) {
    assert.equal(validateUsageEndpoint(value), true);
  }
  for (const value of [
    "https://flume-usage.<your-subdomain>.workers.dev/v1/usage",
    "https://flume-usage.your-subdomain.workers.dev/v1/usage",
    "https://collector.example.com/v1/usage",
    "https://localhost:8787/v1/usage",
    "https://x/v1/usage",
    "https://bad..flume.app/v1/usage",
    "https://-bad.flume.app/v1/usage",
    "https://127.0.0.1/v1/usage",
    "https://[::1]/v1/usage",
    "https://[::ffff:127.0.0.1]/v1/usage",
    "https://8.8.8.8/v1/usage",
    "https://[2001:4860:4860::8888]/v1/usage",
    "http://collector.flume.app/v1/usage",
    "https://",
    "https://collector.flume.app",
    "https://collector.flume.app/v1/usage/",
    "https://collector.flume.app/v1/x/../usage",
    "https://user:pass@collector.flume.app/v1/usage",
    "https://@collector.flume.app/v1/usage",
    "https://collector.flume.app/v1/usage?",
    "https://collector.flume.app/v1/usage?foo=bar",
    "https://collector.flume.app/v1/usage#",
    `${good}\n`,
  ]) {
    assert.throws(() => validateUsageEndpoint(value), undefined, value);
  }
});

test("absent secret exports nothing, leaving Rust option_env! inert", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "flume-endpoint-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const envFile = join(dir, "github-env");
  assert.equal(exportUsageEndpoint(undefined, envFile), false);
  assert.equal(exportUsageEndpoint("", envFile), false);
  assert.throws(() => readFileSync(envFile));
});

test("configured endpoint is exported once and rejected values never enter GITHUB_ENV", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "flume-endpoint-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const envFile = join(dir, "github-env");
  assert.throws(() =>
    exportUsageEndpoint("https://example.com/v1/usage", envFile),
  );
  assert.throws(() => readFileSync(envFile));
  assert.equal(exportUsageEndpoint(good, envFile), true);
  assert.equal(readFileSync(envFile, "utf8"), `FLUME_USAGE_ENDPOINT=${good}\n`);
});

test("CLI fails closed without echoing a malformed secret", () => {
  const secret = "https://flume-usage.<your-subdomain>.workers.dev/v1/usage";
  const result = spawnSync(process.execPath, [script], {
    env: {
      ...process.env,
      ENDPOINT: secret,
      GITHUB_ENV: join(tmpdir(), "missing-flume-env"),
    },
    encoding: "utf8",
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /::error::FLUME_USAGE_ENDPOINT/);
  assert.doesNotMatch(result.stderr, /your-subdomain/);
  assert.doesNotMatch(result.stdout, /your-subdomain/);
});

test("CLI keeps absent secrets inert and exports a valid endpoint", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "flume-endpoint-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const envFile = join(dir, "github-env");
  const env = { ...process.env, GITHUB_ENV: envFile };

  const absent = spawnSync(process.execPath, [script], {
    env: { ...env, ENDPOINT: "" },
    encoding: "utf8",
  });
  assert.equal(absent.status, 0);
  assert.throws(() => readFileSync(envFile));

  const present = spawnSync(process.execPath, [script], {
    env: { ...env, ENDPOINT: good },
    encoding: "utf8",
  });
  assert.equal(present.status, 0);
  assert.equal(readFileSync(envFile, "utf8"), `FLUME_USAGE_ENDPOINT=${good}\n`);
  assert.doesNotMatch(present.stdout, /real-account/);
});
