-- @no-transaction
-- Revisions are read per conversation by context id and time.
--
-- Why: the index is built CONCURRENTLY so commits to document_revisions
-- continue while it builds, which is only legal outside a transaction. A
-- build interrupted part-way leaves an invalid index behind, so a replay
-- drops it before building again.
DROP INDEX CONCURRENTLY IF EXISTS idx_document_revisions_context_created;
CREATE INDEX CONCURRENTLY idx_document_revisions_context_created
    ON document_revisions (context_id, created_at);
