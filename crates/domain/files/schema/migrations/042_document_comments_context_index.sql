-- @no-transaction
-- The comments written in one conversation context, by time.
--
-- Why: the index is built CONCURRENTLY so comment writes continue while it
-- builds, which is only legal outside a transaction. A build interrupted
-- part-way leaves an invalid index behind, so a replay drops it before
-- building again.
DROP INDEX CONCURRENTLY IF EXISTS idx_document_comments_context_created;
CREATE INDEX CONCURRENTLY idx_document_comments_context_created
    ON document_comments (context_id, created_at);
