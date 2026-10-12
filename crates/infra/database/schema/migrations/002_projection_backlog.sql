-- The generic projection queue (schema/projection_backlog.sql) on an
-- established database. Structure only: the functions are declarative and
-- install from the schema file on every boot; nothing is backfilled here.
-- A fresh install stamps this and creates the table from the schema file.
CREATE TABLE IF NOT EXISTS projection_backlog (
    projection TEXT NOT NULL,
    key TEXT NOT NULL,
    queued_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    attempts INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (projection, key)
);
CREATE INDEX IF NOT EXISTS idx_projection_backlog_order
    ON projection_backlog(projection, queued_at, key);
