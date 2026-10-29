-- @no-transaction
-- An agent's reads of one document on one UTC day land on one row keyed by
-- `read_day`: the first read inserts it and every later read bumps `reads`,
-- `last_read_at` and `last_trace_id` through the unique key, so two first
-- reads racing cannot both insert. Rows written before this migration keep a
-- NULL `read_day` and are never merged into; retention ages them out.
--
-- Why: the unique index is built CONCURRENTLY so agent reads keep being
-- recorded while it builds, which is only legal outside a transaction. A
-- build interrupted part-way leaves an invalid index behind, so a replay
-- drops it before building again.
ALTER TABLE document_agent_reads ADD COLUMN IF NOT EXISTS read_day DATE;
ALTER TABLE document_agent_reads ADD COLUMN IF NOT EXISTS last_trace_id TEXT;
DROP INDEX CONCURRENTLY IF EXISTS uq_document_agent_reads_day;
CREATE UNIQUE INDEX CONCURRENTLY uq_document_agent_reads_day
    ON document_agent_reads
    (document_id, user_id, agent_name, attended, action, effective_cap, cap_source, read_day);
