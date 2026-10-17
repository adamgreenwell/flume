/** Validate the endpoint before a release bakes it into a native binary. */
import { appendFileSync } from "node:fs";
import { isIP } from "node:net";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SCHEME = "https://";
const PATH = "/v1/usage";
const PLACEHOLDER_LABELS = new Set([
  "example",
  "placeholder",
  "replace",
  "replace-me",
  "your-domain",
  "your-subdomain",
  "your-worker",
]);
const RESERVED_SUFFIXES = [".example", ".invalid", ".test", ".localhost"];

export function validateUsageEndpoint(value) {
  // GitHub expands an absent secret to "". In that case the next build step
  // must receive no FLUME_USAGE_ENDPOINT variable at all, so reporting stays
  // inert rather than compiling Some("").
  if (value === undefined || value === "") return false;
  if (
    typeof value !== "string" ||
    !value.startsWith(SCHEME) ||
    /\s/.test(value)
  ) {
    throw new Error("FLUME_USAGE_ENDPOINT must be an HTTPS URL for /v1/usage.");
  }

  // Check the literal path as well as the parsed URL. URL parsers normalize
  // dot segments and can accept empty query/hash delimiters, but the release
  // contract is exactly one route with no suffix or extra components.
  const rest = value.slice(SCHEME.length);
  const slash = rest.indexOf("/");
  if (
    slash < 1 ||
    rest.slice(slash) !== PATH ||
    rest.slice(0, slash).includes("@")
  ) {
    throw new Error(
      "FLUME_USAGE_ENDPOINT must have a host and exactly /v1/usage, without credentials, query, or hash.",
    );
  }

  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error("FLUME_USAGE_ENDPOINT is not a valid HTTPS URL.");
  }
  if (
    url.protocol !== "https:" ||
    !url.hostname ||
    !/[a-z0-9]/i.test(url.hostname) ||
    url.pathname !== PATH ||
    url.username ||
    url.password ||
    url.search ||
    url.hash
  ) {
    throw new Error(
      "FLUME_USAGE_ENDPOINT must be an HTTPS URL with a host and exactly /v1/usage.",
    );
  }

  const host = url.hostname.toLowerCase();
  const labels = host.split(".");
  const bareHost = host.replace(/^\[|\]$/g, "");
  const ipVersion = isIP(bareHost);
  const validDnsHost =
    labels.length >= 2 &&
    labels.every(
      (label) =>
        label.length <= 63 && /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/i.test(label),
    );
  if (
    ipVersion !== 0 ||
    !validDnsHost ||
    host === "localhost" ||
    RESERVED_SUFFIXES.some((suffix) => host.endsWith(suffix)) ||
    labels.some((label) => PLACEHOLDER_LABELS.has(label))
  ) {
    throw new Error(
      "FLUME_USAGE_ENDPOINT requires a concrete DNS host, not a placeholder, local host, or IP address.",
    );
  }
  return true;
}

export function exportUsageEndpoint(value, githubEnvPath) {
  if (!validateUsageEndpoint(value)) return false;
  if (!githubEnvPath)
    throw new Error("GITHUB_ENV is required to export the endpoint.");
  appendFileSync(githubEnvPath, `FLUME_USAGE_ENDPOINT=${value}\n`);
  return true;
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  try {
    const configured = exportUsageEndpoint(
      process.env.ENDPOINT,
      process.env.GITHUB_ENV,
    );
    console.log(
      configured
        ? "Collector endpoint validated and configured."
        : "No FLUME_USAGE_ENDPOINT secret configured; usage reporting compiles inert.",
    );
  } catch (error) {
    // The error never includes the supplied value. Release logs must not print
    // even a malformed secret that Actions may fail to mask.
    console.error(`::error::${error.message}`);
    process.exitCode = 1;
  }
}
