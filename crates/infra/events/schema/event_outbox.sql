CREATE TABLE IF NOT EXISTS event_outbox (
    id TEXT PRIMARY KEY,
    channel TEXT NOT NULL,
    user_id TEXT NOT NULL,
    payload JSONB NOT NULL,
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'job', 'mcp')),
    actor_id TEXT NOT NULL CONSTRAINT event_outbox_actor_id_nonempty CHECK (length(actor_id) > 0),
    origin_instance_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    consumer TEXT,
    fact JSONB,
    processed_at TIMESTAMPTZ,
    deliver_to_origin BOOLEAN NOT NULL DEFAULT FALSE,
    CONSTRAINT event_outbox_fact_pair CHECK (
        (consumer IS NULL AND fact IS NULL AND processed_at IS NULL)
        OR (consumer IS NOT NULL AND length(consumer) > 0 AND fact IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_event_outbox_created_at ON event_outbox(created_at);
CREATE INDEX IF NOT EXISTS idx_event_outbox_actor ON event_outbox(actor_kind, actor_id);
CREATE INDEX IF NOT EXISTS idx_event_outbox_pending
    ON event_outbox(consumer, created_at, id)
    WHERE consumer IS NOT NULL AND processed_at IS NULL;

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
