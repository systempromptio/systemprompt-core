-- The one queue every projection drains from.
--
-- A projection is one denormalised row per entity, rebuilt by a
-- `refresh_<projection>(keys TEXT[])` function from the ledger rows it
-- summarises. Writers never call the refresh inline: a statement trigger on
-- the ledger queues the touched keys here, and the projection's drainer job
-- takes a batch, refreshes it and deletes the rows in one transaction
-- (`systemprompt_database::projection::drain`). A migration that adds a
-- projection to an established database queues the keys it must rebuild
-- here instead of rebuilding them on the boot path, so an upgrade costs one
-- keys-only INSERT however large the history is. A fresh install never runs
-- that migration (migrations are stamped on an empty database), so its
-- projections fill from the triggers alone.
--
-- `attempts` counts failed refreshes of a key; the drainer leaves a key that
-- failed more than its retry budget queued and reports it, so one poisoned
-- row never blocks the rest of the queue. Depth per projection is what
-- "facts pending" on an admin page shows: NULL in a fact column means
-- pending, never "compute it from the ledger instead".
CREATE TABLE IF NOT EXISTS projection_backlog (
    projection TEXT NOT NULL,
    key TEXT NOT NULL,
    queued_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    attempts INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (projection, key)
);
CREATE INDEX IF NOT EXISTS idx_projection_backlog_order
    ON projection_backlog(projection, queued_at, key);

-- Queues keys for a projection; a key already queued stays where it is.
-- Returns how many were newly queued.
CREATE OR REPLACE FUNCTION queue_projection_backlog(projection_name TEXT, keys TEXT[])
RETURNS BIGINT LANGUAGE sql AS $$
WITH queued AS (
    INSERT INTO projection_backlog(projection, key)
    SELECT projection_name, k FROM unnest(keys) AS k WHERE k IS NOT NULL
    ON CONFLICT DO NOTHING
    RETURNING 1)
SELECT COUNT(*) FROM queued
$$;

-- Takes up to `batch` queued keys of one projection, oldest first, removing
-- them from the queue in the caller's transaction. SKIP LOCKED lets two
-- drainers work the same queue without taking the same key twice; a
-- transaction that rolls back puts its keys straight back. Keys past
-- `max_attempts` are left queued for an operator.
--
-- The batch is picked once (a materialized CTE) and deleted by key through
-- the primary key. Written as `key IN (<locking subquery>)`, a migration
-- that queues tens of thousands of keys into a table last analyzed empty
-- gets a nested-loop plan that re-runs the locking subquery per queued row:
-- quadratic, and a 64k-key backlog never finished its first batch.
CREATE OR REPLACE FUNCTION take_projection_backlog(projection_name TEXT, batch INTEGER, max_attempts INTEGER DEFAULT 5)
RETURNS TEXT[] LANGUAGE sql AS $$
WITH picked AS MATERIALIZED (
    SELECT q.key FROM projection_backlog q
    WHERE q.projection = projection_name AND q.attempts <= max_attempts
    ORDER BY q.queued_at, q.key
    LIMIT GREATEST(batch, 1)
    FOR UPDATE SKIP LOCKED
), taken AS (
    DELETE FROM projection_backlog b
    WHERE b.projection = projection_name AND b.key = ANY (ARRAY(SELECT p.key FROM picked p))
    RETURNING b.key)
SELECT COALESCE(array_agg(key), '{}'::TEXT[]) FROM taken
$$;

-- Re-queues keys whose refresh failed, one attempt dearer. Called by the
-- drainer after it rolled the failed batch back.
CREATE OR REPLACE FUNCTION requeue_projection_backlog(projection_name TEXT, keys TEXT[])
RETURNS BIGINT LANGUAGE sql AS $$
WITH queued AS (
    INSERT INTO projection_backlog(projection, key, attempts)
    SELECT projection_name, k, 1 FROM unnest(keys) AS k WHERE k IS NOT NULL
    ON CONFLICT (projection, key) DO UPDATE SET attempts = projection_backlog.attempts + 1
    RETURNING 1)
SELECT COUNT(*) FROM queued
$$;

-- Keys still pending for one projection ("facts pending").
CREATE OR REPLACE FUNCTION projection_backlog_depth(projection_name TEXT)
RETURNS BIGINT LANGUAGE sql STABLE AS $$
SELECT COUNT(*) FROM projection_backlog WHERE projection = projection_name
$$;
