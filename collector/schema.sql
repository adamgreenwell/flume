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
