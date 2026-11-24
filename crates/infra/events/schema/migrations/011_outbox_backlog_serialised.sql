-- A recount and a compaction of the outbox backlog ledger take one
-- transaction-scoped advisory lock, and the recount deletes and counts on
-- one snapshot. Functions only, as the base schema; no table is locked or
-- rewritten.

-- The class of the one transaction-scoped advisory lock that serialises a
-- recount against a compaction; `systemprompt_database::OUTBOX_BACKLOG_LOCK_CLASS`
-- holds the same value. Appends never take it.
CREATE OR REPLACE FUNCTION event_outbox_backlog_lock_class()
RETURNS INTEGER LANGUAGE sql IMMUTABLE AS $$ SELECT 1397751873 $$;

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
RETURNS BIGINT LANGUAGE plpgsql AS $$
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
END $$;

-- Compacts every consumer's deltas to one row: the rows up to the highest
-- id in the snapshot are replaced by their per-consumer sums, and rows a
-- concurrent append writes after that id survive. It holds the ledger lock,
-- so two compactions, or a compaction and a recount, never interleave.
CREATE OR REPLACE FUNCTION compact_event_outbox_backlog()
RETURNS BIGINT LANGUAGE plpgsql AS $$
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
END $$;
