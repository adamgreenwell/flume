-- Flume usage collector.
--
-- One row per event, already bucketed and hour-truncated by the client. There
-- is deliberately no column for an IP address, a User-Agent, or anything free
-- text: the Worker never reads the first two, and the schema below has nowhere
-- to put the third even if a future client bug tried to send one.

CREATE TABLE IF NOT EXISTS events (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  -- Random v4 UUID generated on the user's machine when they consented.
  -- Not a machine id, and deleted on their side when consent is withdrawn.
  install_id   TEXT    NOT NULL,
  app_version  TEXT    NOT NULL,
  os           TEXT    NOT NULL,
  arch         TEXT    NOT NULL,
  -- Event name, from the allowlist in schema.json.
  event        TEXT    NOT NULL,
  -- The event's single bounded field, if it has one. Both NULL for events
  -- that carry nothing. One generic column rather than one per event keeps a
  -- new event type from needing a migration.
  field        TEXT,
  value        TEXT,
  -- Unix seconds, truncated to the hour by the client.
  at           INTEGER NOT NULL,
  -- When the collector stored it. The only server-side timestamp, and the
  -- only thing here the client did not choose.
  received_at  INTEGER NOT NULL
);

-- The three queries this exists to answer: counts over time, counts per
-- version, and how many distinct installs are represented.
CREATE INDEX IF NOT EXISTS events_at      ON events (at);
CREATE INDEX IF NOT EXISTS events_event   ON events (event, at);
CREATE INDEX IF NOT EXISTS events_install ON events (install_id, at);

-- Events the collector discarded, as counts. A batch is validated one event
-- at a time: the valid ones go into `events` and each invalid one adds one
-- here, so a single bad event no longer costs the whole batch -- and the
-- drops stay visible instead of silent. A spike in `unknownEvent` or
-- `unknownValue` for a new app_version is the signature of a release that
-- went out before this Worker was redeployed with its schema.
--
-- Nothing a client chose freely is stored. `event` is a name from the
-- schema's allowlist, or '' when the client sent one that is not on it -- the
-- unrecognised name itself is never kept, because it is the one string here a
-- client could put anything into. No install id, and no rejected value.
CREATE TABLE IF NOT EXISTS rejections (
  -- When the collector received them, truncated to the hour.
  hour         INTEGER NOT NULL,
  app_version  TEXT    NOT NULL,
  os           TEXT    NOT NULL,
  event        TEXT    NOT NULL,
  -- One of the `Reason` values in src/index.ts.
  reason       TEXT    NOT NULL,
  count        INTEGER NOT NULL,
  PRIMARY KEY (hour, app_version, os, event, reason)
);
