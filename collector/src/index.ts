/**
 * Flume usage collector.
 *
 * The privacy promise is enforced here, not in the client. The client is
 * best-effort code running on someone else's machine; this is the thing that
 * cannot be talked into storing a torrent name. Every event is checked against
 * `schema.json` and anything that does not match exactly — an unknown event, an
 * unknown field, a value outside the allowlist — is discarded rather than
 * stored. A future bug in the Rust client therefore cannot widen what is
 * collected without a deliberate change to this file.
 *
 * Validation is per event, and the envelope is the only all-or-nothing part.
 * It used to be the whole batch: the first bad event threw, the batch got a
 * 400, and the client kept it and retried hourly — so a client bug that
 * produced one bad kind of event silenced that install for good, because
 * every later batch carried another. Now the valid events are stored, each
 * discarded one is counted in `rejections` by reason, and the reply is 204.
 * The client needs no change and old releases benefit at once.
 *
 * Counts are approximate by design. Delivery is at-least-once and there is no
 * dedupe table: a response lost after the row was written means the client
 * retries and a handful of events are counted twice. For aggregate counters
 * that is a better trade than the storage and complexity of exact-once, and
 * saying so is more honest than implying a precision the transport cannot give.
 */

import SCHEMA from "../schema.json";

/** Bindings this Worker expects. */
export interface Env {
  /** The D1 database holding `events`. */
  DB: D1Database;
}

/** Largest body accepted, before parsing. */
const MAX_BODY_BYTES = 256 * 1024;

/** Largest number of events in one batch. Mirrors the client's queue cap. */
const MAX_EVENTS = 5_000;

/** Seconds in an hour; every `at` must be a multiple. */
const HOUR = 3_600;

/** How far in the past an event may be dated. Matches the client's retry cap. */
const MAX_AGE_SECONDS = 4 * 24 * HOUR;

/** How far into the future an event may be dated, allowing for clock skew. */
const MAX_SKEW_SECONDS = 2 * HOUR;

/** A v4 UUID, which is the only form of install id the client produces. */
const INSTALL_ID =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

/** A semver-ish version, which is all `CARGO_PKG_VERSION` ever is. */
const APP_VERSION = /^\d{1,4}\.\d{1,4}\.\d{1,4}(?:-[0-9A-Za-z.-]{1,32})?$/;

/** One row ready to insert. */
interface Row {
  event: string;
  field: string | null;
  value: string | null;
  at: number;
}

/** Why a batch was rejected. Returned to the client, never stored. */
class Invalid extends Error {}

/**
 * Why one event was discarded. A closed list, because it is stored.
 *
 * - `notAnObject` — not a JSON object, or no string `event` name.
 * - `unknownEvent` — a name that is not in the schema. Release drift looks
 *   like this: a client sending an event the deployed Worker predates.
 * - `unexpectedKeys` — keys other than exactly `at`, `event` and the
 *   schema's field. The case the privacy promise is about.
 * - `badTimestamp` — `at` is not an integer number of hours.
 * - `tooOld` / `tooNew` — `at` is outside the accepted window. The client
 *   prunes at three days against this four, so either means a wrong clock.
 * - `unknownValue` — the field's value is not on its allowlist; drift again,
 *   when a release adds a value such as a new settings key.
 */
export type Reason =
  | "notAnObject"
  | "unknownEvent"
  | "unexpectedKeys"
  | "badTimestamp"
  | "tooOld"
  | "tooNew"
  | "unknownValue";

/**
 * One event discarded. `event` is the name only when it is on the schema's
 * allowlist and `null` otherwise, so an unrecognised name never reaches a
 * row: it is the one string in an event a client could fill with anything.
 */
class Rejected extends Error {
  constructor(
    readonly reason: Reason,
    readonly event: string | null,
  ) {
    super(reason);
  }
}

/**
 * `schema.json` itself is wrong. A collector bug, not a client one, so the
 * client is told to keep the batch (503) until a fixed Worker is deployed
 * rather than having its events counted as rejected.
 */
class SchemaBroken extends Error {}

/** Narrows an unknown value to a plain JSON object. */
function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Whether an object's keys are exactly `expected`, in any order.
 *
 * Refusing unknown keys rather than ignoring them is the whole mechanism:
 * stripping an unexpected field and storing the rest would let a client start
 * sending one and nobody would notice until it was already in the database.
 * An event with an extra key is discarded whole and counted.
 */
function hasExactKeys(
  value: Record<string, unknown>,
  expected: string[],
): boolean {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  return (
    actual.length === wanted.length && actual.every((k, i) => k === wanted[i])
  );
}

/** Validates one event and flattens it into a row, or throws `Rejected`. */
function validateEvent(raw: unknown, now: number): Row {
  if (!isObject(raw)) throw new Rejected("notAnObject", null);

  const name = raw.event;
  if (typeof name !== "string") throw new Rejected("notAnObject", null);

  // `Object.hasOwn`, not a lookup: `toString` or `__proto__` sent as a name
  // would otherwise find something on the prototype.
  const events = SCHEMA.events as Record<string, Record<string, unknown[]>>;
  const fields = Object.hasOwn(events, name) ? events[name] : undefined;
  if (fields === undefined) throw new Rejected("unknownEvent", null);

  const fieldNames = Object.keys(fields);
  if (!hasExactKeys(raw, ["at", "event", ...fieldNames])) {
    throw new Rejected("unexpectedKeys", name);
  }

  const at = raw.at;
  if (typeof at !== "number" || !Number.isInteger(at) || at % HOUR !== 0) {
    throw new Rejected("badTimestamp", name);
  }
  // A timestamp outside this window is a broken clock or a replay, and either
  // way it would distort every time series it landed in.
  if (at < now - MAX_AGE_SECONDS) throw new Rejected("tooOld", name);
  if (at > now + MAX_SKEW_SECONDS) throw new Rejected("tooNew", name);

  if (fieldNames.length === 0) {
    return { event: name, field: null, value: null, at };
  }

  // Every event in the schema carries at most one field, which is what lets
  // the table be (field, value) rather than a column per event. Checked at
  // runtime as well as pinned by `every_event_carries_at_most_one_field` in
  // src-tauri/tests/usage_contract.rs, so a schema edit that broke the
  // assumption would be refused here rather than half-stored.
  const field = fieldNames[0];
  if (field === undefined || fieldNames.length > 1) {
    throw new SchemaBroken(`${name} must declare exactly one field`);
  }

  const allowed = fields[field];
  if (allowed === undefined) {
    throw new SchemaBroken(`${name}.${field} has no allowed values`);
  }

  const value = raw[field];
  if (!allowed.includes(value as never)) {
    throw new Rejected("unknownValue", name);
  }

  return { event: name, field, value: String(value), at };
}

/** Discarded events of one kind, ready to add to `rejections`. */
interface Dropped {
  event: string;
  reason: Reason;
  count: number;
}

/**
 * Validates a whole envelope and flattens it into rows to store and counts
 * of what was discarded.
 *
 * Throws `Invalid` only for the envelope, which every event shares, and
 * `SchemaBroken` for the collector's own schema. An invalid event is never
 * thrown out of here: it is counted and the rest go on.
 */
function validate(
  body: unknown,
  now: number,
): {
  installId: string;
  appVersion: string;
  os: string;
  arch: string;
  rows: Row[];
  dropped: Dropped[];
} {
  if (!isObject(body)) throw new Invalid("envelope: not an object");
  if (!hasExactKeys(body, SCHEMA.envelope.required)) {
    const got = Object.keys(body).sort().join(", ");
    throw new Invalid(
      `envelope: expected keys [${[...SCHEMA.envelope.required].sort().join(", ")}], got [${got}]`,
    );
  }

  if (body.schema !== SCHEMA.schema) {
    throw new Invalid(`envelope: unsupported schema ${String(body.schema)}`);
  }

  const installId = body.installId;
  if (typeof installId !== "string" || !INSTALL_ID.test(installId)) {
    throw new Invalid("envelope: installId is not a v4 UUID");
  }

  const appVersion = body.appVersion;
  if (typeof appVersion !== "string" || !APP_VERSION.test(appVersion)) {
    throw new Invalid("envelope: appVersion is not a version");
  }

  const os = body.os;
  if (typeof os !== "string" || !SCHEMA.envelope.os.includes(os)) {
    throw new Invalid("envelope: unknown os");
  }

  const arch = body.arch;
  if (typeof arch !== "string" || !SCHEMA.envelope.arch.includes(arch)) {
    throw new Invalid("envelope: unknown arch");
  }

  const events = body.events;
  if (!Array.isArray(events) || events.length === 0) {
    throw new Invalid("envelope: events must be a non-empty array");
  }
  if (events.length > MAX_EVENTS) {
    throw new Invalid("envelope: too many events");
  }

  const rows: Row[] = [];
  const dropped = new Map<string, Dropped>();
  for (const event of events) {
    try {
      rows.push(validateEvent(event, now));
    } catch (thrown) {
      if (!(thrown instanceof Rejected)) throw thrown;
      // '' rather than null for an unrecognised name: it is part of the
      // table's primary key, and SQLite treats every NULL as distinct there,
      // so NULLs would never aggregate.
      const name = thrown.event ?? "";
      const key = `${thrown.reason}\u0000${name}`;
      const seen = dropped.get(key);
      if (seen) seen.count += 1;
      else dropped.set(key, { event: name, reason: thrown.reason, count: 1 });
    }
  }

  return {
    installId,
    appVersion,
    os,
    arch,
    rows,
    dropped: [...dropped.values()],
  };
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (request.method !== "POST" || url.pathname !== "/v1/usage") {
      return new Response("not found\n", { status: 404 });
    }

    // Checked before reading, so an oversized body is refused rather than
    // buffered.
    const declared = Number(request.headers.get("content-length") ?? "0");
    if (declared > MAX_BODY_BYTES) {
      return new Response("payload too large\n", { status: 413 });
    }

    let body: unknown;
    try {
      const text = await request.text();
      if (text.length > MAX_BODY_BYTES) {
        return new Response("payload too large\n", { status: 413 });
      }
      body = JSON.parse(text);
    } catch {
      return new Response("invalid json\n", { status: 400 });
    }

    const now = Math.floor(Date.now() / 1000);
    let batch: ReturnType<typeof validate>;
    try {
      batch = validate(body, now);
    } catch (thrown) {
      if (thrown instanceof SchemaBroken) {
        return new Response("collector schema is broken\n", { status: 503 });
      }
      // The reason goes back to the client, which logs it at debug level. It
      // is never stored: a rejection message can quote the offending value.
      const reason = thrown instanceof Invalid ? thrown.message : "invalid";
      return new Response(`${reason}\n`, { status: 400 });
    }

    // Note what is *not* read anywhere above or below: `CF-Connecting-IP`,
    // `User-Agent`, and any other request header. The row carries only what
    // the envelope declared.
    const statement = env.DB.prepare(
      `INSERT INTO events
         (install_id, app_version, os, arch, event, field, value, at, received_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    );

    const tally = env.DB.prepare(
      `INSERT INTO rejections (hour, app_version, os, event, reason, count)
       VALUES (?, ?, ?, ?, ?, ?)
       ON CONFLICT (hour, app_version, os, event, reason)
       DO UPDATE SET count = count + excluded.count`,
    );
    const hour = now - (now % HOUR);

    // One batch, so D1 runs it as one transaction: the events and the count
    // of what was dropped from them land together or not at all. Never empty
    // -- the envelope requires at least one event, and each is either a row
    // or part of a tally.
    try {
      await env.DB.batch([
        ...batch.rows.map((row) =>
          statement.bind(
            batch.installId,
            batch.appVersion,
            batch.os,
            batch.arch,
            row.event,
            row.field,
            row.value,
            row.at,
            now,
          ),
        ),
        ...batch.dropped.map((d) =>
          tally.bind(
            hour,
            batch.appVersion,
            batch.os,
            d.event,
            d.reason,
            d.count,
          ),
        ),
      ]);
    } catch {
      // A 5xx tells the client to keep the batch and try again later.
      return new Response("could not store\n", { status: 503 });
    }

    // 204 even when events were dropped: retrying would drop them again. The
    // count is a header so a person testing with curl can see it; the client
    // does not read it.
    const discarded = batch.dropped.reduce((sum, d) => sum + d.count, 0);
    return new Response(null, {
      status: 204,
      headers:
        discarded > 0 ? { "Flume-Discarded": String(discarded) } : undefined,
    });
  },
};
