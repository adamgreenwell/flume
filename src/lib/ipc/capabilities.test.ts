import { readFileSync, readdirSync } from "node:fs";
import { extname, join, resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * Keeps `src-tauri/capabilities/default.json` in step with the plugin APIs the
 * frontend actually calls.
 *
 * This is the one part of the IPC contract nothing checked. A missing
 * permission is not a compile error, not a lint error, and not visible in any
 * test: Tauri's ACL denies the call at runtime, in a release build, on a user's
 * machine. `writeText` shipped that way — the Copy button in the diagnostics
 * report did nothing at all from the day it was written until someone pressed
 * it, because the capability granted `allow-read-text` and nothing else.
 *
 * The reverse matters too. CLAUDE.md asks for the minimum permission set, and
 * a grant nobody needs is invisible in exactly the same way — it is not wrong
 * until the day it is.
 *
 * Both directions are asserted here, and an API this file has never heard of
 * fails rather than passing silently. That last rule is what stops the test
 * being false comfort: a scanner that quietly ignores what it does not
 * recognise reports success for the case it was written to catch.
 */

// Resolved from the working directory rather than `import.meta.url`, for the
// same reason as `tokens.test.ts`: these run under jsdom, where module URLs are
// `http:` and cannot be converted to a path.
const ROOT = resolve(process.cwd());
const CAPABILITIES = resolve(ROOT, "src-tauri/capabilities/default.json");
const SOURCE = resolve(ROOT, "src");

/**
 * Which permission each plugin API needs.
 *
 * Hand-maintained, and deliberately exhaustive per plugin rather than
 * best-effort: an unlisted export of a listed plugin is a failure below, so
 * adding a new call forces a decision about its permission instead of letting
 * one be forgotten.
 */
const REQUIRES: Record<string, Record<string, string>> = {
  "@tauri-apps/plugin-clipboard-manager": {
    readText: "clipboard-manager:allow-read-text",
    writeText: "clipboard-manager:allow-write-text",
    readImage: "clipboard-manager:allow-read-image",
    writeImage: "clipboard-manager:allow-write-image",
    writeHtml: "clipboard-manager:allow-write-html",
    clear: "clipboard-manager:allow-clear",
  },
  "@tauri-apps/plugin-dialog": {
    open: "dialog:allow-open",
    save: "dialog:allow-save",
    message: "dialog:allow-message",
    ask: "dialog:allow-ask",
    confirm: "dialog:allow-confirm",
  },
  "@tauri-apps/plugin-opener": {
    openUrl: "opener:allow-open-url",
    openPath: "opener:allow-open-path",
    revealItemInDir: "opener:allow-reveal-item-in-dir",
  },
  "@tauri-apps/plugin-notification": {
    sendNotification: "notification:allow-notify",
    requestPermission: "notification:allow-request-permission",
    isPermissionGranted: "notification:allow-is-permission-granted",
  },
};

/**
 * Permissions granted for reasons no frontend import can reveal.
 *
 * Each needs a reason, because "it was already there" is how a permission set
 * stops being minimal.
 */
const GRANTED_ELSEWHERE: Record<string, string> = {
  "core:default":
    "Tauri's own baseline — event listening, which every hook in src/hooks uses.",
  "core:window:allow-start-dragging":
    "TitleBar's `data-tauri-drag-region`, which is an attribute rather than an import.",
  "notification:default":
    "Rust: `telemetry.rs` announces a finished torrent through NotificationExt.",
  "deep-link:default":
    "Rust: `deeplink.rs` registers the magnet handler and receives opened URLs.",
};

/** Every `.ts`/`.tsx` file under `src`, excluding tests and stories. */
function sourceFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sourceFiles(path);
    if (![".ts", ".tsx"].includes(extname(entry.name))) return [];
    // A test or a story never runs under the ACL, so its imports say nothing
    // about what the shipped app needs.
    if (/\.(test|stories)\.tsx?$/.test(entry.name)) return [];
    return [path];
  });
}

/** Plugin imports found in the shipped frontend, as `[module, symbol]`. */
function pluginImports(): Array<{
  file: string;
  module: string;
  symbol: string;
}> {
  // `[^}]*` spans newlines, so a multi-line import list is matched whole.
  const pattern =
    /import\s*\{([^}]*)\}\s*from\s*["'](@tauri-apps\/plugin-[a-z-]+)["']/g;

  return sourceFiles(SOURCE).flatMap((file) => {
    const body = readFileSync(file, "utf8");
    return [...body.matchAll(pattern)].flatMap((match) =>
      match[1]
        .split(",")
        .map((name) =>
          name
            .trim()
            .split(/\s+as\s+/)[0]
            .trim(),
        )
        .filter((name) => name.length > 0)
        .map((symbol) => ({
          file: file.slice(ROOT.length + 1),
          module: match[2],
          symbol,
        })),
    );
  });
}

/** The permissions the capability file grants. */
function granted(): string[] {
  const file = JSON.parse(readFileSync(CAPABILITIES, "utf8")) as {
    permissions: string[];
  };
  return file.permissions;
}

describe("the Tauri capability set", () => {
  it("grants a permission for every plugin API the frontend calls", () => {
    const permissions = new Set(granted());

    for (const { file, module, symbol } of pluginImports()) {
      const needed = REQUIRES[module]?.[symbol];
      expect(
        needed,
        `${file} imports ${symbol} from ${module}, which this test does not ` +
          `know the permission for. Add it to REQUIRES — silently skipping it ` +
          `would defeat the point of the test.`,
      ).toBeDefined();

      expect(
        permissions.has(needed as string),
        `${file} calls ${symbol}(), which needs "${needed}". ` +
          `src-tauri/capabilities/default.json does not grant it, so the call ` +
          `fails at runtime in a release build with nothing in any test to ` +
          `show it.`,
      ).toBe(true);
    }
  });

  it("grants nothing the app does not use", () => {
    const used = new Set(
      pluginImports()
        .map(({ module, symbol }) => REQUIRES[module]?.[symbol])
        .filter((permission): permission is string => permission !== undefined),
    );

    for (const permission of granted()) {
      const explained = used.has(permission) || permission in GRANTED_ELSEWHERE;
      expect(
        explained,
        `src-tauri/capabilities/default.json grants "${permission}", which no ` +
          `frontend call needs. If Rust or a DOM attribute needs it, say so in ` +
          `GRANTED_ELSEWHERE; otherwise remove it. CLAUDE.md asks for the ` +
          `minimum set.`,
      ).toBe(true);
    }
  });

  it("cannot be evaded by an import shape it does not parse", () => {
    // The scanner understands `import { a, b } from "@tauri-apps/plugin-x"`
    // and nothing else. A namespace import, a dynamic import, or a raw
    // `invoke("plugin:...")` would reach the same APIs while this file saw
    // none of it — and every test above would still pass. So any reference it
    // cannot account for is a failure, rather than a silent gap.
    const named =
      /import\s*\{([^}]*)\}\s*from\s*["'](@tauri-apps\/plugin-[a-z-]+)["']/g;

    for (const file of sourceFiles(SOURCE)) {
      const body = readFileSync(file, "utf8");

      // Comments stripped, and only *quoted* specifiers counted. The first
      // version of this check counted every mention of the string and failed
      // on `platform.ts`, whose doc comment names `@tauri-apps/plugin-os` to
      // explain why it is deliberately not used. Prose about a plugin is not a
      // call to one.
      const code = body
        .replace(/\/\*[\s\S]*?\*\//g, "")
        .replace(/^\s*\/\/.*$/gm, "");
      const mentions = code.match(/["']@tauri-apps\/plugin-[a-z-]+["']/g) ?? [];
      const parsed = [...code.matchAll(named)];

      expect(
        mentions.length,
        `${file.slice(ROOT.length + 1)} refers to a Tauri plugin ${mentions.length} ` +
          `time(s) but only ${parsed.length} are named imports this test can read. ` +
          `Use \`import { x } from "@tauri-apps/plugin-y"\`, or teach the scanner ` +
          `the new shape — an unparsed call is an ungoverned permission.`,
      ).toBe(parsed.length);

      // Bypasses the plugin wrapper entirely and speaks to the ACL directly.
      expect(
        /invoke\s*[(<][^)]*["']plugin:/.test(code),
        `${file.slice(ROOT.length + 1)} calls a plugin command through invoke(), ` +
          `which needs a permission this test cannot infer. Use the plugin's own ` +
          `JS API so the requirement is visible.`,
      ).toBe(false);
    }
  });

  it("finds the call sites it is supposed to be checking", () => {
    // A scan that silently matched nothing would pass both tests above while
    // checking nothing at all — the failure mode of every lint written against
    // a regex.
    const found = pluginImports();
    expect(found.length).toBeGreaterThan(0);

    const modules = new Set(found.map(({ module }) => module));
    expect([...modules].sort()).toEqual([
      "@tauri-apps/plugin-clipboard-manager",
      "@tauri-apps/plugin-dialog",
      "@tauri-apps/plugin-opener",
    ]);
  });
});
