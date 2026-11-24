-- @no-transaction
-- A fact may name the aggregate it belongs to; a claim hands out an
-- aggregate's rows one at a time in sequence order. Rows with the empty key
-- keep the created_at, id order they had.
--
-- Why: the pending-by-aggregate index is built CONCURRENTLY so appends
-- continue while it builds, which is only legal outside a transaction. The
-- defaulted NOT NULL columns are metadata-only adds. A build interrupted
-- part-way leaves an invalid index behind, so a replay drops it before
-- building again.
ALTER TABLE event_outbox ADD COLUMN IF NOT EXISTS aggregate_key TEXT NOT NULL DEFAULT '';
ALTER TABLE event_outbox ADD COLUMN IF NOT EXISTS aggregate_seq BIGINT NOT NULL DEFAULT 0;
DROP INDEX CONCURRENTLY IF EXISTS idx_event_outbox_aggregate_pending;
CREATE INDEX CONCURRENTLY idx_event_outbox_aggregate_pending
    ON event_outbox (consumer, aggregate_key, aggregate_seq, created_at, id)
    WHERE aggregate_key <> '' AND processed_at IS NULL;
