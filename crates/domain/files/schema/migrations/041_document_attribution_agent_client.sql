-- Conversation attribution: a revision records the agent and the OAuth
-- client that wrote it, and a comment records the conversation context and
-- the OAuth client it was written in.
--
-- Why: document_revisions is a large table; a nullable column with no
-- default and no constraint is a catalog-only change, so neither statement
-- rewrites or scans a table.
ALTER TABLE document_revisions
    ADD COLUMN IF NOT EXISTS agent_name TEXT,
    ADD COLUMN IF NOT EXISTS client_id TEXT;

ALTER TABLE document_comments
    ADD COLUMN IF NOT EXISTS context_id VARCHAR(255),
    ADD COLUMN IF NOT EXISTS client_id TEXT;
