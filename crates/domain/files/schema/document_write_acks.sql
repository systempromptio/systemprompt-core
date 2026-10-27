CREATE TABLE IF NOT EXISTS document_write_acks (
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    op_id TEXT NOT NULL CHECK (length(op_id) > 0),
    revision_id UUID NOT NULL REFERENCES document_revisions(id) ON DELETE CASCADE,
    ack JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    user_id VARCHAR(255),
    request_sha256 TEXT,
    PRIMARY KEY (document_id, op_id)
);

CREATE INDEX IF NOT EXISTS idx_document_write_acks_created
    ON document_write_acks (created_at);
