CREATE TABLE IF NOT EXISTS document_presence (
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    user_id VARCHAR(255) NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('bridge', 'agent', 'web')),
    state TEXT NOT NULL CHECK (state IN ('viewing', 'editing')),
    cursor JSONB,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (document_id, user_id, source)
);

CREATE INDEX IF NOT EXISTS idx_document_presence_expires_at ON document_presence (expires_at);
CREATE INDEX IF NOT EXISTS idx_document_presence_user
    ON document_presence (user_id, document_id);
