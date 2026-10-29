-- @no-transaction
-- The recent listing builds its candidates from the caller's own revisions
-- and presence, and every listing reads the caller's last touch per document.
--
-- Why: the indexes are built CONCURRENTLY so writes to document_revisions and
-- document_presence continue while they build, which is only legal outside a
-- transaction. A build interrupted part-way leaves an invalid index behind,
-- so a replay drops it before building again.
DROP INDEX CONCURRENTLY IF EXISTS idx_document_revisions_author_document;
CREATE INDEX CONCURRENTLY idx_document_revisions_author_document
    ON document_revisions (author_user_id, document_id, created_at);
DROP INDEX CONCURRENTLY IF EXISTS idx_document_presence_user;
CREATE INDEX CONCURRENTLY idx_document_presence_user
    ON document_presence (user_id, document_id);
