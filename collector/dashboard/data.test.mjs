import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";

import { loadDashboardData } from "./data.mjs";

const now = Date.parse("2026-09-27T12:00:00Z") / 1000;
const DAY = 86_400;

function database(t) {
  const db = new DatabaseSync(":memory:");
  db.exec(readFileSync(new URL("../schema.sql", import.meta.url), "utf8"));
  t.after(() => db.close());
  return db;
}

function readWith(db) {
  return async ({ sql }) =>
    JSON.stringify([{ success: true, results: db.prepare(sql).all() }]);
}

test("aggregates distinct installs per range and keeps identifiers out of the result", async (t) => {
  const db = database(t);
  const insert = db.prepare(`
    INSERT INTO events
      (install_id, app_version, os, arch, event, field, value, at, received_at)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
  `);
  const first = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
  const second = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
  const third = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

  insert.run(
    first,
    "1.2.0",
    "macos",
    "aarch64",
    "launched",
    null,
    null,
    now - DAY,
    now - 30,
  );
  insert.run(
    first,
    "1.2.0",
    "macos",
    "aarch64",
    "torrentAdded",
    null,
    null,
    now - DAY,
    now - 30,
  );
  insert.run(
    second,
    "1.2.0",
    "linux",
    "x86_64",
    "launched",
    null,
    null,
    now - DAY,
    now - 60,
  );
  insert.run(
    second,
    "1.1.0",
    "linux",
    "x86_64",
    "launched",
    null,
    null,
    now - 8 * DAY,
    now - 8 * DAY,
  );
  insert.run(
    third,
    "1.0.0",
    "windows",
    "x86_64",
    "launched",
    null,
    null,
    now - 50 * DAY,
    now - 50 * DAY,
  );
  insert.run(
    third,
    "1.0.0",
    "windows",
    "x86_64",
    "operationFailed",
    "kind",
    "metadataTimeout",
    now - 100 * DAY,
    now - 100 * DAY,
  );

  const data = await loadDashboardData({ now, execute: readWith(db) });

  assert.equal(data.generatedAt, now);
  assert.equal(data.source, "remote");
  assert.equal(data.latestReceivedAt, now - 30);
  assert.deepEqual(
    Object.fromEntries(
      Object.entries(data.ranges).map(([period, range]) => [
        period,
        [range.summary.events, range.summary.installs],
      ]),
    ),
    { 7: [3, 2], 30: [4, 2], 90: [5, 3], all: [6, 3] },
  );
  assert.deepEqual(data.ranges["7"].daily, [
    { day: "2026-09-26", installs: 2, events: 3 },
  ]);
  assert.deepEqual(data.ranges["7"].events, [
    { name: "launched", count: 2 },
    { name: "torrentAdded", count: 1 },
  ]);
  assert.deepEqual(data.ranges.all.breakdowns, [
    { name: "operationFailed", value: "metadataTimeout", count: 1 },
  ]);
  assert.deepEqual(data.ranges["7"].versions, [
    { name: "1.2.0", installs: 2, events: 3 },
  ]);
  assert.deepEqual(data.ranges["7"].platforms, [
    { name: "macos / aarch64", installs: 1, events: 2 },
    { name: "linux / x86_64", installs: 1, events: 1 },
  ]);
  assert.equal(JSON.stringify(data).includes(first), false);
  assert.equal(JSON.stringify(data).includes(second), false);
  assert.equal(JSON.stringify(data).includes(third), false);
});

test("calendar-day boundaries include the first day and exclude tomorrow", async (t) => {
  const db = database(t);
  const insert = db.prepare(`
    INSERT INTO events
      (install_id, app_version, os, arch, event, field, value, at, received_at)
    VALUES (?, '1.2.0', 'macos', 'aarch64', 'launched', NULL, NULL, ?, ?)
  `);
  const nearMidnight = Date.parse("2026-09-27T23:30:00Z") / 1000;
  const today = Date.parse("2026-09-27T00:00:00Z") / 1000;
  const at = (daysAgo) => today - daysAgo * DAY;

  // An event at 00:00 on the first day belongs to that range; one from the
  // preceding hour does not. The next day's 00:00 is within the collector's
  // two-hour clock-skew allowance at 23:30, but belongs to no chart yet.
  for (const timestamp of [
    at(89) - 3_600,
    at(89),
    at(29) - 3_600,
    at(29),
    at(6) - 3_600,
    at(6),
    today + DAY,
  ]) {
    insert.run("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", timestamp, nearMidnight);
  }

  const data = await loadDashboardData({
    now: nearMidnight,
    execute: readWith(db),
  });

  assert.equal(data.ranges["7"].summary.events, 1);
  assert.equal(data.ranges["7"].summary.firstAt, at(6));
  assert.deepEqual(data.ranges["7"].daily, [
    { day: "2026-09-21", installs: 1, events: 1 },
  ]);
  assert.equal(data.ranges["30"].summary.events, 3);
  assert.equal(data.ranges["30"].summary.firstAt, at(29));
  assert.equal(data.ranges["90"].summary.events, 5);
  assert.equal(data.ranges["90"].summary.firstAt, at(89));
  assert.equal(data.ranges.all.summary.events, 6);
  assert.equal(data.ranges.all.summary.lastAt, at(6));
  assert.equal(data.latestReceivedAt, nearMidnight);
});

test("returns explicit empty ranges when the database has no events", async (t) => {
  const db = database(t);
  const data = await loadDashboardData({
    local: true,
    now,
    execute: readWith(db),
  });

  assert.equal(data.source, "local");
  assert.equal(data.latestReceivedAt, null);
  for (const range of Object.values(data.ranges)) {
    assert.deepEqual(range.summary, {
      events: 0,
      installs: 0,
      firstAt: null,
      lastAt: null,
    });
    assert.deepEqual(range.daily, []);
    assert.deepEqual(range.events, []);
    assert.deepEqual(range.breakdowns, []);
    assert.deepEqual(range.versions, []);
    assert.deepEqual(range.platforms, []);
  }
});

test("explains how to initialize a missing local table", async () => {
  await assert.rejects(
    loadDashboardData({
      local: true,
      now,
      execute: async () => {
        throw new Error("D1_ERROR: no such table: events");
      },
    }),
    /npm run migrate:local/,
  );
});
