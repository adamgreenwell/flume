import { execFile as execFileCallback } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFile = promisify(execFileCallback);
const collectorDir = fileURLToPath(new URL("../", import.meta.url));
const wranglerScript = fileURLToPath(
  new URL("../node_modules/wrangler/bin/wrangler.js", import.meta.url),
);
const periods = ["7", "30", "90", "all"];
const DAY = 86_400;

/**
 * Each output field is an aggregate JSON array. The UUID participates only in
 * COUNT(DISTINCT ...); neither it nor an individual event leaves this query.
 * Event dates use the client's hour-bucketed `at`. `received_at` is used only
 * to say when the collector last heard from anyone.
 */
export function dashboardSql(now) {
  if (!Number.isSafeInteger(now) || now < 0) {
    throw new TypeError("now must be a non-negative Unix timestamp");
  }

  // "Last N days" means today (UTC) and the preceding N-1 calendar days.
  // The upper bound keeps a clock-skewed event dated tomorrow out of both the
  // summary and its daily chart, including in the all-time view.
  const today = now - (now % DAY);
  const tomorrow = today + DAY;

  return `
WITH periods(period, cutoff) AS (
  VALUES ('7', ${today - 6 * DAY}),
         ('30', ${today - 29 * DAY}),
         ('90', ${today - 89 * DAY}),
         ('all', NULL)
),
windowed AS (
  SELECT p.period, e.id, e.install_id, e.app_version, e.os, e.arch,
         e.event, e.field, e.value, e.at
  FROM periods AS p
  JOIN events AS e ON (p.cutoff IS NULL OR e.at >= p.cutoff)
                  AND e.at < ${tomorrow}
)
SELECT
  (SELECT json_group_array(json_object(
    'period', period, 'events', events, 'installs', installs,
    'firstAt', firstAt, 'lastAt', lastAt
  )) FROM (
    SELECT p.period, COUNT(w.id) AS events,
           COUNT(DISTINCT w.install_id) AS installs,
           MIN(w.at) AS firstAt, MAX(w.at) AS lastAt
    FROM periods AS p LEFT JOIN windowed AS w ON w.period = p.period
    GROUP BY p.period
  )) AS summaries,
  (SELECT json_group_array(json_object(
    'period', period, 'day', day, 'events', events, 'installs', installs
  )) FROM (
    SELECT period, date(at, 'unixepoch') AS day, COUNT(*) AS events,
           COUNT(DISTINCT install_id) AS installs
    FROM windowed GROUP BY period, date(at, 'unixepoch')
  )) AS daily,
  (SELECT json_group_array(json_object(
    'period', period, 'name', name, 'count', count
  )) FROM (
    SELECT period, event AS name, COUNT(*) AS count
    FROM windowed GROUP BY period, event
  )) AS events,
  (SELECT json_group_array(json_object(
    'period', period, 'name', name, 'value', value, 'count', count
  )) FROM (
    SELECT period, event AS name, value, COUNT(*) AS count
    FROM windowed WHERE field IS NOT NULL GROUP BY period, event, value
  )) AS breakdowns,
  (SELECT json_group_array(json_object(
    'period', period, 'name', name, 'events', events, 'installs', installs
  )) FROM (
    SELECT period, app_version AS name, COUNT(*) AS events,
           COUNT(DISTINCT install_id) AS installs
    FROM windowed GROUP BY period, app_version
  )) AS versions,
  (SELECT json_group_array(json_object(
    'period', period, 'name', name, 'events', events, 'installs', installs
  )) FROM (
    SELECT period, os || ' / ' || arch AS name, COUNT(*) AS events,
           COUNT(DISTINCT install_id) AS installs
    FROM windowed GROUP BY period, os, arch
  )) AS platforms,
  (SELECT MAX(received_at) FROM events) AS latestReceivedAt`;
}

function errorText(error) {
  try {
    const failure = JSON.parse(error?.stdout);
    const detail = failure?.error?.notes?.[0]?.text || failure?.error?.text;
    if (detail) return String(detail);
  } catch {
    // Wrangler also writes ordinary failures to stderr.
  }
  return String(error?.stderr || error?.message || error)
    .trim()
    .split("\n")[0];
}

async function executeWrangler({ local, sql }) {
  if (!existsSync(wranglerScript)) {
    throw new Error(
      "Wrangler is not installed. Run `npm ci` in collector/ first.",
    );
  }

  try {
    const { stdout } = await execFile(
      process.execPath,
      [
        wranglerScript,
        "d1",
        "execute",
        "flume-usage",
        local ? "--local" : "--remote",
        "--command",
        sql,
        "--json",
      ],
      { cwd: collectorDir, timeout: 30_000, maxBuffer: 10 * 1024 * 1024 },
    );
    return stdout;
  } catch (error) {
    const detail = errorText(error);
    if (local && /no such table: events/i.test(detail)) {
      throw new Error(
        "The local D1 database has no events table. Run `npm run migrate:local` in collector/ first.",
        { cause: error },
      );
    }
    throw new Error(
      `Could not read ${local ? "local" : "remote"} Flume usage data with Wrangler: ${detail}`,
      { cause: error },
    );
  }
}

function integer(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`Wrangler returned an invalid ${label}`);
  }
  return value;
}

function optionalInteger(value, label) {
  return value === null ? null : integer(value, label);
}

function string(value, label) {
  if (typeof value !== "string") {
    throw new Error(`Wrangler returned an invalid ${label}`);
  }
  return value;
}

function parseRows(stdout) {
  let payload;
  try {
    payload = JSON.parse(stdout);
  } catch {
    throw new Error("Wrangler did not return JSON for the D1 query");
  }
  if (
    !Array.isArray(payload) ||
    payload.length !== 1 ||
    payload[0]?.success !== true ||
    !Array.isArray(payload[0]?.results) ||
    payload[0].results.length !== 1
  ) {
    throw new Error("Wrangler did not return a successful D1 result");
  }

  const result = payload[0].results[0];
  const rows = [];
  for (const [column, section] of [
    ["summaries", "summary"],
    ["daily", "daily"],
    ["events", "events"],
    ["breakdowns", "breakdowns"],
    ["versions", "versions"],
    ["platforms", "platforms"],
  ]) {
    if (typeof result?.[column] !== "string") {
      throw new Error(`Wrangler returned no ${column} aggregate`);
    }
    let group;
    try {
      group = JSON.parse(result[column]);
    } catch {
      throw new Error(`Wrangler returned invalid ${column} aggregate JSON`);
    }
    if (!Array.isArray(group)) {
      throw new Error(`Wrangler returned an invalid ${column} aggregate`);
    }
    rows.push(...group.map((row) => ({ ...row, section })));
  }
  rows.push({
    section: "freshness",
    latestReceivedAt: result.latestReceivedAt,
  });
  return rows;
}

function emptyRange() {
  return {
    summary: null,
    daily: [],
    events: [],
    breakdowns: [],
    versions: [],
    platforms: [],
  };
}

/**
 * Read owner-only aggregates from D1. No HTTP route or browser token is needed.
 * `execute` and `now` can be supplied by tests; normal callers only choose
 * `local` (defaults to the deployed database).
 */
export async function loadDashboardData({
  local = false,
  now = Math.floor(Date.now() / 1000),
  execute = executeWrangler,
} = {}) {
  const ranges = Object.fromEntries(
    periods.map((period) => [period, emptyRange()]),
  );
  let latestReceivedAt;

  let stdout;
  try {
    stdout = await execute({ local, sql: dashboardSql(now) });
  } catch (error) {
    const detail = errorText(error);
    if (local && /no such table: events/i.test(detail)) {
      throw new Error(
        "The local D1 database has no events table. Run `npm run migrate:local` in collector/ first.",
        { cause: error },
      );
    }
    throw error;
  }

  for (const row of parseRows(stdout)) {
    if (row?.section === "freshness") {
      if (latestReceivedAt !== undefined) {
        throw new Error("Wrangler returned duplicate freshness rows");
      }
      latestReceivedAt = optionalInteger(
        row.latestReceivedAt,
        "latest received time",
      );
      continue;
    }

    const range = ranges[row?.period];
    if (!range) {
      throw new Error("Wrangler returned an unknown dashboard period");
    }
    switch (row.section) {
      case "summary":
        if (range.summary !== null) {
          throw new Error("Wrangler returned a duplicate dashboard summary");
        }
        range.summary = {
          events: integer(row.events, "event count"),
          installs: integer(row.installs, "install count"),
          firstAt: optionalInteger(row.firstAt, "first event time"),
          lastAt: optionalInteger(row.lastAt, "last event time"),
        };
        break;
      case "daily":
        if (!/^\d{4}-\d{2}-\d{2}$/.test(row.day)) {
          throw new Error("Wrangler returned an invalid event day");
        }
        range.daily.push({
          day: row.day,
          installs: integer(row.installs, "daily install count"),
          events: integer(row.events, "daily event count"),
        });
        break;
      case "events":
        range.events.push({
          name: string(row.name, "event name"),
          count: integer(row.count, "event count"),
        });
        break;
      case "breakdowns":
        range.breakdowns.push({
          name: string(row.name, "breakdown event name"),
          value: string(row.value, "breakdown value"),
          count: integer(row.count, "breakdown count"),
        });
        break;
      case "versions":
      case "platforms":
        range[row.section].push({
          name: string(row.name, `${row.section} name`),
          installs: integer(row.installs, `${row.section} install count`),
          events: integer(row.events, `${row.section} event count`),
        });
        break;
      default:
        throw new Error("Wrangler returned an unknown dashboard section");
    }
  }

  if (
    latestReceivedAt === undefined ||
    periods.some((period) => !ranges[period].summary)
  ) {
    throw new Error("Wrangler returned incomplete dashboard aggregates");
  }
  for (const range of Object.values(ranges)) {
    range.daily.sort((a, b) => a.day.localeCompare(b.day));
    range.events.sort(
      (a, b) => b.count - a.count || a.name.localeCompare(b.name),
    );
    range.breakdowns.sort(
      (a, b) =>
        b.count - a.count ||
        a.name.localeCompare(b.name) ||
        a.value.localeCompare(b.value),
    );
    for (const section of ["versions", "platforms"]) {
      range[section].sort(
        (a, b) =>
          b.installs - a.installs ||
          b.events - a.events ||
          a.name.localeCompare(b.name),
      );
    }
  }

  return {
    generatedAt: now,
    source: local ? "local" : "remote",
    latestReceivedAt,
    ranges,
  };
}
