-- @no-transaction
-- @cost: rows=2000000 measured=20s triggers=live
-- The retention job deletes ai_request_messages and releases
-- ai_request_payloads bodies in batches selected by created_at; each batch
-- must reach its rows through an index, and the payload batch must skip the
-- rows already released rather than scan them again.
--
-- Why: the indexes are built CONCURRENTLY so writes to ai_request_messages
-- and ai_request_payloads continue while they build, which is only legal
-- outside a transaction. A build interrupted part-way leaves an invalid
-- index behind, so a replay drops it before building again.
DROP INDEX CONCURRENTLY IF EXISTS idx_ai_request_messages_created_at;
CREATE INDEX CONCURRENTLY idx_ai_request_messages_created_at
    ON ai_request_messages(created_at);

DROP INDEX CONCURRENTLY IF EXISTS idx_ai_request_payloads_bodies_created_at;
CREATE INDEX CONCURRENTLY idx_ai_request_payloads_bodies_created_at
    ON ai_request_payloads(created_at)
    WHERE request_body IS NOT NULL OR response_body IS NOT NULL;
