-- The outbox backlog counter (health reads it instead of counting the
-- ledger). Structure and triggers as the base schema; every consumer with
-- rows is queued for one recount by the outbox_backlog_rollup job.
-- @cost: rows=30778 measured=25ms triggers=live

-- Rows waiting per consumer, kept current by the statement triggers below
-- so a health read never counts the ledger. A key queued for the
-- `event_outbox_backlog` projection recounts one consumer exactly.
CREATE TABLE IF NOT EXISTS event_outbox_backlog (
    consumer TEXT PRIMARY KEY,
    pending BIGINT NOT NULL DEFAULT 0 CHECK (pending >= 0)
);

CREATE OR REPLACE FUNCTION event_outbox_backlog_on_insert()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO event_outbox_backlog (consumer, pending)
    SELECT n.consumer, count(*) FROM new_rows n
    WHERE n.consumer IS NOT NULL AND n.processed_at IS NULL
    GROUP BY n.consumer
    ON CONFLICT (consumer) DO UPDATE
    SET pending = event_outbox_backlog.pending + EXCLUDED.pending;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION event_outbox_backlog_on_update()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO event_outbox_backlog (consumer, pending)
    SELECT DISTINCT n.consumer, 0 FROM new_rows n
    WHERE n.consumer IS NOT NULL AND n.processed_at IS NULL
    ON CONFLICT (consumer) DO NOTHING;
    UPDATE event_outbox_backlog b
    SET pending = GREATEST(b.pending + d.change, 0)
    FROM (
        SELECT consumer, sum(change) AS change FROM (
            SELECT n.consumer, 1 AS change FROM new_rows n
            WHERE n.consumer IS NOT NULL AND n.processed_at IS NULL
            UNION ALL
            SELECT o.consumer, -1 FROM old_rows o
            WHERE o.consumer IS NOT NULL AND o.processed_at IS NULL
        ) moves GROUP BY consumer HAVING sum(change) <> 0
    ) d WHERE b.consumer = d.consumer;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION event_outbox_backlog_on_delete()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    UPDATE event_outbox_backlog b
    SET pending = GREATEST(b.pending - gone.n, 0)
    FROM (
        SELECT o.consumer, count(*) AS n FROM old_rows o
        WHERE o.consumer IS NOT NULL AND o.processed_at IS NULL
        GROUP BY o.consumer
    ) gone WHERE b.consumer = gone.consumer;
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS event_outbox_backlog_insert ON event_outbox;
CREATE TRIGGER event_outbox_backlog_insert
    AFTER INSERT ON event_outbox REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION event_outbox_backlog_on_insert();
DROP TRIGGER IF EXISTS event_outbox_backlog_update ON event_outbox;
CREATE TRIGGER event_outbox_backlog_update
    AFTER UPDATE ON event_outbox REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION event_outbox_backlog_on_update();
DROP TRIGGER IF EXISTS event_outbox_backlog_delete ON event_outbox;
CREATE TRIGGER event_outbox_backlog_delete
    AFTER DELETE ON event_outbox REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION event_outbox_backlog_on_delete();

-- Recounts the queued consumers from the ledger: the projector an upgrade
-- drains after it queues every consumer, and the repair for a counter that
-- drifted.
CREATE OR REPLACE FUNCTION refresh_event_outbox_backlog(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE written BIGINT;
BEGIN
    WITH counted AS (
        INSERT INTO event_outbox_backlog (consumer, pending)
        SELECT k, (SELECT count(*) FROM event_outbox e
                   WHERE e.consumer = k AND e.processed_at IS NULL)
        FROM unnest(keys) AS k
        ON CONFLICT (consumer) DO UPDATE SET pending = EXCLUDED.pending
        RETURNING 1)
    SELECT count(*) INTO written FROM counted;
    RETURN written;
END $$;

INSERT INTO projection_backlog (projection, key)
SELECT DISTINCT 'event_outbox_backlog', consumer FROM event_outbox
WHERE consumer IS NOT NULL
ON CONFLICT DO NOTHING;
