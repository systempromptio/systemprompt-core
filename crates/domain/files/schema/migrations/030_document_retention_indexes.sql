-- @no-transaction
-- An agent's repeat reads of one document on one day fold into the day's
-- first row: `reads` counts them and `last_read_at` is the latest. The agent
-- read retention job deletes the oldest rows first, by `created_at`.
--
-- Why: the index is built CONCURRENTLY so agent reads keep being recorded
-- while it builds, which is only legal outside a transaction. A build
-- interrupted part-way leaves an invalid index behind, so a replay drops it
-- before building again.
ALTER TABLE document_agent_reads ADD COLUMN IF NOT EXISTS reads INTEGER NOT NULL DEFAULT 1;
ALTER TABLE document_agent_reads ADD COLUMN IF NOT EXISTS last_read_at TIMESTAMPTZ;
DROP INDEX CONCURRENTLY IF EXISTS idx_document_agent_reads_created;
CREATE INDEX CONCURRENTLY idx_document_agent_reads_created
    ON document_agent_reads (created_at);
