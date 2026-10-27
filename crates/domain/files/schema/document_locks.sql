CREATE TABLE IF NOT EXISTS document_locks (
    document_id UUID PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    holder_user_id VARCHAR(255) NOT NULL,
    holder_kind TEXT NOT NULL CHECK (holder_kind IN ('bridge', 'editor', 'agent')),
    holder_id TEXT NOT NULL CHECK (length(holder_id) > 0),
    acquired_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_document_locks_expires_at ON document_locks (expires_at);
