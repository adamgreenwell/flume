// @vitest-environment node
import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { extractSection, releaseBody } from "./release-notes.mjs";

const CHANGELOG = `# Changelog

[kac]: https://keepachangelog.com/en/1.1.0/

## [Unreleased]

## [1.2.0] — 2026-10-01

**Lead paragraph.**

### Added

- A thing. ([#62][i62])

#### Detail

- Deeper.

## [1.1.0] — 2026-09-01

### Fixed

- Another thing.

## [1.0.1] — 2026-08-31

## [1.0.0] — 2026-08-30

- First.

[i62]: https://github.com/adamgreenwell/flume/issues/62
[1.2.0]: https://github.com/adamgreenwell/flume/releases/tag/v1.2.0
`;

const DOWNLOAD_NOTES = "Download the file for your platform below.\n";

describe("extractSection", () => {
  it("takes only the requested version, with headings raised a level", () => {
    expect(extractSection(CHANGELOG, "1.1.0")).toBe(
      "## Fixed\n\n- Another thing.",
    );
  });

  it("raises deeper headings by one level too", () => {
    expect(extractSection(CHANGELOG, "1.2.0")).toContain("\n### Detail\n");
  });

  it("carries reference link definitions from elsewhere in the file", () => {
    const section = extractSection(CHANGELOG, "1.2.0");
    expect(section).toMatch(
      /\n\n\[i62\]: https:\/\/github\.com\/adamgreenwell\/flume\/issues\/62$/,
    );
    expect(section).not.toContain("[1.2.0]:");
  });

  it("stops at the end of the file for the oldest version", () => {
    expect(extractSection(CHANGELOG, "1.0.0")).toBe(
      "- First.\n\n[i62]: https://github.com/adamgreenwell/flume/issues/62\n[1.2.0]: https://github.com/adamgreenwell/flume/releases/tag/v1.2.0",
    );
  });

  it("does not match a version that only shares a prefix", () => {
    expect(extractSection(CHANGELOG, "1.0")).toBeNull();
    expect(extractSection(CHANGELOG, "1.2.0.1")).toBeNull();
  });

  it("returns null for a missing or empty section", () => {
    expect(extractSection(CHANGELOG, "9.9.9")).toBeNull();
    expect(extractSection(CHANGELOG, "1.0.1")).toBeNull();
  });
});

describe("releaseBody", () => {
  it("puts the changelog section before the download notes", () => {
    const { body, warning } = releaseBody({
      tag: "v1.1.0",
      changelog: CHANGELOG,
      downloadNotes: DOWNLOAD_NOTES,
    });
    expect(body).toBe(
      "## Fixed\n\n- Another thing.\n\nDownload the file for your platform below.\n",
    );
    expect(warning).toBeNull();
  });

  it("refuses a stable tag with no section", () => {
    expect(() =>
      releaseBody({
        tag: "v1.0.1",
        changelog: CHANGELOG,
        downloadNotes: DOWNLOAD_NOTES,
      }),
    ).toThrow(/no "## \[1\.0\.1\]" section/);
  });

  it("lets a pre-release ship with the download notes alone, and says so", () => {
    const { body, warning } = releaseBody({
      tag: "v1.3.0-rc.1",
      changelog: CHANGELOG,
      downloadNotes: DOWNLOAD_NOTES,
    });
    expect(body).toBe(DOWNLOAD_NOTES);
    expect(warning).toMatch(/pre-release/);
  });
});

describe("the real CHANGELOG.md", () => {
  const real = readFileSync(
    new URL("../CHANGELOG.md", import.meta.url),
    "utf8",
  );
  const versions = [...real.matchAll(/^## \[(\d+\.\d+\.\d+)\]/gm)].map(
    (m) => m[1],
  );

  it("has a non-empty section for every released version", () => {
    expect(versions.length).toBeGreaterThan(0);
    for (const version of versions) {
      expect(extractSection(real, version), version).toBeTruthy();
    }
  });

  it("never leaves a version heading or an unraised ### in a section", () => {
    for (const version of versions) {
      const section = extractSection(real, version);
      expect(section, version).not.toMatch(/^## \[/m);
      expect(section, version).not.toMatch(
        /^### (Added|Changed|Fixed|Removed|Security|Deprecated)\b/m,
      );
    }
  });
});
