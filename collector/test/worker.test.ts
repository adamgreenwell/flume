// @vitest-environment node
/**
 * The collector, driven through its `fetch` against a fake D1 that records
 * every statement it is handed.
 *
 * Lives outside `collector/src` so the Worker's own `tsconfig` — which has
 * no vitest — never sees it, and runs from the root vitest so `npm run
 * check`, CI and preflight all cover it without a fourth gate to keep in
 * step.
 *
 * Most assertions are about what was stored, but the privacy ones are about
 * what was *not*: a rejected value or an unrecognised event name must not
 * appear in any bound argument at all, which is checked by searching every
 * argument rather than by trusting the column it would have gone in.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import worker, { type Env } from "../src/index";

/** A bound statement as the fake D1 saw it. */
interface Bound {
  sql: string;
  args: unknown[];
}

/** A fake D1 that records each batch, or fails it when told to. */
function fakeEnv(options: { fail?: boolean } = {}) {
  const batches: Bound[][] = [];
  const db = {
    prepare(sql: string) {
      return { bind: (...args: unknown[]): Bound => ({ sql, args }) };
    },
    async batch(statements: Bound[]) {
      if (options.fail) throw new Error("D1 is down");
      batches.push(statements);
      return [];
    },
  };
  const statements = () => batches.flat();
  return {
    env: { DB: db } as unknown as Env,
    batches,
    inserts: () => statements().filter((s) => s.sql.includes("INTO events")),
    tallies: () =>
      statements().filter((s) => s.sql.includes("INTO rejections")),
    /** Every argument bound anywhere, for "this never reached D1" checks. */
    allArgs: () => statements().flatMap((s) => s.args),
  };
}

// A fixed clock on an hour boundary, so the window edges are exact.
const NOW = 1_800_000_000 - (1_800_000_000 % 3_600);
const HOUR = 3_600;

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW * 1000);
});
afterEach(() => {
  vi.useRealTimers();
});

function envelope(events: unknown[], overrides: Record<string, unknown> = {}) {
  return {
    schema: 1,
    installId: "0b8e3c1e-5f0a-4c4b-9d55-2f6a1c9e7b10",
    appVersion: "1.4.1",
    os: "windows",
    arch: "x86_64",
    events,
    ...overrides,
  };
}

function post(body: unknown, path = "/v1/usage"): Request {
  return new Request(`https://collector.example${path}`, {
    method: "POST",
    body: typeof body === "string" ? body : JSON.stringify(body),
  });
}

const launched = { event: "launched", at: NOW };
const added = { event: "torrentAdded", at: NOW - HOUR };
const previewed = { event: "torrentPreviewed", at: NOW, source: "magnet" };

describe("a valid batch", () => {
  it("stores every event and reports nothing discarded", async () => {
    const db = fakeEnv();
    const res = await worker.fetch(
      post(envelope([launched, added, previewed])),
      db.env,
    );

    expect(res.status).toBe(204);
    expect(res.headers.get("Flume-Discarded")).toBeNull();
    expect(db.batches).toHaveLength(1);
    expect(db.inserts().map((s) => s.args[4])).toEqual([
      "launched",
      "torrentAdded",
      "torrentPreviewed",
    ]);
    expect(db.inserts()[2]?.args.slice(5, 7)).toEqual(["source", "magnet"]);
    expect(db.tallies()).toHaveLength(0);
  });
});

describe("one invalid event among valid ones", () => {
  it("stores the valid ones, counts the invalid one, and answers 204", async () => {
    const db = fakeEnv();
    const res = await worker.fetch(
      post(envelope([launched, { event: "launched", at: NOW + 1 }, added])),
      db.env,
    );

    // 204, not 400: this is the change. A 400 made the client keep the batch
    // and retry it, so one bad event silenced the install for good.
    expect(res.status).toBe(204);
    expect(res.headers.get("Flume-Discarded")).toBe("1");
    expect(db.inserts().map((s) => s.args[4])).toEqual([
      "launched",
      "torrentAdded",
    ]);
    expect(db.tallies()).toHaveLength(1);
    expect(db.tallies()[0]?.args).toEqual([
      NOW,
      "1.4.1",
      "windows",
      "launched",
      "badTimestamp",
      1,
    ]);
  });

  it("writes the events and the tally in one batch, so one transaction", async () => {
    const db = fakeEnv();
    await worker.fetch(
      post(envelope([launched, { event: "nope", at: NOW }])),
      db.env,
    );
    expect(db.batches).toHaveLength(1);
    expect(db.batches[0]).toHaveLength(2);
  });
});

describe("each reason", () => {
  const cases: [string, unknown, string, string][] = [
    ["not an object", "launched", "notAnObject", ""],
    ["no event name", { at: NOW }, "notAnObject", ""],
    [
      "an unknown event",
      { event: "torrentShared", at: NOW },
      "unknownEvent",
      "",
    ],
    [
      "an extra key",
      { event: "launched", at: NOW, name: "x" },
      "unexpectedKeys",
      "launched",
    ],
    [
      "a missing field",
      { event: "torrentPreviewed", at: NOW },
      "unexpectedKeys",
      "torrentPreviewed",
    ],
    [
      "a non-hour at",
      { event: "launched", at: NOW + 59 },
      "badTimestamp",
      "launched",
    ],
    [
      "a string at",
      { event: "launched", at: String(NOW) },
      "badTimestamp",
      "launched",
    ],
    [
      "an at past the age window",
      { event: "launched", at: NOW - 4 * 24 * HOUR - HOUR },
      "tooOld",
      "launched",
    ],
    [
      "an at past the skew window",
      { event: "launched", at: NOW + 3 * HOUR },
      "tooNew",
      "launched",
    ],
    [
      "a value off the allowlist",
      { event: "torrentPreviewed", at: NOW, source: "dht" },
      "unknownValue",
      "torrentPreviewed",
    ],
  ];

  it.each(cases)("%s is counted as %s", async (_, event, reason, name) => {
    const db = fakeEnv();
    const res = await worker.fetch(post(envelope([event])), db.env);
    expect(res.status).toBe(204);
    expect(db.inserts()).toHaveLength(0);
    expect(db.tallies().map((s) => s.args.slice(3, 6))).toEqual([
      [name, reason, 1],
    ]);
  });

  it("accepts both edges of the window", async () => {
    const db = fakeEnv();
    const res = await worker.fetch(
      post(
        envelope([
          { event: "launched", at: NOW - 4 * 24 * HOUR },
          { event: "launched", at: NOW + 2 * HOUR },
        ]),
      ),
      db.env,
    );
    expect(res.status).toBe(204);
    expect(db.inserts()).toHaveLength(2);
    expect(db.tallies()).toHaveLength(0);
  });
});

describe("what never reaches the database", () => {
  it("keeps no unrecognised event name", async () => {
    const db = fakeEnv();
    await worker.fetch(
      post(envelope([{ event: "ubuntu-24.04-desktop-amd64.iso", at: NOW }])),
      db.env,
    );
    expect(db.allArgs()).not.toContain("ubuntu-24.04-desktop-amd64.iso");
    expect(db.tallies()[0]?.args[3]).toBe("");
  });

  it("keeps no value from an extra key or a rejected field", async () => {
    const db = fakeEnv();
    await worker.fetch(
      post(
        envelope([
          { event: "launched", at: NOW, torrent: "secret-name" },
          {
            event: "torrentPreviewed",
            at: NOW,
            source: "tracker.example:6969",
          },
        ]),
      ),
      db.env,
    );
    const args = db.allArgs().map(String);
    expect(args.some((a) => a.includes("secret-name"))).toBe(false);
    expect(args.some((a) => a.includes("tracker.example"))).toBe(false);
  });

  it("keeps no install id in a tally", async () => {
    const db = fakeEnv();
    await worker.fetch(post(envelope([{ event: "nope", at: NOW }])), db.env);
    expect(db.tallies()[0]?.args).not.toContain(
      "0b8e3c1e-5f0a-4c4b-9d55-2f6a1c9e7b10",
    );
  });

  // The old code looked the name up with `SCHEMA.events[name]`, which finds
  // `Object.prototype.toString` -- a function, so no keys, so a field-less
  // event, so a stored row named "toString". `Object.hasOwn` closes that.
  it.each(["toString", "constructor", "__proto__", "hasOwnProperty"])(
    "treats the prototype name %s as unknown",
    async (name) => {
      const db = fakeEnv();
      await worker.fetch(post(envelope([{ event: name, at: NOW }])), db.env);
      expect(db.inserts()).toHaveLength(0);
      expect(db.tallies().map((s) => s.args.slice(3, 5))).toEqual([
        ["", "unknownEvent"],
      ]);
    },
  );
});

describe("tallies", () => {
  it("aggregate repeats of the same reason and event into one row", async () => {
    const db = fakeEnv();
    const res = await worker.fetch(
      post(
        envelope([
          { event: "nope", at: NOW },
          { event: "also-nope", at: NOW },
          { event: "launched", at: NOW + 1 },
          launched,
        ]),
      ),
      db.env,
    );
    expect(res.headers.get("Flume-Discarded")).toBe("3");
    expect(
      db
        .tallies()
        .map((s) => s.args.slice(3, 6))
        .sort(),
    ).toEqual([
      ["", "unknownEvent", 2],
      ["launched", "badTimestamp", 1],
    ]);
  });

  it("are bucketed by the hour they were received in", async () => {
    vi.setSystemTime((NOW + 1_234) * 1000);
    const db = fakeEnv();
    await worker.fetch(post(envelope([{ event: "nope", at: NOW }])), db.env);
    expect(db.tallies()[0]?.args[0]).toBe(NOW);
  });

  it("add to an existing row rather than replacing it", async () => {
    const db = fakeEnv();
    await worker.fetch(post(envelope([{ event: "nope", at: NOW }])), db.env);
    expect(db.tallies()[0]?.sql).toMatch(
      /ON CONFLICT[\s\S]*count = count \+ excluded\.count/,
    );
  });

  it("can be the whole batch when every event is invalid", async () => {
    const db = fakeEnv();
    const res = await worker.fetch(
      post(envelope([{ event: "nope", at: NOW }])),
      db.env,
    );
    expect(res.status).toBe(204);
    expect(db.inserts()).toHaveLength(0);
    expect(db.tallies()).toHaveLength(1);
  });
});

describe("the envelope is still all or nothing", () => {
  const cases: [string, unknown][] = [
    ["an unknown os", envelope([launched], { os: "haiku" })],
    ["an unknown arch", envelope([launched], { arch: "riscv64" })],
    ["another schema version", envelope([launched], { schema: 2 })],
    ["a non-v4 install id", envelope([launched], { installId: "abc" })],
    ["a bad version", envelope([launched], { appVersion: "latest" })],
    ["no events", envelope([])],
    ["events not an array", envelope([], { events: {} })],
    ["an extra envelope key", { ...envelope([launched]), hostname: "box" }],
    ["not an object", [launched]],
  ];

  it.each(cases)("%s is a 400 that stores nothing", async (_, body) => {
    const db = fakeEnv();
    const res = await worker.fetch(post(body), db.env);
    expect(res.status).toBe(400);
    expect(db.batches).toHaveLength(0);
  });

  it("invalid JSON is a 400", async () => {
    const db = fakeEnv();
    const res = await worker.fetch(post("{not json"), db.env);
    expect(res.status).toBe(400);
    expect(db.batches).toHaveLength(0);
  });
});

describe("transport", () => {
  it("answers 503 when D1 fails, so the client keeps the batch", async () => {
    const db = fakeEnv({ fail: true });
    const res = await worker.fetch(post(envelope([launched])), db.env);
    expect(res.status).toBe(503);
  });

  it("is 404 anywhere but POST /v1/usage", async () => {
    const db = fakeEnv();
    expect(
      (await worker.fetch(post(envelope([launched]), "/"), db.env)).status,
    ).toBe(404);
    const get = new Request("https://collector.example/v1/usage");
    expect((await worker.fetch(get, db.env)).status).toBe(404);
  });
});
