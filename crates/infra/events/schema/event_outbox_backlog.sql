-- Pending-row deltas per consumer, appended by the statement triggers below
-- so a health read never counts the ledger. Deltas rather than one counter
-- row: an open transaction that appended for a consumer would otherwise
-- hold that consumer's counter row and block every other append for it
-- until it committed. The sum of a consumer's deltas is its backlog; the
-- outbox_backlog_rollup job compacts the rows to one per consumer, and a
-- key queued for the `event_outbox_backlog` projection recounts one
-- consumer from the ledger.
CREATE TABLE IF NOT EXISTS event_outbox_backlog (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    consumer TEXT NOT NULL,
    delta BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_event_outbox_backlog_consumer
    ON event_outbox_backlog(consumer);

CREATE OR REPLACE FUNCTION event_outbox_backlog_on_insert()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO event_outbox_backlog (consumer, delta)
    SELECT n.consumer, count(*) FROM new_rows n
    WHERE n.consumer IS NOT NULL AND n.processed_at IS NULL
    GROUP BY n.consumer;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION event_outbox_backlog_on_update()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO event_outbox_backlog (consumer, delta)
    SELECT consumer, sum(change) FROM (
        SELECT n.consumer, 1 AS change FROM new_rows n
        WHERE n.consumer IS NOT NULL AND n.processed_at IS NULL
        UNION ALL
        SELECT o.consumer, -1 FROM old_rows o
        WHERE o.consumer IS NOT NULL AND o.processed_at IS NULL
    ) moves GROUP BY consumer HAVING sum(change) <> 0;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION event_outbox_backlog_on_delete()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO event_outbox_backlog (consumer, delta)
    SELECT o.consumer, -count(*) FROM old_rows o
    WHERE o.consumer IS NOT NULL AND o.processed_at IS NULL
    GROUP BY o.consumer;
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

-- The class of the one transaction-scoped advisory lock that serialises a
-- recount against a compaction; `systemprompt_database::OUTBOX_BACKLOG_LOCK_CLASS`
-- holds the same value. Appends never take it.
CREATE OR REPLACE FUNCTION event_outbox_backlog_lock_class()
RETURNS INTEGER LANGUAGE sql IMMUTABLE AS $ SELECT 1397751873 $;

-- Recounts the queued consumers from the ledger: the projector an upgrade
-- drains after it queues every consumer, and the repair for a sum that
-- drifted. The consumer's deltas are replaced by one row. It holds the
-- ledger lock, so a compaction is either wholly before it (its folded row is
-- deleted with the rest) or wholly after it (it folds the recount). The
-- delete and the count are one statement on one snapshot: an append that
-- committed before it is deleted and counted once, one that commits after it
-- keeps its delta row and is not counted. The rollup job compacts, it does
-- not recount, so a queued key is the only way a drift is repaired.
CREATE OR REPLACE FUNCTION refresh_event_outbox_backlog(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $
BEGIN
    PERFORM pg_advisory_xact_lock(event_outbox_backlog_lock_class(), 0);
    WITH gone AS (
        DELETE FROM event_outbox_backlog WHERE consumer = ANY(keys)
        RETURNING 1
    )
    INSERT INTO event_outbox_backlog (consumer, delta)
    SELECT k, (SELECT count(*) FROM event_outbox e
               WHERE e.consumer = k AND e.processed_at IS NULL)
    FROM unnest(keys) AS k;
    RETURN cardinality(keys);
END $;

-- Compacts every consumer's deltas to one row: the rows up to the highest
-- id in the snapshot are replaced by their per-consumer sums, and rows a
-- concurrent append writes after that id survive. It holds the ledger lock,
-- so two compactions, or a compaction and a recount, never interleave.
CREATE OR REPLACE FUNCTION compact_event_outbox_backlog()
RETURNS BIGINT LANGUAGE plpgsql AS $
DECLARE compacted BIGINT;
BEGIN
    PERFORM pg_advisory_xact_lock(event_outbox_backlog_lock_class(), 0);
    WITH gone AS (
        DELETE FROM event_outbox_backlog
        WHERE id <= (SELECT coalesce(max(id), 0) FROM event_outbox_backlog)
        RETURNING consumer, delta
    ), kept AS (
        INSERT INTO event_outbox_backlog (consumer, delta)
        SELECT consumer, sum(delta) FROM gone
        GROUP BY consumer HAVING sum(delta) <> 0
        RETURNING 1
    )
    SELECT count(*) INTO compacted FROM gone;
    RETURN compacted;
END $;
