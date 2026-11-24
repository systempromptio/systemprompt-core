-- Queued projection keys reach `projection_backlog` when the queueing
-- transaction commits, sorted, instead of from each statement that queued
-- them: two transactions queueing overlapping keys in different orders
-- (concurrent publishes over shared owners) no longer deadlock. Adds the
-- unlogged staging table and its deferred flush; `queue_projection_backlog`
-- keeps its signature. No existing table is locked or rewritten.

-- Keys a transaction has queued, held until it commits. Inserting a key
-- into `projection_backlog` waits on another transaction's uncommitted
-- insert of the same key, so two transactions that queue overlapping keys
-- from several statements in different orders would deadlock; staged keys
-- reach the queue once, at commit, sorted, so every transaction takes the
-- queue's key locks in one global order and holds them only while it
-- commits. Unlogged: a row lives only inside its own open transaction.
CREATE UNLOGGED TABLE IF NOT EXISTS projection_backlog_staged (
    xact xid8 NOT NULL DEFAULT pg_current_xact_id(),
    projection TEXT NOT NULL,
    key TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_projection_backlog_staged_xact
    ON projection_backlog_staged(xact);

-- Moves the committing transaction's staged keys into the queue in
-- (projection, key) order; a key already queued stays where it is. The
-- first deferred firing moves them all, later firings find none.
CREATE OR REPLACE FUNCTION flush_projection_backlog_staged()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    WITH mine AS (
        DELETE FROM projection_backlog_staged WHERE xact = pg_current_xact_id()
        RETURNING projection, key
    )
    INSERT INTO projection_backlog(projection, key)
    SELECT DISTINCT projection, key FROM mine ORDER BY projection, key
    ON CONFLICT DO NOTHING;
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS projection_backlog_staged_flush ON projection_backlog_staged;
CREATE CONSTRAINT TRIGGER projection_backlog_staged_flush
    AFTER INSERT ON projection_backlog_staged
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION flush_projection_backlog_staged();

-- Queues keys for a projection: they reach `projection_backlog` when the
-- calling transaction commits (a statement outside a transaction commits at
-- its end). Returns how many distinct keys were queued.
CREATE OR REPLACE FUNCTION queue_projection_backlog(projection_name TEXT, keys TEXT[])
RETURNS BIGINT LANGUAGE sql AS $$
WITH staged AS (
    INSERT INTO projection_backlog_staged(projection, key)
    SELECT DISTINCT projection_name, k FROM unnest(keys) AS k WHERE k IS NOT NULL
    RETURNING 1)
SELECT COUNT(*) FROM staged
$$;
