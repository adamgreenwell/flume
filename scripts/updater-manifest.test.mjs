// @vitest-environment node
import { describe, expect, it } from "vitest";

import { REQUIRED, buildManifest, keysFor } from "./updater-manifest.mjs";

/**
 * The installer names v1.4.1 shipped, verbatim from its release, plus the
 * macOS archive tauri-action would add (`Flume_universal.app.tar.gz` -- the
 * one name with no version in it). With updater artifacts on, each of these
 * gains a `.sig` beside it.
 */
const RELEASE_ASSETS = [
  "Flume-1.4.1-1.aarch64.rpm",
  "Flume-1.4.1-1.x86_64.rpm",
  "Flume_1.4.1_aarch64.AppImage",
  "Flume_1.4.1_amd64.AppImage",
  "Flume_1.4.1_amd64.deb",
  "Flume_1.4.1_arm64-setup.exe",
  "Flume_1.4.1_arm64.deb",
  "Flume_1.4.1_arm64_en-US.msi",
  "Flume_1.4.1_universal.dmg",
  "Flume_1.4.1_x64-setup.exe",
  "Flume_1.4.1_x64_en-US.msi",
  "Flume_universal.app.tar.gz",
];

const signedRelease = () =>
  RELEASE_ASSETS.map((name) => ({ name, signature: `sig-of-${name}\n` }));

describe("keysFor", () => {
  it.each([
    [
      "Flume_universal.app.tar.gz",
      [
        "darwin-aarch64",
        "darwin-aarch64-app",
        "darwin-x86_64",
        "darwin-x86_64-app",
      ],
    ],
    ["Flume_1.4.1_x64_en-US.msi", ["windows-x86_64", "windows-x86_64-msi"]],
    ["Flume_1.4.1_x64-setup.exe", ["windows-x86_64-nsis"]],
    ["Flume_1.4.1_arm64_en-US.msi", ["windows-aarch64", "windows-aarch64-msi"]],
    ["Flume_1.4.1_arm64-setup.exe", ["windows-aarch64-nsis"]],
    ["Flume_1.4.1_amd64.AppImage", ["linux-x86_64", "linux-x86_64-appimage"]],
    [
      "Flume_1.4.1_aarch64.AppImage",
      ["linux-aarch64", "linux-aarch64-appimage"],
    ],
  ])("maps %s", (name, keys) => {
    expect(keysFor(name)).toEqual(keys);
  });

  // Flume never replaces these in place, and a dmg is not an updater format.
  it.each([
    "Flume_1.4.1_amd64.deb",
    "Flume_1.4.1_arm64.deb",
    "Flume-1.4.1-1.x86_64.rpm",
    "Flume_1.4.1_universal.dmg",
    "SHA256SUMS.txt",
  ])("gives %s no key", (name) => {
    expect(keysFor(name)).toEqual([]);
  });
});

describe("buildManifest", () => {
  const build = (signed = signedRelease()) =>
    buildManifest({
      tag: "v1.5.0",
      notes: "notes",
      pubDate: "2026-10-20T00:00:00.000Z",
      signed,
    });

  it("covers every required platform from a real release's assets", () => {
    const manifest = build();
    for (const key of REQUIRED) expect(manifest.platforms[key]).toBeDefined();
  });

  it("points each key at its asset on this release, with its signature", () => {
    const { platforms, version } = build();
    expect(version).toBe("1.5.0");
    expect(platforms["windows-x86_64-nsis"]).toEqual({
      url: "https://github.com/adamgreenwell/flume/releases/download/v1.5.0/Flume_1.4.1_x64-setup.exe",
      signature: "sig-of-Flume_1.4.1_x64-setup.exe",
    });
  });

  it("gives .deb and .rpm installs no key of their own", () => {
    const keys = Object.keys(build().platforms);
    expect(keys.some((k) => k.endsWith("-deb") || k.endsWith("-rpm"))).toBe(
      false,
    );
  });

  // The point of building it in one job: a release missing a leg publishes
  // nothing, rather than a manifest that silently stops updating a platform.
  it("refuses to publish when a platform's artifact is missing", () => {
    const withoutArmLinux = signedRelease().filter(
      (a) => a.name !== "Flume_1.4.1_aarch64.AppImage",
    );
    expect(() => build(withoutArmLinux)).toThrow(/linux-aarch64/);
  });

  it("refuses two artifacts claiming the same key", () => {
    const doubled = [
      ...signedRelease(),
      { name: "Flume_1.5.0_amd64.AppImage", signature: "other" },
    ];
    expect(() => build(doubled)).toThrow(/two updater artifacts claim/);
  });
});
