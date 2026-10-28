-- Agent caps: an owner-set override on a document or folder that only
-- tightens what the agent access policy allows agents, and an audit row for
-- every agent read made under a cap below edit.

ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS agent_cap TEXT
        CHECK (agent_cap IN ('none', 'read', 'comment', 'suggest', 'edit'));

ALTER TABLE document_folders
    ADD COLUMN IF NOT EXISTS agent_cap TEXT
        CHECK (agent_cap IN ('none', 'read', 'comment', 'suggest', 'edit'));

CREATE TABLE IF NOT EXISTS document_agent_reads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    agent_name TEXT NOT NULL CHECK (length(agent_name) BETWEEN 1 AND 64),
    attended BOOLEAN NOT NULL,
    action TEXT NOT NULL,
    effective_cap TEXT NOT NULL CHECK (effective_cap IN ('none', 'read', 'comment', 'suggest', 'edit')),
    cap_source TEXT NOT NULL,
    trace_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_document_agent_reads_document
    ON document_agent_reads (document_id, created_at);
