/**
 * Flume usage collector.
 *
 * The privacy promise is enforced here, not in the client. The client is
 * best-effort code running on someone else's machine; this is the thing that
 * cannot be talked into storing a torrent name. Every event is checked against
 * `schema.json` and anything that does not match exactly — an unknown event, an
 * unknown field, a value outside the allowlist — is rejected rather than
 * stored. A future bug in the Rust client therefore cannot widen what is
 * collected without a deliberate change to this file.
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

/** Narrows an unknown value to a plain JSON object. */
function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Checks an object's keys are exactly `expected`, in any order.
 *
 * Rejecting unknown keys rather than ignoring them is the whole mechanism:
 * silently dropping an unexpected field would let a client start sending one
 * and nobody would notice until it was already in the database.
 */
function requireExactKeys(
  value: Record<string, unknown>,
  expected: string[],
  what: string,
): void {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (
    actual.length !== wanted.length ||
    actual.some((k, i) => k !== wanted[i])
  ) {
    throw new Invalid(
      `${what}: expected keys [${wanted.join(", ")}], got [${actual.join(", ")}]`,
    );
  }
}

/** Validates one event and flattens it into a row. */
function validateEvent(raw: unknown, now: number): Row {
  if (!isObject(raw)) throw new Invalid("event: not an object");

  const name = raw.event;
  if (typeof name !== "string") throw new Invalid("event: missing name");

  const fields = (SCHEMA.events as Record<string, Record<string, unknown[]>>)[
    name
  ];
  if (fields === undefined) throw new Invalid(`event: unknown type ${name}`);

  const fieldNames = Object.keys(fields);
  requireExactKeys(raw, ["at", "event", ...fieldNames], `event ${name}`);

  const at = raw.at;
  if (typeof at !== "number" || !Number.isInteger(at) || at % HOUR !== 0) {
    throw new Invalid(`event ${name}: 'at' must be an integer hour`);
  }
  // A timestamp outside this window is a broken clock or a replay, and either
  // way it would distort every time series it landed in.
  if (at < now - MAX_AGE_SECONDS || at > now + MAX_SKEW_SECONDS) {
    throw new Invalid(`event ${name}: 'at' is outside the accepted window`);
  }

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
    throw new Invalid(
      `event ${name}: the schema must declare exactly one field`,
    );
  }

  const allowed = fields[field];
  if (allowed === undefined) {
    throw new Invalid(`event ${name}: '${field}' has no allowed values`);
  }

  const value = raw[field];
  if (!allowed.includes(value as never)) {
    throw new Invalid(`event ${name}: '${field}' is not an allowed value`);
  }

  return { event: name, field, value: String(value), at };
}

/** Validates a whole envelope and flattens it into rows. */
function validate(
  body: unknown,
  now: number,
): {
  installId: string;
  appVersion: string;
  os: string;
  arch: string;
  rows: Row[];
} {
  if (!isObject(body)) throw new Invalid("envelope: not an object");
  requireExactKeys(body, SCHEMA.envelope.required, "envelope");

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

  return {
    installId,
    appVersion,
    os,
    arch,
    rows: events.map((event) => validateEvent(event, now)),
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

    try {
      await env.DB.batch(
        batch.rows.map((row) =>
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
      );
    } catch {
      // A 5xx tells the client to keep the batch and try again later.
      return new Response("could not store\n", { status: 503 });
    }

    return new Response(null, { status: 204 });
  },
};
