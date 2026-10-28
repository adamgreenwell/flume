#!/usr/bin/env node

import { execFile } from "node:child_process";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";

import { loadDashboardData } from "./dashboard/data.mjs";

const collectorDir = fileURLToPath(new URL("./", import.meta.url));
const outputDir = fileURLToPath(new URL("./.dashboard/", import.meta.url));
const outputFile = fileURLToPath(
  new URL("./.dashboard/index.html", import.meta.url),
);
const templateFile = fileURLToPath(
  new URL("./dashboard/index.html", import.meta.url),
);
const marker = "/* FLUME_DASHBOARD_DATA */ null";

function usage() {
  console.log("Usage: npm run dashboard -- [--local] [--no-open]");
  console.log(
    "Reads aggregate usage counts from D1 and opens a local HTML snapshot.",
  );
}

function parseArgs(args) {
  const options = { local: false, open: true };
  for (const arg of args) {
    if (arg === "--local") options.local = true;
    else if (arg === "--no-open") options.open = false;
    else if (arg === "--help" || arg === "-h") {
      usage();
      process.exit(0);
    } else throw new Error(`Unknown option: ${arg}`);
  }
  return options;
}

function inlineJson(value) {
  // This JSON lives in a script element. Escaping '<' prevents a value from
  // terminating the element, even if the D1 content ever changes shape.
  return JSON.stringify(value).replace(
    /[<>&\u2028\u2029]/g,
    (character) =>
      `\\u${character.charCodeAt(0).toString(16).padStart(4, "0")}`,
  );
}

async function openReport(path) {
  const launch =
    process.platform === "darwin"
      ? ["open", [path]]
      : process.platform === "linux"
        ? ["xdg-open", [path]]
        : process.platform === "win32"
          ? [
              "rundll32.exe",
              ["url.dll,FileProtocolHandler", pathToFileURL(path).href],
            ]
          : null;
  if (!launch) return;
  await new Promise((resolve, reject) => {
    execFile(launch[0], launch[1], { cwd: collectorDir }, (error) =>
      error ? reject(error) : resolve(),
    );
  });
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const report = await loadDashboardData({ local: options.local });
  const template = await readFile(templateFile, "utf8");
  if (!template.includes(marker))
    throw new Error("Dashboard template is missing its data marker");

  await mkdir(outputDir, { recursive: true });
  const temporary = `${outputFile}.tmp`;
  await writeFile(temporary, template.replace(marker, inlineJson(report)), {
    mode: 0o600,
  });
  await rename(temporary, outputFile);
  console.log(`Flume usage report: ${pathToFileURL(outputFile).href}`);
  console.log(
    `Source: ${options.local ? "local" : "remote"} D1 · snapshot at ${new Date(report.generatedAt * 1000).toLocaleString()}`,
  );
  if (options.open) {
    try {
      await openReport(outputFile);
    } catch {
      console.log(
        "Could not open a browser automatically. Open the report path above.",
      );
    }
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
