-- @no-transaction
-- The bridge Drive change feed pages revisions and path history on the
-- landing transaction id and a shared sequence, so a commit that lands
-- late, or a proposal accepted long after it was written, is still read
-- after any cursor a client holds. Rows written before this migration carry
-- no landing key; clients resume from a fresh manifest cursor.
--
-- Why: the feed index is built CONCURRENTLY so commits to document_revisions
-- continue while it builds, which is only legal outside a transaction. A
-- build interrupted part-way leaves an invalid index behind, so a replay
-- drops it before building again.
CREATE SEQUENCE IF NOT EXISTS document_landing_seq AS BIGINT;

ALTER TABLE document_revisions ADD COLUMN IF NOT EXISTS landing_xid BIGINT;
ALTER TABLE document_revisions ADD COLUMN IF NOT EXISTS landing_seq BIGINT;

ALTER TABLE document_path_changes ADD COLUMN IF NOT EXISTS landing_xid BIGINT;
ALTER TABLE document_path_changes ADD COLUMN IF NOT EXISTS landing_seq BIGINT;
ALTER TABLE document_path_changes
    ALTER COLUMN landing_xid SET DEFAULT pg_current_xact_id()::text::bigint;
ALTER TABLE document_path_changes
    ALTER COLUMN landing_seq SET DEFAULT nextval('document_landing_seq');

DROP INDEX CONCURRENTLY IF EXISTS idx_document_revisions_landing;
CREATE INDEX CONCURRENTLY idx_document_revisions_landing
    ON document_revisions (landing_xid, landing_seq)
    WHERE landing_xid IS NOT NULL;

DROP INDEX CONCURRENTLY IF EXISTS idx_document_path_changes_landing;
CREATE INDEX CONCURRENTLY idx_document_path_changes_landing
    ON document_path_changes (landing_xid, landing_seq)
    WHERE landing_xid IS NOT NULL;
