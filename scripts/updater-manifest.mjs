#!/usr/bin/env node
/**
 * Builds the updater manifest, `latest.json`, for a release, and uploads it.
 *
 * Run by the release workflow's `updater-manifest` job, once, after every
 * build leg has attached its artifacts and signatures to the draft:
 *
 *   node scripts/updater-manifest.mjs <tag>
 *
 * Why this exists rather than tauri-action's `uploadUpdaterJson`: that option
 * has every matrix leg read the release's latest.json, merge its own entry in,
 * delete the asset and upload the result. Five legs doing that concurrently
 * race -- upstream documents the 404s -- and a lost race publishes a manifest
 * missing platforms. Codex caught it on #236 before a keyed release ran. One
 * job after all legs cannot race, and it can refuse to publish a manifest that
 * is missing anything, which a leg on its own cannot know.
 *
 * Keys follow tauri-action's: `{os}-{arch}` for the platform's primary updater
 * artifact, and `{os}-{arch}-{bundle}` for each installer kind. `.deb` and
 * `.rpm` get no keys of their own on purpose: Flume never replaces them in
 * place (`updates::delivery_for`), and on those installs the plugin falls back
 * to the generic Linux key, which is all it needs to learn the version.
 */

import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = "adamgreenwell/flume";

/**
 * Platforms a release must cover before its manifest is published. A release
 * that lost a leg publishes no manifest at all rather than one that silently
 * stops updating a platform.
 */
export const REQUIRED = [
  "darwin-aarch64",
  "darwin-x86_64",
  "windows-x86_64",
  "windows-aarch64",
  "linux-x86_64",
  "linux-aarch64",
];

/**
 * The manifest keys one updater artifact provides, by its file name.
 *
 * Matched on suffixes and architecture tokens, not exact names, because the
 * names come from two places: Tauri's CLI (`Flume_1.5.0_x64_en-US.msi`) and
 * tauri-action's renaming of the macOS archive (`Flume_universal.app.tar.gz`,
 * with no version in it).
 *
 * @param {string} name An asset name, without `.sig`.
 * @returns {string[]} Keys, or an empty list for anything not an updater
 *   artifact Flume publishes.
 */
export function keysFor(name) {
  if (name.endsWith(".app.tar.gz")) {
    // One universal build serves both Mac architectures.
    if (name.includes("universal")) {
      return [
        "darwin-aarch64",
        "darwin-aarch64-app",
        "darwin-x86_64",
        "darwin-x86_64-app",
      ];
    }
    if (name.includes("aarch64"))
      return ["darwin-aarch64", "darwin-aarch64-app"];
    if (name.includes("x64") || name.includes("x86_64")) {
      return ["darwin-x86_64", "darwin-x86_64-app"];
    }
    return [];
  }

  const windowsArch = name.includes("_arm64")
    ? "aarch64"
    : name.includes("_x64")
      ? "x86_64"
      : null;
  if (windowsArch && name.endsWith(".msi")) {
    // The MSI is the generic key, as in tauri-action's default.
    return [`windows-${windowsArch}`, `windows-${windowsArch}-msi`];
  }
  if (windowsArch && name.endsWith("-setup.exe")) {
    return [`windows-${windowsArch}-nsis`];
  }

  if (name.endsWith(".AppImage")) {
    const arch = name.includes("_aarch64")
      ? "aarch64"
      : name.includes("_amd64")
        ? "x86_64"
        : null;
    return arch ? [`linux-${arch}`, `linux-${arch}-appimage`] : [];
  }

  return [];
}

/**
 * Builds the manifest from a release's assets.
 *
 * @param {object} release
 * @param {string} release.tag The tag, e.g. `v1.5.0`.
 * @param {string} release.notes Release notes.
 * @param {string} release.pubDate RFC 3339 publication time.
 * @param {{ name: string, signature: string }[]} release.signed Each
 *   updater artifact that has a `.sig`, with that signature's contents.
 * @returns {{ version: string, notes: string, pub_date: string,
 *   platforms: Record<string, { url: string, signature: string }> }}
 * @throws {Error} When a required platform has no artifact, or two artifacts
 *   claim the same key.
 */
export function buildManifest({ tag, notes, pubDate, signed }) {
  const platforms = {};
  for (const { name, signature } of signed) {
    const url = `https://github.com/${REPO}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(name)}`;
    for (const key of keysFor(name)) {
      if (platforms[key]) {
        throw new Error(
          `two updater artifacts claim ${key}: ${platforms[key].url} and ${url}`,
        );
      }
      platforms[key] = { url, signature: signature.trim() };
    }
  }

  const missing = REQUIRED.filter((key) => !platforms[key]);
  if (missing.length > 0) {
    throw new Error(
      `no updater artifact for ${missing.join(", ")} -- refusing to publish a manifest that would stop updating them`,
    );
  }

  return {
    version: tag.replace(/^v/, ""),
    notes,
    pub_date: pubDate,
    platforms,
  };
}

/** Runs `gh` and returns its stdout. */
function gh(args) {
  return execFileSync("gh", args, { encoding: "utf8" });
}

function main(tag) {
  const dir = mkdtempSync(join(tmpdir(), "flume-manifest-"));
  gh(["release", "download", tag, "-R", REPO, "-p", "*.sig", "-D", dir]);

  const signed = readdirSync(dir)
    .filter((file) => file.endsWith(".sig"))
    .map((file) => ({
      name: file.slice(0, -".sig".length),
      signature: readFileSync(join(dir, file), "utf8"),
    }));
  const notes = JSON.parse(
    gh(["release", "view", tag, "-R", REPO, "--json", "body"]),
  ).body;

  const manifest = buildManifest({
    tag,
    notes: notes ?? "",
    pubDate: new Date().toISOString(),
    signed,
  });

  const out = join(dir, "latest.json");
  writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);
  gh(["release", "upload", tag, out, "--clobber", "-R", REPO]);
  console.log(
    `latest.json for ${manifest.version}: ${Object.keys(manifest.platforms).sort().join(", ")}`,
  );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const tag = process.argv[2];
  if (!tag) {
    console.error("usage: node scripts/updater-manifest.mjs <tag>");
    process.exit(2);
  }
  try {
    main(tag);
  } catch (err) {
    console.error(`::error::${err instanceof Error ? err.message : err}`);
    process.exit(1);
  }
}
