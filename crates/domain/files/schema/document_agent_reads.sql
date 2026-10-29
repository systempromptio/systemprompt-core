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
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    reads INTEGER NOT NULL DEFAULT 1,
    last_read_at TIMESTAMPTZ,
    read_day DATE,
    last_trace_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_document_agent_reads_document
    ON document_agent_reads (document_id, created_at);
CREATE INDEX IF NOT EXISTS idx_document_agent_reads_created
    ON document_agent_reads (created_at);
CREATE UNIQUE INDEX IF NOT EXISTS uq_document_agent_reads_day
    ON document_agent_reads
    (document_id, user_id, agent_name, attended, action, effective_cap, cap_source, read_day);
