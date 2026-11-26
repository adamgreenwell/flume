#!/usr/bin/env node
/**
 * Writes a release's GitHub body: its section of CHANGELOG.md, then the
 * download notes every release carries.
 *
 * `release.yml` used to give tauri-action the download notes alone, and the
 * changelog was pasted in by hand before the draft was published. That step
 * lived in nobody's checklist, and v1.5.0 to v1.5.4 shipped without it — so
 * the website's changelog, which renders release bodies, showed only the
 * download notes for all five, and so did the in-app update banner, which
 * `updater-manifest.mjs` fills from the same body.
 *
 * Usage: `node scripts/release-notes.mjs <tag>`, with the download notes on
 * stdin. Prints the body to stdout.
 *
 * A stable tag with no changelog section fails rather than publishing an
 * empty one: that is the mistake this exists to stop. A pre-release tag
 * (anything with a `-`, matching `release.yml`'s `prerelease:` test) may
 * have none — release candidates do not always get an entry — and falls back
 * to the download notes with a warning.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/**
 * Pulls one version's section out of a Keep a Changelog file.
 *
 * Headings are raised a level, `###` to `##`, because a release body has no
 * version heading above them — GitHub titles the release itself — and the
 * hand-written bodies through v1.4.1 opened their sections at `##`. The site
 * styles release notes on that assumption too.
 *
 * Reference-style links (`[#62][i62]`) whose definitions sit elsewhere in the
 * file are carried along, so a section never renders a dangling `[i62]`.
 *
 * @param {string} changelog The whole of CHANGELOG.md.
 * @param {string} version A version without its `v`, e.g. `1.5.3`.
 * @returns {string | null} The section's markdown, trimmed, or `null` when
 *   the changelog has no heading for that version or the heading has nothing
 *   under it.
 */
export function extractSection(changelog, version) {
  const lines = changelog.replace(/\r\n/g, "\n").split("\n");
  const escaped = version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const heading = new RegExp(`^## \\[?v?${escaped}\\]?(\\s|$)`);

  const start = lines.findIndex((line) => heading.test(line));
  if (start === -1) return null;

  let end = lines.findIndex((line, i) => i > start && /^## /.test(line));
  if (end === -1) end = lines.length;

  const body = lines
    .slice(start + 1, end)
    .map((line) => line.replace(/^###(#*) /, "##$1 "))
    .join("\n")
    .trim();
  if (!body) return null;

  const defined = (label) => new RegExp(`^\\[${label}\\]:`, "mi").test(body);
  const definitions = [...body.matchAll(/\]\[([^\]]+)\]/g)]
    .map((m) => m[1])
    .filter((label, i, all) => all.indexOf(label) === i && !defined(label))
    .map((label) =>
      lines.find((line) =>
        line.toLowerCase().startsWith(`[${label.toLowerCase()}]:`),
      ),
    )
    .filter(Boolean);

  return definitions.length ? `${body}\n\n${definitions.join("\n")}` : body;
}

/**
 * Joins a changelog section and the download notes into a release body.
 *
 * @param {object} options
 * @param {string} options.tag The release tag, e.g. `v1.5.3`.
 * @param {string} options.changelog The whole of CHANGELOG.md.
 * @param {string} options.downloadNotes The text every release ends with.
 * @returns {{ body: string, warning: string | null }} The body, and a warning
 *   when a pre-release had no section and got the download notes alone.
 * @throws {Error} When a stable tag has no changelog section.
 */
export function releaseBody({ tag, changelog, downloadNotes }) {
  const version = tag.replace(/^v/, "");
  const section = extractSection(changelog, version);
  const notes = downloadNotes.trim();

  if (section) return { body: `${section}\n\n${notes}\n`, warning: null };

  const missing = `CHANGELOG.md has no "## [${version}]" section, or it is empty.`;
  if (!tag.includes("-")) {
    throw new Error(
      `${missing} Add one before tagging a stable release; the release notes come from it.`,
    );
  }
  return {
    body: `${notes}\n`,
    warning: `${missing} ${tag} is a pre-release, so it ships with the download notes alone.`,
  };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const tag = process.argv[2];
  if (!tag) {
    console.error(
      "usage: node scripts/release-notes.mjs <tag> < download-notes",
    );
    process.exit(2);
  }
  try {
    const { body, warning } = releaseBody({
      tag,
      changelog: readFileSync(
        new URL("../CHANGELOG.md", import.meta.url),
        "utf8",
      ),
      downloadNotes: readFileSync(0, "utf8"),
    });
    if (warning) console.error(`::warning::${warning}`);
    process.stdout.write(body);
  } catch (error) {
    console.error(`::error::${error.message}`);
    process.exit(1);
  }
}
