-- @no-transaction
-- The daily upload quota sums a user's uploads since the start of the day.
--
-- Why: the index is built CONCURRENTLY so uploads keep being recorded while
-- it builds, which is only legal outside a transaction. A build interrupted
-- part-way leaves an invalid index behind, so a replay drops it before
-- building again.
DROP INDEX CONCURRENTLY IF EXISTS idx_document_uploads_quota_created;
CREATE INDEX CONCURRENTLY idx_document_uploads_quota_created
    ON document_uploads (quota_user_id, created_at);
