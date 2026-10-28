CREATE TABLE IF NOT EXISTS document_blobs (
    content_sha256 TEXT PRIMARY KEY CHECK (content_sha256 ~ '^[0-9a-f]{64}$'),
    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),
    storage_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_document_blobs_created_at ON document_blobs (created_at);
